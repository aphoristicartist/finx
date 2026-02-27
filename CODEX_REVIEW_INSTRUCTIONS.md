# Codex Review Instructions

You are reviewing the Phase 8: Backtesting Engine implementation for Ferrotick.

## Your Task
Review the implementation in `crates/ferrotick-backtest/` and create a file called `REVIEW_CODEX.md` with your findings.

## Review Checklist

### 1. Code Quality
- [ ] All files follow Rust best practices
- [ ] Error handling is comprehensive
- [ ] Code is well-documented with comments
- [ ] No obvious bugs or logic errors

### 2. Architecture
- [ ] Event-driven design is properly implemented
- [ ] Portfolio tracking is accurate
- [ ] Order execution simulation is realistic
- [ ] Performance metrics calculations are correct

### 3. Integration
- [ ] Properly uses ferrotick-core types (Bar, Symbol, UtcDateTime)
- [ ] Follows existing ferrotick patterns
- [ ] No circular dependencies

### 4. Completeness
- [ ] All required components are implemented
- [ ] Missing features are documented as TODOs
- [ ] Edge cases are handled

### 5. Testing
- [ ] Identify areas that need unit tests
- [ ] Suggest test cases for critical functions

## Output Format

Create `REVIEW_CODEX.md` with this structure:

```markdown
# Phase 8 Implementation Review (Codex)

## Summary
[Brief overall assessment]

## Strengths
- [What was done well]

## Issues Found

### Critical
- [Issues that must be fixed before merge]

### Important
- [Issues that should be fixed]

### Minor
- [Nice-to-have improvements]

## Missing Features
- [What's missing from the requirements]

## Testing Recommendations
- [What tests should be added]

## Code Examples
[Show specific code that needs fixing]

## Verdict
- [ ] APPROVED - Ready to merge
- [ ] NEEDS FIXES - Requires changes before merge
- [ ] MAJOR REVISION - Significant rework needed

## Detailed Notes
[Your detailed review notes]
```

## Files to Review
- crates/ferrotick-backtest/Cargo.toml
- crates/ferrotick-backtest/src/lib.rs
- crates/ferrotick-backtest/src/error.rs
- crates/ferrotick-backtest/src/engine/mod.rs
- crates/ferrotick-backtest/src/engine/event_driven.rs
- crates/ferrotick-backtest/src/engine/executor.rs
- crates/ferrotick-backtest/src/portfolio/mod.rs
- crates/ferrotick-backtest/src/portfolio/position.rs
- crates/ferrotick-backtest/src/portfolio/order.rs
- crates/ferrotick-backtest/src/portfolio/cash.rs
- crates/ferrotick-backtest/src/metrics/mod.rs
- crates/ferrotick-backtest/src/metrics/returns.rs
- crates/ferrotick-backtest/src/metrics/risk.rs
- crates/ferrotick-backtest/src/metrics/drawdown.rs
- crates/ferrotick-backtest/src/costs/mod.rs
- crates/ferrotick-backtest/src/costs/slippage.rs
- crates/ferrotick-backtest/src/costs/fees.rs

Review thoroughly and provide actionable feedback.
