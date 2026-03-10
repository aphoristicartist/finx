# Phase 9 Correctness Review

## Findings (Ordered by Severity)

### Critical

1. Signal-to-order routing is incorrect for multi-strategy execution.
   - Evidence:
     - `crates/ferrotick-strategies/src/signals/generator.rs:28` forwards every incoming signal to every strategy via `strategy.on_signal(signal)`.
     - All built-in strategies place orders for any `Buy`/`Sell` signal without checking `signal.strategy_name`:
       - `crates/ferrotick-strategies/src/strategies/ma_crossover.rs:100`
       - `crates/ferrotick-strategies/src/strategies/rsi_reversion.rs:121`
       - `crates/ferrotick-strategies/src/strategies/macd_trend.rs:138`
       - `crates/ferrotick-strategies/src/strategies/bb_squeeze.rs:128`
   - Impact:
     - One strategy signal can generate duplicate/conflicting orders from unrelated strategies.
   - Verdict: `FAIL`

2. Position sizing configuration is not applied correctly in strategy-built execution flow.
   - Evidence:
     - `build_strategy` passes `spec.position_sizing.value` directly as fixed `order_quantity` for all strategy types:
       - `crates/ferrotick-strategies/src/dsl/mod.rs:41`
       - `crates/ferrotick-strategies/src/dsl/mod.rs:50`
       - `crates/ferrotick-strategies/src/dsl/mod.rs:59`
       - `crates/ferrotick-strategies/src/dsl/mod.rs:67`
     - Strategies then always place fixed-size orders using `self.order_quantity` in `on_signal`.
   - Impact:
     - `percent`, `volatility`, and `kelly` methods are ignored in this path (e.g., `percent: 0.1` becomes `0.1` shares).
   - Verdict: `FAIL`

### High

3. DSL-to-strategy parameter extraction can silently ignore user intent.
   - Evidence:
     - RSI `overbought` is read only from `exit_rules` `operator == ">"`:
       - `crates/ferrotick-strategies/src/dsl/mod.rs:49`
     - `extract_value` matches only by operator, not indicator:
       - `crates/ferrotick-strategies/src/dsl/mod.rs:107`
     - `extract_period` uses loose indicator substring matching:
       - `crates/ferrotick-strategies/src/dsl/mod.rs:102`
   - Impact:
     - Valid-looking YAML can build with defaults/wrong parameters without explicit failure.
   - Verdict: `FAIL`

### Medium

4. DSL validation does not fully enforce “buildable strategy” constraints.
   - Evidence:
     - Strategy `type` is only checked for non-empty, not supported values:
       - `crates/ferrotick-strategies/src/dsl/validator.rs:19`
     - No operator/value-shape compatibility checks (e.g., `between` should require range).
     - No `period > 0` validation when `period` is provided in rules.
   - Impact:
     - Misconfigurations pass validation and fail later or behave unexpectedly.
   - Verdict: `FAIL`

5. Indicator compute failures are converted into `Hold` signals instead of surfacing errors.
   - Evidence:
     - RSI: `crates/ferrotick-strategies/src/strategies/rsi_reversion.rs:95`
     - MACD: `crates/ferrotick-strategies/src/strategies/macd_trend.rs:96`
     - BB: `crates/ferrotick-strategies/src/strategies/bb_squeeze.rs:91`
   - Impact:
     - Upstream data/indicator faults can be masked as neutral market behavior.
   - Verdict: `RISK`

## Requested Verification Matrix

| Area | Result | Notes |
|---|---|---|
| MA Crossover signal generation | PASS | Correct crossover logic using prior fast/slow state (`prev_fast`, `prev_slow`). |
| RSI oversold/overbought detection | PASS | Correct threshold checks (`rsi < oversold`, `rsi > overbought`). |
| MACD signal-line crossover | PASS | Correct prior/current MACD vs signal crossover logic. |
| BB Squeeze volatility detection | PASS | Detects squeeze via bandwidth threshold (`bandwidth < 0.05`). |
| Warmup periods | PASS | Warmups align with indicator implementations (`slow`, `period`, `slow+signal-1`, `period`). |
| State management | PASS | Reset and rolling history logic are correct for strategy-local state. |
| Signal generation logic (pipeline) | FAIL | Cross-strategy order fan-out bug in `SignalGenerator::on_signal`. |
| Position sizing correctness | FAIL | Strategy execution path bypasses sizing methods and uses fixed quantity. |
| DSL parses YAML | PASS | Struct parsing and unknown-field rejection work as implemented. |
| DSL creates valid strategies | FAIL | Parameter extraction and validation gaps can produce incorrect strategy instances. |
| DSL error handling | PASS | Parse errors map to `YamlParse`; validation errors map to `ValidationErrors`. |

## Validation Performed

- Read and reviewed all strategy, sizing, signal, trait, and DSL source files under `crates/ferrotick-strategies/src`.
- Ran test suite: `cargo test -p ferrotick-strategies`
  - Result: `43 passed, 0 failed` (`12 behavioral + 31 strategy tests`).
- Performed static cross-check against indicator warmup behavior in:
  - `crates/ferrotick-ml/src/features/indicators.rs`
