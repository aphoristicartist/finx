# Goose Implementation Instructions

You are implementing Phase 8: Backtesting Engine for Ferrotick.

## Your Task
Read PLAN.md and create ALL the files specified in it. The plan has complete code for 17 files.

## Step-by-Step Instructions

1. Read PLAN.md completely - it contains all the code you need to write
2. Create the directory structure: `crates/ferrotick-backtest/src/{engine,portfolio,metrics,costs}/`
3. Create each file in the order specified in PLAN.md
4. Update the workspace Cargo.toml to include the new crate

## Files to Create (from PLAN.md)

### Step 1: `crates/ferrotick-backtest/Cargo.toml`
### Step 2: `crates/ferrotick-backtest/src/lib.rs`
### Step 3: `crates/ferrotick-backtest/src/error.rs`
### Step 4: `crates/ferrotick-backtest/src/engine/mod.rs`
### Step 5: `crates/ferrotick-backtest/src/engine/event_driven.rs`
### Step 6: `crates/ferrotick-backtest/src/engine/executor.rs`
### Step 7: `crates/ferrotick-backtest/src/portfolio/mod.rs`
### Step 8: `crates/ferrotick-backtest/src/portfolio/position.rs`
### Step 9: `crates/ferrotick-backtest/src/portfolio/order.rs`
### Step 10: `crates/ferrotick-backtest/src/portfolio/cash.rs`
### Step 11: `crates/ferrotick-backtest/src/metrics/mod.rs`
### Step 12: `crates/ferrotick-backtest/src/metrics/returns.rs`
### Step 13: `crates/ferrotick-backtest/src/metrics/risk.rs`
### Step 14: `crates/ferrotick-backtest/src/metrics/drawdown.rs`
### Step 15: `crates/ferrotick-backtest/src/costs/mod.rs`
### Step 16: `crates/ferrotick-backtest/src/costs/slippage.rs`
### Step 17: `crates/ferrotick-backtest/src/costs/fees.rs`

## After Creating Files

1. Update `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/Cargo.toml` to add:
```toml
members = [
  "crates/ferrotick-core",
  "crates/ferrotick-cli",
  "crates/ferrotick-warehouse",
  "crates/ferrotick-agent",
  "crates/ferrotick-ml",
  "crates/ferrotick-backtest",  # ADD THIS LINE
]
```

2. Run `cargo check -p ferrotick-backtest` to verify compilation
3. Fix any errors that arise

## Important Notes

- Use EXACT code from PLAN.md - do not modify it
- The plan uses ferrotick-core types (Symbol, UtcDateTime, Bar)
- All error handling uses thiserror
- Follow the existing patterns in ferrotick-ml and ferrotick-core

## Validation

After all files are created, run:
```bash
cargo check -p ferrotick-backtest
```

If there are errors, read them carefully and fix the issues.
