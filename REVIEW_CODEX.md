# Phase 8 Implementation Review (Codex)

## Summary
The `ferrotick-backtest` crate has a clean modular structure and compiles, but core execution/accounting behavior currently produces biased and sometimes invalid backtest results. The biggest blockers are same-bar signal execution (look-ahead bias), limit/stop fill pricing that can violate order constraints, and trade statistics that can misreport win rate and realized PnL.

## Strengths
- Good crate/module decomposition (`engine`, `portfolio`, `metrics`, `costs`) with clear boundaries.
- Correct integration with `ferrotick-core` domain types (`Bar`, `Symbol`, `UtcDateTime`).
- Event model (`Bar -> Signal -> Order -> Fill`) is straightforward and easy to extend.
- Cost modeling interfaces are simple and serializable (`FeeModel`, `SlippageModel`).
- Basic risk metrics (Sharpe, Sortino, drawdown, VaR/CVaR) are implemented and wired into reports.

## Issues Found

### Critical
- Same-bar execution creates look-ahead bias. In [`crates/ferrotick-backtest/src/engine/event_driven.rs:177`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/event_driven.rs:177), strategy signals generated from the current bar are converted to orders and executed against the same bar in [`event_driven.rs:188`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/event_driven.rs:188). This allows using bar-close information and filling at that same bar, inflating performance.
- Limit/stop orders can fill at prices that violate order constraints. Execution always uses slippage on `bar.close` in [`executor.rs:42-44`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/executor.rs:42), even for limit/stop orders triggered via high/low checks. A buy limit can be filled above its limit, and a sell limit below its limit.

### Important
- `start_date` / `end_date` are unused. They exist in [`event_driven.rs:58-59`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/event_driven.rs:58) but are never applied in `run`, so config does not enforce backtest date bounds.
- Trade win-rate accounting is inaccurate for partial exits. `closed_trades` increments on every sell fill in [`portfolio/mod.rs:87-91`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/portfolio/mod.rs:87), not when a trade round-trip actually closes.
- Realized PnL used for win-rate ignores buy-side fees. Sell-side realized PnL subtracts only sell fees in [`position.rs:105`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/portfolio/position.rs:105). Buy fees reduce cash but not position cost basis/realized trade PnL, which can overstate wins.
- Engine state is not reset between runs. `BacktestEngine::run` mutates persistent `portfolio`, `latest_bars`, and `event_bus` state and does not clear them, so reusing the same engine instance carries prior run state.
- Input validation gaps for order price fields. `execute` checks quantity but does not validate `limit_price`/`stop_price` finiteness or positivity when present; invalid values can pass through trigger logic.

### Minor
- `OrderStatus` is defined but never meaningfully used/updated (`New` is set in [`portfolio/order.rs:63`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/portfolio/order.rs:63), but no lifecycle transitions are recorded).
- `BacktestEngine::run` is `async` without awaiting anything (could be synchronous until real async feeds are introduced).
- No TODO markers for explicitly deferred features, despite planned future items (for example vectorized backtesting).
- VaR/CVaR are returned as raw return quantiles (often negative). If the intended API is loss magnitude, sign conventions should be documented or normalized.

## Missing Features
- No CLI integration for Phase 8 deliverable (`ferrotick backtest ...`) appears implemented in current source tree.
- No benchmark comparison output (e.g., S&P 500) as listed in roadmap deliverables/tasks.
- No persistent order book/lifecycle support (pending orders across bars, cancel/reject paths, status transitions).
- No partial-fill / liquidity-cap execution model (volume participation is only used for slippage bps).
- No explicit support for short-selling/margin workflows (currently sell quantities are constrained by held long position).
- No vectorized engine stub/module (`vectorized.rs`) and no in-code TODO explaining deferment.

## Testing Recommendations
- Add unit tests for execution realism:
  - `limit_buy` never fills above limit; `limit_sell` never below limit.
  - Stop orders trigger/fill with gap scenarios (open through stop).
- Add regression tests preventing look-ahead bias:
  - Signal generated on bar `t` executes earliest at bar `t+1` (or documented policy).
- Add portfolio accounting tests:
  - Round-trip trade PnL includes both buy/sell fees.
  - Partial exits do not inflate `closed_trades` and `win_rate`.
- Add config behavior tests:
  - `start_date`/`end_date` filtering and boundary inclusion behavior.
  - Re-running engine instance does not contaminate results (or enforce single-use API).
- Add robustness tests:
  - Invalid/NaN limit/stop prices rejected.
  - Extreme fee/slippage inputs do not produce NaN/inf report fields.
- Add metrics tests with known fixtures:
  - Sharpe/Sortino/drawdown/VaR/CVaR against deterministic expected values.

## Code Examples
```rust
// Current behavior (look-ahead): signal from current bar can be executed on current bar
if let Some(signal) = strategy.on_bar(&bar_event, &self.portfolio) {
    self.event_bus.publish(BacktestEvent::Signal(signal))?;
}
...
BacktestEvent::Order(order) => {
    let bar = self.latest_bars.get(&order.symbol)...?;
    if let Some(fill) = self.order_executor.execute(&order, bar, &self.config.costs)? {
        self.event_bus.publish(BacktestEvent::Fill(fill))?;
    }
}
```
Source: [`crates/ferrotick-backtest/src/engine/event_driven.rs:177`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/event_driven.rs:177), [`event_driven.rs:188`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/event_driven.rs:188)

```rust
// Current behavior: all orders price off close, even limit/stop
let execution_price = self
    .slippage
    .execution_price(order.side, bar, order.quantity);
```
Source: [`crates/ferrotick-backtest/src/engine/executor.rs:42`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/engine/executor.rs:42)

```rust
// Current behavior: win-rate increments on every sell fill
if fill.side == OrderSide::Sell {
    self.closed_trades += 1;
    if realized_delta > 0.0 {
        self.winning_trades += 1;
    }
}
```
Source: [`crates/ferrotick-backtest/src/portfolio/mod.rs:87`](/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-backtest/src/portfolio/mod.rs:87)

Recommended direction (high-level):
```rust
// Enforce non-lookahead policy:
// 1) Generate orders on bar t
// 2) Queue them as pending
// 3) Fill earliest on next bar using open/limit/stop logic with price bounds

// Enforce price constraints:
// buy limit fill_price <= limit
// sell limit fill_price >= limit
// stop orders transition to market-on-trigger with explicit trigger price policy
```

## Verdict
- [ ] APPROVED - Ready to merge
- [x] NEEDS FIXES - Requires changes before merge
- [ ] MAJOR REVISION - Significant rework needed

## Detailed Notes
- Architecture quality is strong: the crate shape and event abstraction are maintainable and aligned with the Phase 8 direction.
- The largest risk is correctness, not structure. Current execution semantics can materially overestimate performance.
- Report quality is currently constrained by accounting semantics (trade closure and fee attribution), which directly affects user-facing `win_rate` and potentially strategy ranking.
- The module compiles and is easy to read, but production-grade confidence is low due to zero tests (`cargo test -p ferrotick-backtest` ran with 0 unit/doc tests in this crate).
- Recommend fixing critical execution semantics first, then adding a focused regression test suite before further feature expansion.
