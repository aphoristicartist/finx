# Phase 0-2 Correctness Review: Data Fetching

## Scope Reviewed

- `crates/ferrotick-core/src/routing.rs`
- `crates/ferrotick-core/src/adapters/*.rs` (provider implementations live here; `src/provider/` does not exist)
- Supporting runtime behavior code used by providers:
  - `crates/ferrotick-core/src/data_source.rs`
  - `crates/ferrotick-core/src/throttling.rs`
  - `crates/ferrotick-core/src/cache.rs`
  - `crates/ferrotick-core/src/circuit_breaker.rs`

## Checklist Verdict

- All providers implement same interface: **PARTIAL**
  - All adapters implement `DataSource`.
  - Capability declarations and endpoint behavior are not fully consistent.
- Error handling is consistent: **NO**
- Rate limiting works correctly: **NO**
- Caching is implemented correctly: **NO**
- Circuit breaker logic is sound: **NO** (core primitive is okay; integration with routing/providers is not)

## Findings (Ordered by Severity)

## 1) Circuit breaker is bypassed in Yahoo quote/bars/search/fundamentals paths

- **Severity:** Critical
- **Where:**
  - `crates/ferrotick-core/src/adapters/yahoo.rs:405-415`
  - `crates/ferrotick-core/src/adapters/yahoo.rs:539-556`
  - `crates/ferrotick-core/src/adapters/yahoo.rs:849-851`
  - `crates/ferrotick-core/src/adapters/yahoo.rs:696-702`
- **Issue:** `get_crumb()` runs before `allow_request()` check, so network/auth refresh can happen even when breaker is open.
- **Impact:** Open breaker does not fully protect upstream calls; returned errors can be auth/network errors instead of breaker errors.
- **Observed behavior:** `when_network_request_fails_user_receives_unavailable_error` fails (`tests/error_handling_security.rs:33`) because error message does not mention circuit breaker state.

## 2) Open breaker can get stuck open under router health gating

- **Severity:** Critical
- **Where:**
  - Breaker state transition only in `allow_request`: `crates/ferrotick-core/src/circuit_breaker.rs:66-87`
  - `state()` is passive: `crates/ferrotick-core/src/circuit_breaker.rs:115-121`
  - Router skips unhealthy sources before invoke: `crates/ferrotick-core/src/routing.rs:405-415`
  - Adapter health derives from passive `state()`: e.g. `crates/ferrotick-core/src/adapters/yahoo.rs:379-399` (same pattern in other adapters)
- **Issue:** Router checks health first and skips Unhealthy providers, but only `allow_request()` advances Open -> HalfOpen after timeout.
- **Impact:** A provider can remain effectively excluded indefinitely in routed calls after opening.

## 3) Capability contracts are incorrect for Polygon and AlphaVantage

- **Severity:** High
- **Where:**
  - Polygon reports full capabilities: `crates/ferrotick-core/src/adapters/polygon.rs:332-334`
  - Polygon returns unsupported for `financials` and `earnings`: `crates/ferrotick-core/src/adapters/polygon.rs:417-447`
  - AlphaVantage reports full capabilities: `crates/ferrotick-core/src/adapters/alphavantage.rs:354-356`
  - AlphaVantage returns unsupported for `financials` and `earnings`: `crates/ferrotick-core/src/adapters/alphavantage.rs:424-454`
- **Issue:** Advertised capability set disagrees with actual endpoint implementations.
- **Impact:** Router planning and source scoring are misleading and can route to guaranteed unsupported endpoints.

## 4) Yahoo auth cache validity logic is broken

- **Severity:** High
- **Where:**
  - Validity requires both cookie and crumb: `crates/ferrotick-core/src/adapters/yahoo.rs:57-64`
  - Refresh writes crumb + timestamp only: `crates/ferrotick-core/src/adapters/yahoo.rs:158-160`
  - No cookie assignment in refresh flow.
- **Issue:** `cookie` in `YahooAuthManager` is never set, so `is_auth_valid()` stays false.
- **Impact:** Crumb/auth refresh runs repeatedly; higher latency and elevated upstream auth/rate-limit pressure.

## 5) Data response caching is not wired into fetching pipeline

- **Severity:** High
- **Where:**
  - Cache implementation exists: `crates/ferrotick-core/src/cache.rs`
  - No adapter/router usage of `CacheStore` / `CacheMode` (search in `routing.rs` and `adapters/*.rs`).
- **Issue:** Cache is implemented as utility but not applied to provider fetch calls.
- **Impact:** "Caching implemented correctly" is false for Phase 0-2 data fetching paths.

## 6) AlphaVantage throttling bookkeeping is incomplete for fundamentals

- **Severity:** Medium
- **Where:**
  - Acquire is called: `crates/ferrotick-core/src/adapters/alphavantage.rs:251`
  - Function returns without `complete_one()` / `record_success()`: `crates/ferrotick-core/src/adapters/alphavantage.rs:241-268`
  - Compare with quote/bars/search success handling using `complete_one()` and `record_success()`: `:113-114`, `:192-193`, `:306-307`
- **Issue:** Fundamentals path does not reconcile throttling queue nor breaker success state.
- **Impact:** Queue/state can drift from actual behavior under repeated fundamentals requests.

## 7) Error classification for rate limits is inconsistent

- **Severity:** Medium
- **Where:**
  - Yahoo crumb "Too Many Requests" mapped to `Unavailable`: `crates/ferrotick-core/src/adapters/yahoo.rs:149-153`
  - Most non-2xx statuses map to `Unavailable` in adapters rather than distinguishing 429.
- **Issue:** Similar upstream conditions are surfaced with different semantic error kinds.
- **Impact:** Retry/fallback policy and user messaging become inconsistent.

## 8) Core behavior requested by Phase 0-2 is not validated in active tests

- **Severity:** Medium
- **Where:** many behavior tests for multi-provider fetch/fallback are ignored, e.g.:
  - `tests/data_provider_behavior.rs:203`, `:325`
  - `tests/error_handling_security.rs:183`
  - `tests/cli_user_journeys.rs:22`, `:66`, etc.
- **Issue:** Most "actual behavior" tests for multi-provider success/fallback are disabled.
- **Impact:** Regressions in the most important runtime paths are likely to go undetected.

## Test Execution Summary

## Executed

- `cargo test -p ferrotick-core -- --nocapture`
  - Result: **FAILED**
  - Key outcome:
    - `tests/error_handling_security.rs::when_network_request_fails_user_receives_unavailable_error` **FAILED**
    - Most multi-provider success/fallback tests are **ignored**
- `cargo test -p ferrotick-core routing::tests::strict_source_does_not_fallback -- --nocapture`
  - Result: **PASSED**
- `cargo test -p ferrotick-core routing::tests::auto_chain_for_fundamentals_excludes_alpaca -- --nocapture`
  - Result: **PASSED**
- `cargo test -p ferrotick-core circuit_breaker::tests -- --nocapture`
  - Result: **PASSED**
- `cargo test -p ferrotick-core routing::tests::auto_falls_back_to_alpaca_after_polygon_rate_limit -- --ignored --nocapture`
  - Result: **FAILED** (all providers failed under Noop client path)

## Interpretation of Requested Runtime Checks

- Can fetch from multiple providers: **NOT VERIFIED in active suite** (relevant tests are ignored; ignored fallback test currently fails).
- Fallback logic works: **PARTIALLY VERIFIED**
  - Strict no-fallback path works (`strict_source_does_not_fallback` passes).
  - Auto fallback success path is not passing in current ignored test setup.
- Errors are properly propagated: **PARTIAL**
  - Source attribution and failure envelope paths work in active strict-failure tests.
  - One active behavior test currently fails on expected breaker error semantics.

## Overall Conclusion

Phase 0-2 data fetching is **not correctness-complete**.  
The largest blockers are breaker integration defects, capability mismatches, missing fetch-path caching, and insufficient active test coverage for multi-provider success/fallback behavior.
