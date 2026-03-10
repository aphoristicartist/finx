# INTEGRATION_AUDIT.md

Date: 2026-03-01  
Workspace: ferrotick

## Scope
Cross-crate integration review for:

1. `ferrotick-core (data) -> ferrotick-ml (features) -> ferrotick-backtest (testing)`
2. `ferrotick-strategies -> ferrotick-backtest -> ferrotick-optimization`
3. `ferrotick-ml -> ferrotick-strategies -> ferrotick-backtest`
4. Full pipeline: `Data -> Features -> ML -> Strategy -> Backtest -> Optimization -> Trading`

## Verification Runs
- `cargo test -p ferrotick-tests --test integration_workflows` (pass)
- `cargo test -p ferrotick-optimization --tests` (pass)
- `cargo test -p ferrotick-strategies --tests` (pass)
- `cargo test -p ferrotick-backtest --tests` (pass)
- `cargo test -p ferrotick-trading --tests` (pass)
- `cargo test -p ferrotick-tests --test integration_test` (pass)
- `cargo test -p ferrotick-tests --test edge_cases` (pass)

## Findings (Ordered by Severity)

### 1. CRITICAL: Backtest run resets capital to `0.0`
- Evidence:
  - `BacktestEngine::run` calls `self.portfolio.reset()` at `crates/ferrotick-backtest/src/engine/event_driven.rs:153-155`.
  - `Portfolio::reset` hard-resets to `Self::new(0.0)` at `crates/ferrotick-backtest/src/portfolio/mod.rs:43-45`.
- Impact:
  - Capital integrity is broken across every backtest/optimization run.
  - Reported metrics can be materially wrong even when tests pass.
- Required fix:
  - Reset to configured initial capital, not zero.

### 2. CRITICAL: Pending orders execute against stale (previous) bar, not current bar
- Evidence:
  - Comment says execution uses "this bar" at `crates/ferrotick-backtest/src/engine/event_driven.rs:161`.
  - Code actually executes against `latest_bars.get(&order.symbol)` before current `BarEvent` is published (`:165-171`, then current bar publish happens at `:178-180`).
  - `latest_bars` is updated only when processing `BacktestEvent::Bar` (`:214-217`).
- Impact:
  - Fill timing/price integrity is incorrect.
  - Signal-to-fill behavior diverges from intended next-bar execution semantics.

### 3. HIGH: `ferrotick-strategies` is not interface-compatible with `ferrotick-backtest`
- Evidence:
  - `ferrotick-strategies::Strategy` uses `on_bar(&Bar) -> Option<Signal>` and `on_signal(&Signal) -> Option<Order>` with `String` symbol/timestamp (`crates/ferrotick-strategies/src/traits/strategy.rs:11-55`).
  - `ferrotick-backtest::Strategy` uses `on_bar(&BarEvent, &Portfolio) -> Option<SignalEvent>` and `create_order(...)` with typed `Symbol`/`UtcDateTime` (`crates/ferrotick-backtest/src/engine/event_driven.rs:98-109`).
  - No adapter layer exists between these traits in reviewed crates.
- Impact:
  - Strategy flow `strategies -> backtest -> optimization` is not end-to-end wire-compatible.

### 4. HIGH: CLI/Web strategy backtest integration is stubbed, not real
- Evidence:
  - CLI `strategy backtest` explicitly states pending integration (`crates/ferrotick-cli/src/commands/strategy.rs:45-47`).
  - Web backtest route is a stub default response (`crates/ferrotick-web/src/routes/backtest.rs:8-13`).
- Impact:
  - Public entrypoints do not execute real cross-crate backtest flow.

### 5. HIGH: Optimization silently swallows backtest failures and drops error context
- Evidence:
  - `GridSearchOptimizer::optimize` logs errors with `eprintln!` and continues (`crates/ferrotick-optimization/src/grid_search.rs:158-160`).
  - `WalkForwardValidator::run_backtest` converts `Result` to `Option` via `.ok()` (`crates/ferrotick-optimization/src/walk_forward.rs:208`).
- Impact:
  - Error propagation is weak.
  - Failed parameter runs can disappear from reports without structured failure reason.

### 6. HIGH: Optimization forces synthetic symbol `OPT`, losing symbol fidelity
- Evidence:
  - `Symbol::parse("OPT")` used for all bars in grid search (`crates/ferrotick-optimization/src/grid_search.rs:116-127`).
  - Same in walk-forward (`crates/ferrotick-optimization/src/walk_forward.rs:184-192`).
- Impact:
  - Multi-asset symbol identity is discarded.
  - Data integrity is reduced for symbol-specific strategies/analytics.

### 7. MEDIUM: ML models are not integrated into strategy engine; only indicator functions are used
- Evidence:
  - Strategies import only ML indicator helpers:
    - RSI: `crates/ferrotick-strategies/src/strategies/rsi_reversion.rs:4`
    - MACD: `crates/ferrotick-strategies/src/strategies/macd_trend.rs:4`
    - Bollinger: `crates/ferrotick-strategies/src/strategies/bb_squeeze.rs:4`
  - ML model API is available (`crates/ferrotick-ml/src/lib.rs:9`) but not consumed by strategy implementations.
  - Integration test bridges predictions to backtest with a custom test strategy, bypassing `ferrotick-strategies` (`tests/integration_workflows.rs:93-121`, `:203-212`, `:237-249`).
- Impact:
  - ML flow `ml -> strategies -> backtest` is partial (indicator-driven, not model-driven).

### 8. MEDIUM: Core bar data loses `vwap` when loaded into ML feature store
- Evidence:
  - Query selects only `ts, open, high, low, close, volume` (`crates/ferrotick-ml/src/features/store.rs:74-77`).
  - `Bar::new` is called with `vwap = None` (`:111-119`).
- Impact:
  - Data loss at `core -> ml` boundary for `vwap`.

### 9. MEDIUM: Feature rows convert typed identifiers to raw strings
- Evidence:
  - `FeatureRow` stores `symbol: String`, `timestamp: String` (`crates/ferrotick-ml/src/features/mod.rs:92-94`).
  - Conversion from typed values occurs in feature generation (`:179-180`).
- Impact:
  - Type safety degrades between feature and downstream consumers.
  - Re-parse failures become possible where compile-time guarantees previously existed.

### 10. MEDIUM: Dataset builder drops rows with partial features/targets without reporting drop counts
- Evidence:
  - Rows are included only if all feature options and selected target are `Some` (`crates/ferrotick-ml/src/training/dataset.rs:147-153`).
- Impact:
  - Silent row loss can bias training/validation and confuse pipeline observability.

### 11. MEDIUM: Trading endpoint is not full-pipeline ready
- Evidence:
  - Live execution is TODO (`crates/ferrotick-trading/src/executor/live.rs:1-6`).
  - Paper engine updates every open position using the incoming bar close because `Bar` has no symbol (`crates/ferrotick-trading/src/paper/engine.rs:27-31`).
- Impact:
  - Multi-asset pricing correctness is compromised in paper trading.
  - Full `... -> Trading` pipeline is incomplete.

## Flow Verdicts

### 1) Data Flow: `core -> ml -> backtest`
- Data types are compatible: **PARTIAL**
  - `core::Bar` is accepted by ML features (`crates/ferrotick-ml/src/features/mod.rs:127`) and by backtest via `BarEvent` (`crates/ferrotick-backtest/src/engine/event_driven.rs:20-32`).
  - But feature outputs use stringly-typed IDs/timestamps (`crates/ferrotick-ml/src/features/mod.rs:92-94`).
- Conversions are correct: **PARTIAL/FAIL**
  - `vwap` is dropped in ML load path (`crates/ferrotick-ml/src/features/store.rs:74-77`, `:111-119`).
- No data loss: **FAIL**
  - `vwap` loss and row filtering behavior (`crates/ferrotick-ml/src/training/dataset.rs:147-153`).

### 2) Strategy Flow: `strategies -> backtest -> optimization`
- Strategy interface is consistent: **FAIL**
  - Trait mismatch (`crates/ferrotick-strategies/src/traits/strategy.rs:51-55` vs `crates/ferrotick-backtest/src/engine/event_driven.rs:99-109`).
- Signals are passed correctly: **FAIL (no standard bridge)**
  - No adapter; CLI/web backtest paths are stubs (`crates/ferrotick-cli/src/commands/strategy.rs:45-47`, `crates/ferrotick-web/src/routes/backtest.rs:8-13`).
- Optimization works end-to-end: **PARTIAL**
  - Works with backtest-native strategies, but error handling/symbol fidelity issues remain (`crates/ferrotick-optimization/src/grid_search.rs:116-160`).

### 3) ML Flow: `ml -> strategies -> backtest`
- ML models integrate with strategies: **FAIL**
  - Strategies consume indicators, not model predictions (`rsi_reversion.rs:4`, `macd_trend.rs:4`, `bb_squeeze.rs:4`).
- Predictions drive signals: **PARTIAL (test-only manual bridge)**
  - Test converts predictions to actions manually (`tests/integration_workflows.rs:203-212`, `:237-249`).
- Backtest uses ML correctly: **PARTIAL**
  - Backtest can run custom ML-driven backtest strategy in tests (`tests/integration_workflows.rs:93-121`, `:227-268`), but this bypasses `ferrotick-strategies`.

### 4) Full Pipeline: `Data -> Features -> ML -> Strategy -> Backtest -> Optimization -> Trading`
- Type compatibility at transitions: **FAIL/PARTIAL**
  - Break at `strategies <-> backtest` interface boundary.
- Error propagation: **FAIL/PARTIAL**
  - Optimization suppresses errors (`grid_search.rs:158-160`, `walk_forward.rs:208`).
- Data integrity: **FAIL/PARTIAL**
  - `vwap` dropped, symbol flattening to `OPT`, stale-bar execution, capital reset.
- End-to-end executable status: **NOT COMPLETE**
  - Live trading not implemented (`crates/ferrotick-trading/src/executor/live.rs:1-6`).

## Test Coverage Gaps Relevant to Integration
- No integration tests assert backtest starts with configured initial capital after `run()` reset path.
- No integration tests validate that pending orders execute on intended bar (current vs stale).
- No production-path test validates `ferrotick-strategies` directly inside `ferrotick-backtest` without custom test adapters.
- No end-to-end pipeline test reaches actual trading execution (live path is TODO).

## Overall Audit Result
Cross-crate integration is **partially functional** for isolated and test-specific paths, but **fails full end-to-end requirements** due to interface mismatches, critical backtest execution defects, incomplete public backtest/trading wiring, and data/error propagation integrity gaps.
