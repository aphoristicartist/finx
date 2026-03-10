# Phases 11-17 Correctness Review (Code-Only)

## Scope
- Reviewed implementation code for Phases 11-17:
  - Phase 11: `ferrotick-optimization`
  - Phase 12: `ferrotick-ai`
  - Phase 13: `ferrotick-backtest` vectorized engine
  - Phase 14: `ferrotick-ml` RL modules
  - Phase 15: `ferrotick-trading`
  - Phase 16: `ferrotick-web`
  - Phase 17: `ferrotick-core` multi-asset modules
- Executed tests:
  - `cargo test -p ferrotick-optimization -p ferrotick-backtest -p ferrotick-ml -p ferrotick-trading -p ferrotick-web --tests`
  - `cargo test -p ferrotick-ai`
  - `cargo test --test multiasset_behavioral --test assets_test`
- All executed tests passed, but several correctness issues remain (many are not covered by tests).

## Findings (Ordered by Severity)

### 1) [CRITICAL] Walk-forward can enter an infinite loop
- Phase: 11 (Optimization)
- Location: `crates/ferrotick-optimization/src/walk_forward.rs:79`, `crates/ferrotick-optimization/src/walk_forward.rs:105`, `crates/ferrotick-optimization/src/walk_forward.rs:121`, `crates/ferrotick-optimization/src/walk_forward.rs:153`
- Issue: `with_step()` accepts zero/tiny percentages; after float->usize conversion, `step_len` can be `0`, then `start += step_len` never advances inside the `while` loop.
- Impact: Validation can hang indefinitely for valid API calls (`with_step(0.0)` or very small values).

### 2) [CRITICAL] Vectorized backtest is not behaviorally equivalent to event-driven engine (look-ahead bias)
- Phase: 13 (Vectorized Backtesting)
- Location:
  - Same-bar execution in vectorized path: `crates/ferrotick-backtest/src/vectorized/engine.rs:204`
  - Event-driven next-bar execution: `crates/ferrotick-backtest/src/engine/event_driven.rs:161`, `crates/ferrotick-backtest/src/engine/event_driven.rs:228`
  - Equivalence test compares against a helper with same-bar fills, not the engine: `crates/ferrotick-backtest/tests/vectorized_test.rs:31`, `crates/ferrotick-backtest/tests/vectorized_test.rs:114`
- Issue: Vectorized path trades on the same bar used for signal generation, while the event-driven engine intentionally executes on the next bar.
- Impact: Reported "accuracy vs event-driven" is currently false-positive; vectorized results can be systematically optimistic.

### 3) [CRITICAL] Phase 16 backtest endpoint returns fake success without running a backtest
- Phase: 16 (Web Dashboard)
- Location: `crates/ferrotick-web/src/routes/backtest.rs:8`
- Issue: Endpoint always returns status `success` and default metrics, regardless of request content.
- Impact: API clients receive incorrect results and cannot detect that no backtest was executed.

### 4) [CRITICAL] Options module does not implement Black-Scholes or real Greeks
- Phase: 17 (Multi-Asset)
- Location: `crates/ferrotick-core/src/assets/options.rs:30`, `crates/ferrotick-core/src/assets/options.rs:44`, `crates/ferrotick-core/src/assets/options.rs:52`
- Issue: Pricing is intrinsic-value-only; Greeks are constants plus a binary delta step function.
- Impact: Options price/Greeks are materially incorrect for nearly all real use cases and do not meet stated Phase 17 requirements.

### 5) [HIGH] Live trading execution layer is unimplemented
- Phase: 15 (Real-Time Trading)
- Location: `crates/ferrotick-trading/src/executor/live.rs:1`, `crates/ferrotick-trading/src/brokers/ib.rs:1`
- Issue: Live execution and IB integration are placeholders.
- Impact: Real-time order execution path is non-functional.

### 6) [HIGH] Paper trading portfolio valuation is incorrect for multi-symbol scenarios
- Phase: 15 (Real-Time Trading / Paper Engine)
- Location: `crates/ferrotick-trading/src/paper/engine.rs:27`
- Issue: Incoming bar close is applied to every open position because bars have no symbol in this engine path.
- Impact: Multi-asset portfolio values and P&L become wrong as soon as symbols have different prices.

### 7) [HIGH] AI YAML sanitization is unreliable for realistic LLM responses
- Phase: 12 (AI Features)
- Location: `crates/ferrotick-ai/src/validation/sanitizer.rs:29`, `crates/ferrotick-ai/src/validation/sanitizer.rs:30`, `crates/ferrotick-ai/src/compiler/strategy.rs:65`
- Issue: `sanitize_yaml()` uses a single regex replacement that can leave surrounding prose/fences in place; compiler then feeds raw output into YAML parser.
- Impact: Strategy compilation can fail or parse unintended content for common "explanation + code block" responses.

### 8) [HIGH] Q-learning state encoding collapses price-change dimension during updates
- Phase: 14 (Reinforcement Learning)
- Location: `crates/ferrotick-ml/src/rl/qtable.rs:126`, `crates/ferrotick-ml/src/rl/qtable.rs:148`, `crates/ferrotick-ml/src/rl/qtable.rs:159`
- Issue: `choose_action()` updates `last_price` to current price before `update()`, then `update()` builds current `state_key` from the same price, forcing current-step price change to `Flat`.
- Impact: Q-table learns with degraded state information, reducing policy quality and learning correctness.

### 9) [MEDIUM] Grid search combination accounting is internally inconsistent for empty parameter space
- Phase: 11 (Optimization)
- Location: `crates/ferrotick-optimization/src/grid_search.rs:87`, `crates/ferrotick-optimization/src/grid_search.rs:179`
- Issue: `total_combinations()` returns `0` when no params exist, but `generate_combinations()` returns one empty combination.
- Impact: Confusing/incorrect reporting and edge-case behavior in optimization orchestration.

### 10) [MEDIUM] Vectorized DuckDB path does not honor symbol isolation despite schema
- Phase: 13 (Vectorized Backtesting)
- Location: `crates/ferrotick-backtest/src/vectorized/engine.rs:35`, `crates/ferrotick-backtest/src/vectorized/engine.rs:50`, `crates/ferrotick-backtest/src/vectorized/engine.rs:129`
- Issue: Symbol is stored but queries do not filter by symbol; load also wipes table (`DELETE FROM bars`) each call.
- Impact: Design cannot safely extend to multi-symbol sweeps without query/data correctness regressions.

### 11) [MEDIUM] Backtest API request model has no validation guards
- Phase: 16 (Web Dashboard)
- Location: `crates/ferrotick-web/src/models/api.rs:4`, `crates/ferrotick-web/src/routes/backtest.rs:5`
- Issue: No validation for symbol format, date ordering, or `initial_capital > 0`.
- Impact: Invalid requests are accepted and reported as successful.

### 12) [MEDIUM] Forex conversion can divide by zero
- Phase: 17 (Multi-Asset)
- Location: `crates/ferrotick-core/src/assets/forex.rs:13`, `crates/ferrotick-core/src/assets/forex.rs:31`
- Issue: `exchange_rate` is not validated at construction; converting quote->base divides by rate directly.
- Impact: Zero/invalid rates can produce infinities/NaNs in downstream calculations.

### 13) [MEDIUM] Alpaca client does not fail fast on non-2xx HTTP status
- Phase: 15 (Real-Time Trading)
- Location: `crates/ferrotick-trading/src/brokers/alpaca.rs:55`, `crates/ferrotick-trading/src/brokers/alpaca.rs:74`
- Issue: Responses are decoded without `error_for_status()`.
- Impact: Broker rejections can surface as misleading deserialization/network errors instead of explicit API failures.

## Residual Testing Gaps
- No tests currently exercise:
  - Walk-forward zero-step handling.
  - Vectorized equivalence against `BacktestEngine` (next-bar fill semantics).
  - Web `/api/backtest/run` request validation and real execution path.
  - Real live-order execution flow.
  - Black-Scholes formula correctness and Greek sensitivity checks.
