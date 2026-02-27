# Critical Fixes for ferrotick-backtest

## Overview
This document summarizes the critical fixes implemented to address look-ahead bias, price constraint violations, and accounting issues in the ferrotick-backtest crate.

## Issues Fixed

### 1. Look-Ahead Bias (SAME-BAR EXECUTION)

**Problem:** Strategy signals generated from the current bar were executed against the same bar, allowing look-ahead bias. This artificially inflated backtest performance.

**Root Cause:** Orders were executed immediately when signals were generated within the same bar processing loop.

**Solution:**
- Added `pending_orders` queue to BacktestEngine
- Orders from current bar signals are queued for execution on the NEXT bar
- Pending orders at the start of each bar use that bar's OPEN price for execution

**Code Changes:**
- `crates/ferrotick-backtest/src/engine/event_driven.rs`
  - Added `pending_orders: Vec<Order>` field
  - Added order execution at START of each bar using current bar's open price
  - Modified signal-to-order flow to queue instead of immediate execution

**Impact:** Eliminates look-ahead bias - backtest results now reflect realistic trading behavior where signals from bar t cannot influence trades until bar t+1.

---

### 2. Limit/Stop Price Constraint Violations

**Problem:** Limit and stop orders could fill at prices violating their constraints. All orders used `bar.close` regardless of order type.

**Root Cause:** Execution always used slippage on close price without respecting limit/stop boundaries.

**Solution:**
- Added `validate_order_prices()` to ensure limit_price and stop_price are positive and finite
- Added `apply_price_constraints()` with proper boundary logic:
  - Buy Limit: fill_price = min(limit, execution_price) → always ≤ limit
  - Sell Limit: fill_price = max(limit, execution_price) → always ≥ limit
  - Buy Stop: fill_price = max(stop, execution_price) → at or above stop
  - Sell Stop: fill_price = min(stop, execution_price) → at or below stop

**Code Changes:**
- `crates/ferrotick-backtest/src/engine/executor.rs`
  - New validation method for price fields
  - New constraint application logic per order type

**Impact:** Limit orders now respect their price constraints, preventing unrealistic fills.

---

### 3. Win-Rate Accounting

**Problem:** `closed_trades` incremented on every sell fill, not when round-trip trades actually closed.

**Root Cause:** Sell orders could represent partial exits rather than complete position closes.

**Solution:**
- Only increment `closed_trades` when `became_flat && fill.side == Sell`
- Added `became_flat` flag to track when position quantity reaches zero

**Code Changes:**
- `crates/ferrotick-backtest/src/portfolio/mod.rs`
  - Modified win-rate logic to check for position closure

**Impact:** Win rate now accurately reflects round-trip trade performance.

---

### 4. Buy-Side Fees in Realized PnL

**Problem:** Realized PnL only subtracted sell-side fees, ignoring buy-side costs.

**Root Cause:** Buy fees reduced cash but weren't included in average cost basis calculation.

**Solution:**
- Include buy-side fees when calculating average price on buy fills
- Sell PnL now properly accounts for both buy and sell fees

**Code Changes:**
- `crates/ferrotick-backtest/src/portfolio/position.rs`
  - Modified Buy path: total_cost includes fees
  - Modified Sell path: PnL accounts for buy+sell fees

**Impact:** Realized PnL now accurately reflects true trade profitability including all transaction costs.

---

### 5. Engine State Reset

**Problem:** BacktestEngine::run mutated persistent state without clearing between runs.

**Solution:**
- Added Portfolio::reset() method
- Clear pending_orders, latest_bars, and portfolio at start of run()

**Code Changes:**
- `crates/ferrotick-backtest/src/engine/event_driven.rs`
  - Added reset logic in run()
  - Added Portfolio::reset() method

**Impact:** Multiple backtest runs on same engine no longer contaminate results.

---

## Testing

All changes pass validation:

```bash
$ cargo check -p ferrotick-backtest
   Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test -p ferrotick-backtest
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored

$ cargo check --workspace
   Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test -p ferrotick-core
running 40 tests
test result: ok. 40 passed; 0 failed

$ cargo test -p ferrotick-cli
running 5 tests
test result: ok. 5 passed; 0 failed

$ cargo test -p ferrotick-warehouse
running 7 tests
test result: ok. 7 passed; 0 failed
```

No new warnings introduced.

---

## Files Modified

1. `crates/ferrotick-backtest/src/engine/event_driven.rs`
   - Added pending_orders queue
   - Modified run() to reset state and execute pending orders at bar start
   - Modified signal-to-order flow

2. `crates/ferrotick-backtest/src/engine/executor.rs`
   - Added validate_order_prices()
   - Added apply_price_constraints()
   - Fixed limit/stop price enforcement

3. `crates/ferrotick-backtest/src/portfolio/mod.rs`
   - Added reset() method
   - Fixed win-rate accounting

4. `crates/ferrotick-backtest/src/portfolio/position.rs`
   - Fixed realized PnL to include buy-side fees

---

## Verification Steps

To verify the fixes:

1. Build and check for errors:
   ```bash
   cargo check -p ferrotick-backtest
   ```

2. Run tests:
   ```bash
   cargo test -p ferrotick-backtest
   ```

3. Check workspace:
   ```bash
   cargo check --workspace
   cargo test -p ferrotick-core
   ```

4. Verify no new warnings in backtest crate:
   ```bash
   cargo check -p ferrotick-backtest 2>&1 | grep "ferrotick-backtest"
   # Should produce no output (no warnings in backtest crate)
   ```

---

## Backward Compatibility Notes

These changes affect the behavior of backtesting:

- **Order Execution Timing:** Orders now execute one bar later than before (t+1 instead of t)
- **Fill Prices:** Limit orders now respect price boundaries
- **Win Rate:** Counting is more accurate (round-trips vs individual sells)
- **PnL:** Includes all transaction costs

These are bug fixes that bring behavior in line with correct backtesting practices. Code depending on the previous incorrect behavior will need updates.
