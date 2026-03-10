# Phase 8 Backtesting Engine Correctness Review (Code-Only)

Scope reviewed:
- `crates/ferrotick-backtest/src/lib.rs` (export surface)
- Event-driven engine, portfolio, costs, metrics, and vectorized engine paths referenced by `lib.rs`

## Findings (ordered by severity)

1. **Critical: Backtest run resets portfolio cash to `0.0`, discarding configured initial capital**
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:153-156` calls `self.portfolio.reset()`.
- Evidence: `crates/ferrotick-backtest/src/portfolio/mod.rs:43-45` implements `reset()` as `Self::new(0.0)`.
- Impact: Portfolio tracking, equity curve, returns, drawdown, win/loss, and order feasibility are all corrupted from run start.
- Affects: Portfolio tracking, equity curve, performance metrics, win/loss tracking.

2. **Critical: "Next bar" execution uses stale previous bar data (not next bar open), introducing temporal misalignment and look-ahead risk**
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:160-171` executes pending orders before consuming current bar, using `latest_bars.get(&order.symbol)`.
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:215-218` updates `latest_bars` only when `Bar` event is processed later.
- Evidence: `crates/ferrotick-backtest/src/costs/slippage.rs:27-40` and `crates/ferrotick-backtest/src/engine/executor.rs:157-160` use bar `close` for market execution/reference.
- Impact: Signals can be effectively executed at prior close data, not true next-bar execution, invalidating time-order assumptions.
- Affects: Order execution logic, look-ahead bias, slippage realism, time ordering.

3. **High: End-of-run pending fills are published but never applied to portfolio**
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:190-205` publishes `BacktestEvent::Fill` for remaining pending orders.
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:208` immediately generates report without draining event queue again.
- Impact: Final fills do not change cash/positions/trade_count/win_rate/equity before report generation.
- Affects: Portfolio tracking, equity curve/final equity, win/loss tracking.

4. **High: Vectorized engine does not apply transaction costs or slippage at all**
- Evidence: `crates/ferrotick-backtest/src/vectorized/engine.rs:100-103` computes metrics from `signals -> equity_curve` only.
- Evidence: `crates/ferrotick-backtest/src/vectorized/engine.rs:204-217` updates equity directly from price/position with no fee/slippage terms.
- Impact: Reported performance is overstated versus event-driven path and requested cost/slippage requirements.
- Affects: Transaction costs, slippage handling, performance metrics.

5. **High: Vectorized path executes on same bar as signal generation (look-ahead)**
- Evidence: `crates/ferrotick-backtest/src/vectorized/engine.rs:127-141` signals are computed from row including current `close`.
- Evidence: `crates/ferrotick-backtest/src/vectorized/engine.rs:199-210` trades are executed using `prices[i]` for that same signal index.
- Impact: Uses information from bar `t` to trade at bar `t` price; inflated and biased results.
- Affects: Look-ahead bias, order execution correctness, time ordering.

6. **Medium: Win-rate classification is inaccurate for partial exits**
- Evidence: `crates/ferrotick-backtest/src/portfolio/mod.rs:94-99` increments `winning_trades` only when position becomes fully flat and only based on that final fill’s `realized_delta`.
- Impact: Multi-leg trades can be misclassified (e.g., profitable earlier partial exits ignored if final leg loses, or vice versa).
- Affects: Win/loss tracking correctness.

7. **Medium: Sortino ratio denominator is inconsistent with risk-free-adjusted numerator**
- Evidence: `crates/ferrotick-backtest/src/metrics/risk.rs:48-54` numerator uses `annualized_return - risk_free_rate`.
- Evidence: `crates/ferrotick-backtest/src/metrics/risk.rs:62-68` downside deviation uses only returns below `0.0` (not risk-free/target threshold).
- Impact: Sortino can be materially biased when risk-free rate is non-zero.
- Affects: Performance metric correctness (Sortino).

8. **Medium: Slippage accounting is absolute-only, so favorable price improvement is counted as positive slippage cost**
- Evidence: `crates/ferrotick-backtest/src/engine/executor.rs:55` computes `abs(execution_price - reference_price) * quantity`.
- Evidence: `crates/ferrotick-backtest/src/portfolio/cash.rs:67-80` accumulates `total_slippage` as non-negative metric.
- Impact: Slippage statistics can be overstated/misinterpreted (though cash impact uses execution price, so PnL cash flow itself is not double-charged).
- Affects: Slippage reporting correctness.

9. **Medium: Event-driven engine does not enforce monotonic timestamp ordering**
- Evidence: `crates/ferrotick-backtest/src/engine/event_driven.rs:160` iterates input `data` in caller-provided order with no ordering validation.
- Impact: Out-of-order bars can silently produce invalid fills/metrics and temporal leakage.
- Affects: Correct time ordering, bias control.

10. **Medium: Data snooping and survivorship controls are not enforced in-engine**
- Evidence: `crates/ferrotick-backtest/src/vectorized/engine.rs:77-93` performs full parameter sweep on same loaded dataset with no built-in out-of-sample split.
- Evidence: Engine interfaces consume already-prepared bars; no in-engine delisting/universe-at-time controls.
- Impact: Easy to overfit and to unknowingly run survivorship-biased studies unless external pipeline enforces controls.
- Affects: Data snooping risk, survivorship bias risk.

## Requested Verification Summary

- Portfolio tracking is accurate: **Fail** (Findings 1, 3, 6).
- Transaction costs are applied correctly: **Partial**.
  - Event-driven fee math path is internally consistent (`costs/fees.rs`, `portfolio/cash.rs`, `portfolio/position.rs`).
  - Vectorized path omits costs entirely (Finding 4).
- Performance metrics (Sharpe, Sortino, VaR): **Partial**.
  - Sharpe and historical VaR/CVaR formulas are implemented coherently.
  - Sortino implementation is threshold-inconsistent (Finding 7).
- Order execution logic is sound: **Fail** (Findings 2, 3, 5, 9).
- Slippage is handled properly: **Partial/Fail**.
  - Event-driven price impact is applied to execution price.
  - Temporal anchor and slippage reporting are flawed (Findings 2 and 8).
- Look-ahead bias: **Present** (Findings 2 and 5).
- Survivorship bias: **Not mitigated in engine** (Finding 10).
- Data snooping issues: **Not mitigated in engine** (Finding 10).
- Correct time ordering: **Not guaranteed** (Finding 9; plus stale-bar execution in Finding 2).
- Equity curve calculation: **Fail in aggregate** due reset/final-fill defects (Findings 1 and 3).
- Drawdown calculation: **Formula itself appears correct** (`metrics/drawdown.rs`), but depends on flawed equity series.
- Win/loss tracking: **Fail/Partial** due trade outcome attribution issue (Finding 6).

## Notes

- `cargo test -p ferrotick-backtest --quiet` passes, but current tests do not cover the critical temporal/capital-reset defects above.
