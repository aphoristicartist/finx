# Ferrotick Fix Plan - Critical Issues Remediation

**Created:** 2026-03-01
**Priority:** Fix all CRITICAL issues blocking production
**Timeline:** 90 minutes per batch

---

## IMMEDIATE BLOCKER (Fix First)

### Issue 0: Compilation Error
**File:** `crates/ferrotick-strategies/tests/strategies_test.rs:830`
**Problem:** Unclosed delimiter
**Impact:** Cannot compile/test strategies crate
**Fix Time:** 2 minutes

---

## BATCH 1: Critical Trading Logic Fixes (30 minutes)

### Fix 1: Backtest Capital Reset
**Severity:** CRITICAL
**File:** `crates/ferrotick-backtest/src/portfolio/mod.rs:43-45`
**Problem:** Portfolio capital resets to 0.0
**Impact:** All backtesting results incorrect
**Fix:**
```rust
// BEFORE
pub fn new(initial_capital: f64) -> Self {
    Self {
        cash: 0.0,  // ❌ WRONG
        ...
    }
}

// AFTER
pub fn new(initial_capital: f64) -> Self {
    Self {
        cash: initial_capital,  // ✅ CORRECT
        ...
    }
}
```

### Fix 2: Stale Bar Execution
**Severity:** CRITICAL
**File:** `crates/ferrotick-backtest/src/engine/event_driven.rs:160-171`
**Problem:** Orders execute on stale bar data (temporal misalignment)
**Impact:** Look-ahead bias, incorrect fills
**Fix:**
```rust
// BEFORE: Execute on current bar (stale)
engine.execute_order(&bar);

// AFTER: Execute on next bar (correct)
pending_orders.push(order);
// Apply on NEXT bar iteration
```

### Fix 3: Signal-to-Order Routing
**Severity:** CRITICAL
**File:** `crates/ferrotick-strategies/src/signals/generator.rs:28`
**Problem:** One strategy's signal routes to ALL strategies
**Impact:** Incorrect order generation
**Fix:**
```rust
// BEFORE
signals.iter().for_each(|s| {
    ALL_STRATEGIES.generate_order(s)  // ❌ Routes to all
});

// AFTER
signals.iter().for_each(|s| {
    s.source_strategy.generate_order(s)  // ✅ Routes to source only
});
```

### Fix 4: ML Label Generation
**Severity:** CRITICAL
**File:** `crates/ferrotick-ml/src/features/mod.rs:167-169`
**Problem:** Labels are backward-looking, not predictive
**Impact:** ML models trained on wrong labels
**Fix:**
```rust
// BEFORE: Label based on past
let label = bars[i].close > bars[i-1].close;  // ❌

// AFTER: Label based on future (what we want to predict)
let label = bars[i+1].close > bars[i].close;  // ✅
```

### Fix 5: Circuit Breaker Bypass
**Severity:** CRITICAL
**File:** `crates/ferrotick-core/src/adapters/yahoo.rs:405-415`
**Problem:** Circuit breaker bypassed in Yahoo auth paths
**Impact:** Rate limiting ineffective
**Fix:**
```rust
// BEFORE
async fn fetch(&self) {
    self.client.get(url).send().await  // ❌ No breaker
}

// AFTER
async fn fetch(&self) {
    self.circuit_breaker.call(|| {
        self.client.get(url).send()
    }).await  // ✅ Through breaker
}
```

### Fix 6: Walk-Forward Infinite Loop
**Severity:** CRITICAL
**File:** `crates/ferrotick-optimization/src/walk_forward.rs:79,105,121,153`
**Problem:** Loop doesn't advance, hangs indefinitely
**Impact:** Optimization never completes
**Fix:**
```rust
// BEFORE
loop {
    let window = &data[start..end];
    // Missing: start += step
}

// AFTER
loop {
    let window = &data[start..end];
    start += step;  // ✅ Advance window
    if start >= data.len() { break; }
}
```

---

## BATCH 2: High Priority Fixes (30 minutes)

### Fix 7: Pending Fills Not Applied
**File:** `crates/ferrotick-backtest/src/engine/event_driven.rs:190-208`
**Fix:** Apply all pending fills before generating report

### Fix 8: Vectorized Ignores Fees/Slippage
**File:** `crates/ferrotick-backtest/src/vectorized/engine.rs:100-103`
**Fix:** Add transaction cost calculations

### Fix 9: Position Sizing Ignored
**File:** `crates/ferrotick-strategies/src/dsl/mod.rs:41,50,59,67`
**Fix:** Wire position sizing to order generation

### Fix 10: Yahoo Auth Cache
**File:** `crates/ferrotick-core/src/adapters/yahoo.rs:57-64`
**Fix:** Fix cache validity check and storage

### Fix 11: Data Caching Not Wired
**File:** `crates/ferrotick-core/src/cache.rs`
**Fix:** Wire cache into provider paths

### Fix 12: Web Backtest Stub
**File:** `crates/ferrotick-web/src/routes/backtest.rs:8`
**Fix:** Implement real backtest endpoint

---

## BATCH 3: Integration & Architecture Fixes (30 minutes)

### Fix 13: Strategy-Backtest Interface Mismatch
**File:** `crates/ferrotick-strategies/src/traits.rs`
**Fix:** Create adapter layer between strategy trait and backtest engine

### Fix 14: Options Pricing No Black-Scholes
**File:** `crates/ferrotick-core/src/assets/options.rs:30,44,52`
**Fix:** Implement actual Black-Scholes formula

### Fix 15: Vectorized/Event-Driven Equivalence
**File:** `crates/ferrotick-backtest/src/vectorized/engine.rs:204`
**Fix:** Ensure both paths produce identical results

### Fix 16: Model Persistence
**File:** `crates/ferrotick-ml/src/models/svm.rs`, `decision_tree.rs`
**Fix:** Add save/load methods for trained models

### Fix 17: Cross-Validation Time-Series
**File:** `crates/ferrotick-ml/src/evaluation.rs:100-104`
**Fix:** Use time-series safe cross-validation (no random shuffle)

### Fix 18: Paper Trading Multi-Symbol
**File:** `crates/ferrotick-trading/src/paper/engine.rs:27`
**Fix:** Handle multiple symbols correctly

---

## EXECUTION ORDER

**Phase 1: Immediate Blocker (2 min)**
- [ ] Fix unclosed delimiter in strategies_test.rs

**Phase 2: Batch 1 - Critical Fixes (30 min)**
- [ ] Fix 1: Backtest capital reset
- [ ] Fix 2: Stale bar execution
- [ ] Fix 3: Signal-to-order routing
- [ ] Fix 4: ML label generation
- [ ] Fix 5: Circuit breaker bypass
- [ ] Fix 6: Walk-forward infinite loop

**Phase 3: Batch 2 - High Priority (30 min)**
- [ ] Fix 7: Pending fills
- [ ] Fix 8: Vectorized fees
- [ ] Fix 9: Position sizing
- [ ] Fix 10: Auth cache
- [ ] Fix 11: Data caching
- [ ] Fix 12: Web backtest

**Phase 4: Batch 3 - Integration (30 min)**
- [ ] Fix 13: Strategy interface
- [ ] Fix 14: Black-Scholes
- [ ] Fix 15: Vectorized equivalence
- [ ] Fix 16: Model persistence
- [ ] Fix 17: Time-series CV
- [ ] Fix 18: Paper trading

**Phase 5: Validation (15 min)**
- [ ] Run cargo test --workspace
- [ ] Run cargo clippy --workspace
- [ ] Verify all tests pass
- [ ] Generate new comprehensive review

---

## SUCCESS CRITERIA

✅ All 12 CRITICAL issues fixed
✅ All 15 HIGH priority issues fixed
✅ cargo test --workspace passes
✅ cargo clippy --workspace passes (warnings only)
✅ Grade improved from D to B+ or higher
✅ Release readiness changed to YES

---

## VALIDATION AFTER FIXES

```bash
# 1. Fix compilation
cargo build --workspace

# 2. Run all tests
cargo test --workspace

# 3. Check clippy
cargo clippy --workspace

# 4. Re-run comprehensive review
codex --model gpt-5.3-codex --full-auto "Re-run comprehensive review after fixes"
```

---

**Plan Created:** 2026-03-01
**Estimated Time:** 2 hours
**Expected Grade:** B+ → A
**Expected Release Readiness:** YES
