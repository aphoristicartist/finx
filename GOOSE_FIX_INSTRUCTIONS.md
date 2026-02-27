# Goose Fix Instructions

You need to fix the CRITICAL issues identified in REVIEW_CODEX.md.

## Critical Issues to Fix

### Issue 1: Look-ahead Bias (SAME-BAR EXECUTION)
**Problem:** Strategy signals generated from the current bar are executed against the same bar, allowing look-ahead bias.

**File:** `crates/ferrotick-backtest/src/engine/event_driven.rs`

**Fix Required:**
1. Orders generated on bar `t` should be queued as pending
2. Pending orders should be executed on bar `t+1` using the OPEN price
3. This prevents using close price information before it's available

**Implementation:**
- Add a `pending_orders: Vec<Order>` field to `BacktestEngine`
- When a signal generates an order, add it to `pending_orders` instead of executing immediately
- At the START of processing each bar, execute any `pending_orders` from the previous bar using the current bar's open price
- Then process the current bar (generate new signals)

### Issue 2: Limit/Stop Price Constraint Violations
**Problem:** Limit/stop orders can fill at prices that violate order constraints because execution always uses `bar.close`.

**File:** `crates/ferrotick-backtest/src/engine/executor.rs`

**Fix Required:**
1. **Buy Limit Orders:** Fill price must be <= limit_price
2. **Sell Limit Orders:** Fill price must be >= limit_price
3. **Stop Orders:** Must trigger correctly and fill at appropriate price

**Implementation:**
- For limit orders: Use `min(limit_price, execution_price)` for buys, `max(limit_price, execution_price)` for sells
- Ensure the fill price respects the order constraint
- Add validation that limit_price and stop_price are positive and finite

## Additional Fixes (Important)

### Issue 3: Win-rate Accounting
**File:** `crates/ferrotick-backtest/src/portfolio/mod.rs`

**Fix:** Only increment `closed_trades` when a position is FULLY closed (quantity goes to 0), not on every sell.

### Issue 4: Include Buy-side Fees in Realized PnL
**File:** `crates/ferrotick-backtest/src/portfolio/position.rs`

**Fix:** When calculating realized PnL, include both buy and sell fees, not just sell fees.

## Validation

After fixes:
1. Run `cargo check -p ferrotick-backtest`
2. Run `cargo test -p ferrotick-backtest`
3. Ensure no new warnings

## Files to Modify

1. `crates/ferrotick-backtest/src/engine/event_driven.rs` - Fix look-ahead bias
2. `crates/ferrotick-backtest/src/engine/executor.rs` - Fix price constraints
3. `crates/ferrotick-backtest/src/portfolio/mod.rs` - Fix win-rate accounting
4. `crates/ferrotick-backtest/src/portfolio/position.rs` - Fix fee accounting

Read REVIEW_CODEX.md for full details on each issue.
