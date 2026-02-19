# Task: Implement Phase 5 - Alpha Vantage + Alpaca Adapters

## Objective
Add Alpha Vantage and Alpaca market data providers to complete the four-provider ecosystem (Yahoo, Polygon, Alpha Vantage, Alpaca) with proper rate limiting, provider policies, and contract tests.

## Requirements

### 1. Alpha Vantage Adapter
1. Create `crates/finx-core/src/adapters/alphavantage.rs`
2. Implement `DataSource` trait with all four endpoints (quote, bars, fundamentals, search)
3. Use query parameter authentication (`apikey`)
4. Set provider score to 70 (lower than Polygon due to rate limits)
5. Read API key from `FINX_ALPHAVANTAGE_API_KEY` environment variable, fallback to "demo"
6. Implement rate limiting using `governor` crate (5 calls per minute for free tier)
7. Follow existing adapter pattern from `polygon.rs` and `yahoo.rs`

### 2. Alpaca Adapter
1. Create `crates/finx-core/src/adapters/alpaca.rs`
2. Implement `DataSource` trait with only quote and bars endpoints (NO fundamentals or search)
3. Use dual header authentication (`APCA-API-KEY-ID` and `APCA-API-SECRET-KEY`)
4. Set provider score to 85 (reliable, fast)
5. Read API key from `FINX_ALPACA_API_KEY` and secret from `FINX_ALPHAVANTAGE_SECRET_KEY` environment variables
6. Follow existing adapter pattern from `polygon.rs` and `yahoo.rs`
7. Return `SourceError::unsupported_endpoint` for fundamentals and search endpoints

### 3. Provider Policies System
1. Create `crates/finx-core/src/provider_policy.rs`
2. Define `ProviderPolicy` struct with:
   - `provider_id: ProviderId`
   - `max_concurrency: usize`
   - `quota_window: Duration`
   - `quota_limit: u32`
   - `retry_backoff: BackoffPolicy`
3. Define `BackoffPolicy` struct with:
   - `initial_delay: Duration`
   - `max_delay: Duration`
   - `multiplier: f64`
   - `max_retries: u32`
4. Implement default policies for Alpha Vantage (1 concurrent, 5 calls/min) and Alpaca (10 concurrent, 100 calls/min)

### 4. Throttling Infrastructure
1. Create `crates/finx-core/src/throttling.rs`
2. Implement `ThrottlingQueue` to buffer requests when rate limits are exceeded
3. Integrate with `governor` crate for rate limiting logic
4. Implement exponential backoff for retries

### 5. Routing Updates
1. Update `crates/finx-core/src/routing.rs` to include Alpha Vantage and Alpaca in provider selection
2. Update provider capability matrix to reflect new providers
3. Ensure fallback chain respects capability constraints (Alpaca not used for fundamentals/search)

### 6. Contract Tests
1. Create `tests/contract/provider_contract.rs`
2. Implement shared contract tests that all four providers must pass:
   - Quote returns valid structure
   - Bars respects limit
   - Unsupported endpoints return appropriate error
3. Test canonical output parity across providers

### 7. Module Registration
1. Update `crates/finx-core/src/adapters/mod.rs` to export new adapters
2. Update `crates/finx-core/src/lib.rs` to export new types
3. Update `crates/finx-cli/src/commands/sources.rs` to include new providers in output

### 8. Documentation
1. Update `README.md` with new provider information
2. Document environment variables for API keys
3. Update capability matrix in documentation

## Affected Files

### New Files:
- `crates/finx-core/src/adapters/alphavantage.rs` — create: Alpha Vantage adapter implementation
- `crates/finx-core/src/adapters/alpaca.rs` — create: Alpaca adapter implementation
- `crates/finx-core/src/provider_policy.rs` — create: Per-provider policy configuration
- `crates/finx-core/src/throttling.rs` — create: Rate limiting and throttling infrastructure
- `tests/contract/provider_contract.rs` — create: Shared contract test suite

### Modified Files:
- `crates/finx-core/src/adapters/mod.rs` — modify: Export Alpha Vantage and Alpaca adapters
- `crates/finx-core/src/routing.rs` — modify: Update provider selection logic
- `crates/finx-core/src/lib.rs` — modify: Export new types (ProviderPolicy, BackoffPolicy, ThrottlingQueue)
- `crates/finx-cli/src/commands/sources.rs` — modify: Include new providers in output
- `README.md` — modify: Document new providers and configuration

## Approach

1. **Follow existing patterns**: Both new adapters should follow the structure of `PolygonAdapter` and `YahooAdapter`, using the same `DataSource` trait, `HttpAuth` patterns, and circuit breaker integration.

2. **Capability-based routing**: The routing system already supports capability checks - we just need to register the new providers with their capability matrices. Alpaca will have `fundamentals: false` and `search: false`.

3. **Provider policies**: Create a separate module for policy configuration to keep it modular and testable. This allows future expansion without modifying adapter code.

4. **Rate limiting**: Use the `governor` crate (already in dependencies) for Alpha Vantage's strict rate limits. Alpaca doesn't need rate limiting beyond the existing circuit breaker.

5. **Contract tests**: Create a parameterized test suite that runs the same tests against all providers, ensuring consistent behavior.

## Acceptance Criteria

- [ ] `cargo test` passes with 0 failures
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` produces no warnings
- [ ] All four providers (Yahoo, Polygon, Alpha Vantage, Alpaca) pass shared contract tests
- [ ] Alpha Vantage adapter implements all four endpoints (quote, bars, fundamentals, search)
- [ ] Alpaca adapter implements quote and bars, returns errors for fundamentals and search
- [ ] Provider routing selects providers based on capabilities and scores
- [ ] Rate limiting works correctly for Alpha Vantage (5 calls/min)
- [ ] Environment variables documented in README
- [ ] No TODOs, FIXMEs, or placeholder implementations

## Out of Scope

- Actual HTTP transport implementation (adapters use injected `HttpClient` trait)
- Real API key integration tests (use mock/demo mode)
- Provider health dashboard
- Real-time rate limit monitoring
- Automatic provider failover beyond existing circuit breaker
- Changes to finx-warehouse or finx-cli crates beyond `sources.rs` command
