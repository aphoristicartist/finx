# Multi-Tool Fix Strategy: Codex + Gemini + Goose

**Objective:** Fix all 47 critical issues using all three tools in combination

---

## TOOL ROLES

### 1. **Codex CLI** (Implementation)
- **Model:** gpt-5.3-codex (REQUIRED)
- **Role:** Deep code analysis + implementation
- **Strengths:** Understanding code, complex refactors, new features

### 2. **Gemini CLI** (Review)
- **Model:** gemini-3.1-pro (REQUIRED)
- **Role:** Peer review + validation
- **Strengths:** Catching edge cases, architecture review, cross-validation

### 3. **Goose** (Quick Fixes)
- **Model:** Local Qwen via LM Studio
- **Role:** Rapid iterations + local fixes
- **Strengths:** Fast, no API costs, good for simple changes

---

## WORKFLOW

```
┌─────────────┐
│   Codex     │ → Implement fix
│ (Implement) │ → Run tests
└──────┬──────┘ → Verify basic functionality
       │
       ▼
┌─────────────┐
│   Gemini    │ → Review Codex implementation
│  (Review)   │ → Check for edge cases
└──────┬──────┘ → Validate architecture
       │
       ▼
┌─────────────┐
│   Goose     │ → Fix issues from review
│   (Polish)  │ → Quick iterations
└──────┬──────┘ → Final validation
       │
       ▼
   ✅ Complete
```

---

## CURRENT STATUS

### Batch 1: Backtest + Strategy (Agent Running)
**Tool:** Codex CLI
**Status:** 🔄 Running (1 min in, 60 min timeout)

**Fixes:**
1. ⏳ Stale bar execution
2. ⏳ Pending fills not applied
3. ⏳ Vectorized ignores fees/slippage
4. ⏳ Vectorized same-bar execution
5. ⏳ Signal-to-order routing
6. ⏳ Position sizing ignored

**Next Step:** Gemini review after Codex completes

---

### Batch 2: ML + Data Fetching (Agent Running)
**Tool:** Codex CLI
**Status:** 🔄 Running (1 min in, 60 min timeout)

**Fixes:**
1. ⏳ ML label generation
2. ⏳ Cross-validation data leakage
3. ⏳ Model persistence
4. ⏳ Circuit breaker bypass
5. ⏳ Circuit breaker stuck open
6. ⏳ Yahoo auth cache
7. ⏳ Data caching not wired

**Next Step:** Gemini review after Codex completes

---

### Batch 3: Phases 11-17 + Integration (Queued)
**Status:** ⏳ Waiting for agent slot

**Will use:** Codex + Gemini + Goose

**Fixes:**
1. ⏳ Walk-forward infinite loop
2. ⏳ Web backtest stub
3. ⏳ Options pricing (Black-Scholes)
4. ⏳ Paper trading multi-symbol
5. ⏳ Live trading unimplemented
6. ⏳ Strategy-backtest interface
7. ⏳ Optimization error swallowing

---

## POST-CODEX: GEMINI REVIEW

After each batch completes, use Gemini for peer review:

```bash
# After Batch 1 (Backtest + Strategy)
gemini --model gemini-3.1-pro "
Review the backtest and strategy fixes just implemented:

1. Read all modified files in:
   - crates/ferrotick-backtest/src/
   - crates/ferrotick-strategies/src/

2. Verify:
   - Temporal alignment is correct (no look-ahead bias)
   - Transaction costs are reasonable
   - Signal routing works correctly
   - Position sizing is wired properly

3. Check for:
   - Edge cases not handled
   - Race conditions
   - Off-by-one errors
   - Mathematical correctness

4. Generate: BATCH1_REVIEW.md with findings

DO NOT read documentation. ONLY review code.
"

# After Batch 2 (ML + Data)
gemini --model gemini-3.1-pro "
Review the ML and data fetching fixes:

1. Read all modified files in:
   - crates/ferrotick-ml/src/
   - crates/ferrotick-core/src/

2. Verify:
   - Labels are predictive (not backward-looking)
   - Cross-validation is time-series safe
   - Circuit breaker logic is correct
   - Caching is properly implemented

3. Check for:
   - Data leakage in ML
   - Cache invalidation issues
   - Circuit breaker edge cases

4. Generate: BATCH2_REVIEW.md with findings

Code-only review.
"

# After Batch 3 (Phases 11-17)
gemini --model gemini-3.1-pro "
Review phases 11-17 fixes:

1. Read all modified files in:
   - crates/ferrotick-optimization/src/
   - crates/ferrotick-web/src/
   - crates/ferrotick-core/src/assets/
   - crates/ferrotick-trading/src/

2. Verify:
   - Walk-forward terminates correctly
   - Web backtest runs real backtests
   - Black-Scholes implementation is correct
   - Paper trading handles multi-symbol

3. Check for:
   - Infinite loops
   - Stub implementations
   - Mathematical correctness (options pricing)

4. Generate: BATCH3_REVIEW.md with findings

Code-only.
"
```

---

## POST-GEMINI: GOOSE POLISH

Use Goose to fix any issues found by Gemini:

```bash
# Quick fixes based on Gemini review
goose -i "
Based on BATCH1_REVIEW.md:
1. Fix any edge cases identified
2. Address race conditions
3. Correct off-by-one errors
4. Run: cargo test -p ferrotick-backtest
5. Run: cargo test -p ferrotick-strategies
"

goose -i "
Based on BATCH2_REVIEW.md:
1. Fix data leakage issues
2. Correct cache invalidation
3. Handle circuit breaker edge cases
4. Run: cargo test -p ferrotick-ml
5. Run: cargo test -p ferrotick-core
"

goose -i "
Based on BATCH3_REVIEW.md:
1. Fix infinite loop edge cases
2. Complete stub implementations
3. Correct Black-Scholes math
4. Run: cargo test --workspace
"
```

---

## FINAL VALIDATION

After all three tools complete:

```bash
# 1. Build
cargo build --workspace

# 2. Test
cargo test --workspace

# 3. Clippy
cargo clippy --workspace

# 4. Final Gemini review
gemini --model gemini-3.1-pro "
Final comprehensive review of all fixes:

1. Read all modified files
2. Verify all 20 critical fixes are implemented correctly
3. Check for any remaining issues
4. Validate test coverage
5. Generate: FINAL_REVIEW.md

Grade the overall fix quality (A-F)
Determine if ready for v1.0.0 release
"

# 5. Goose final polish
goose -i "
Based on FINAL_REVIEW.md:
1. Fix any remaining issues
2. Ensure all tests pass
3. Run: cargo test --workspace
4. Verify: cargo clippy --workspace
"
```

---

## SUCCESS CRITERIA

✅ **Codex:** All 20 fixes implemented
✅ **Gemini:** All fixes reviewed and validated
✅ **Goose:** All review issues addressed
✅ **Tests:** 100% passing (0 failures)
✅ **Clippy:** No errors (warnings only)
✅ **Grade:** A- or higher
✅ **Release:** Ready for v1.0.0

---

## TOOL SYNERGY

**Codex** → Deep understanding + complex implementation
**Gemini** → Independent review + edge case detection
**Goose** → Fast iterations + local validation

**Together:** Comprehensive fix coverage from multiple perspectives

---

## TIMELINE

**+60 min:** Batches 1 & 2 complete (Codex)
**+70 min:** Gemini reviews complete
**+80 min:** Goose polish complete
**+90 min:** Batch 3 starts
**+150 min:** Batch 3 complete
**+160 min:** Final Gemini review
**+170 min:** Final Goose polish
**+180 min:** All fixes complete

---

**Created:** 2026-03-02 16:35 EST
**Status:** Codex agents running, Gemini + Goose queued
