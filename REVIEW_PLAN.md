# Plan Review - Phase 9: Strategy Library

## Clarity Score
9/10
The plan is exceptionally clear, providing exact file paths, explicit actions (create/modify), and complete code blocks for nearly every step. It leaves almost nothing to guesswork, which is ideal for an AI coding assistant or a junior developer to follow literally.

## Completeness
- ✅ **What's covered well**: Workspace setup, error handling, trait definitions, all four required strategies, the YAML parsing/validation pipeline, and complete CLI integration (including the backtest adapter).
- ❌ **What's missing**: 
  - There is a slight performance gap in the CLI adapter (`StrategyBacktestAdapter`) where `latest_atr` is maintained. It computes ATR on the whole `bars_for_atr` history on *every single bar* (an O(N^2) operation).
  - In Step 23, the text instructs to add "one test for parser failure" but the provided code snippet omits this test.

## Ambiguity Flags
1. **Step 9 (`library/ma_crossover.rs`)**: Uses `.expect("checked len")` for SMA calculations in `on_bar`. While the length check is immediately prior, using `expect()` in production code might make a junior developer pause.
2. **Step 21 (`commands/strategy.rs`)**: Inside `StrategyBacktestAdapter::on_bar`, `compute_atr(&self.bars_for_atr, atr_period)` is called on every new bar, passing the ever-growing vector of all historical bars. The plan doesn't specify if `bars_for_atr` should be truncated to prevent memory/CPU bloat over thousands of bars.
3. **Step 23 (Smoke tests)**: The instructions say "Add at least one test for run_validate success path and one for parser failure", but the provided code block only includes the success path test. The implementer will have to guess how to write the failure test.

## Pattern Consistency
The plan strictly adheres to the established codebase patterns. It leverages `thiserror` correctly, uses existing `ferrotick-ml` indicators instead of pulling new dependencies, mimics the CLI command dispatch pattern accurately, and handles errors via `CliError::Command`. The separation of parser, validator, and compiler is an excellent architectural choice that aligns well with idiomatic Rust.

## Recommendations
1. **Optimize ATR Calculation (Step 21)**: Instruct the developer to truncate the `bars_for_atr` vector (e.g., keeping only the last `atr_period * 2` bars) in `StrategyBacktestAdapter::on_bar` to avoid quadratic scaling during backtests.
2. **Provide the Missing Test (Step 23)**: Explicitly include the code for the parser failure test so the implementer doesn't have to invent it, ensuring it stays local and deterministic.
3. **Safe Unwrapping (Step 9)**: Consider replacing `.expect()` with a safe fallback or `?` in `ma_crossover.rs` to guarantee no panics during runtime execution.

## Verdict
APPROVED - The plan is thoroughly detailed and ready for implementation. The implementer should note the minor optimization and testing recommendations above, but otherwise can execute the steps verbatim.