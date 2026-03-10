# Phase 10 ML Correctness Review

Date: 2026-03-01
Scope: `crates/ferrotick-ml/src/models/*.rs` plus related feature/training/evaluation paths used by Phase 10 ML integration.

## Findings (ordered by severity)

### 1) Critical: label generation is backward-looking, not forward-looking (predictive leakage/misalignment)
Evidence:
- `crates/ferrotick-ml/src/features/mod.rs:167-169` computes labels via `simple_returns(&closes, 1|5|20)`.
- `crates/ferrotick-ml/src/features/transforms.rs:7-13` defines `return_t = (price_t - price_{t-period}) / price_{t-period}`.
- `crates/ferrotick-ml/src/training/dataset.rs:141-145` uses `return_1d/5d/20d` directly as training targets for the same row features.

Impact:
- Targets represent historical returns up to timestamp `t`, not future return after `t`.
- In a predictive trading setup this is not the correct supervised label and can inflate apparent performance.

### 2) High: cross-validation leaks future data for time-series usage
Evidence:
- `crates/ferrotick-ml/src/training/evaluation.rs:100-104` uses `split_*` helpers that build train set as `before + after` around the held-out fold.
- `crates/ferrotick-ml/src/training/evaluation.rs:141-143`, `157-159`, `168-169` explicitly concatenate both past and future segments into training.

Impact:
- For chronological financial data, folds include future observations in training relative to test fold.
- This invalidates time-series model validation (look-ahead bias).

### 3) High: metric computation silently truncates when prediction/label lengths differ
Evidence:
- `crates/ferrotick-ml/src/training/evaluation.rs:21` uses `predictions.iter().zip(labels.iter())`.
- `crates/ferrotick-ml/src/training/evaluation.rs:121-127` does not validate `predictions.len() == labels.len()` before scoring.

Impact:
- Missing/excess predictions are ignored instead of failing fast.
- Reported accuracy/precision/recall/F1 can be wrong without any error.

### 4) Medium: overfitting controls are minimal
Evidence:
- `crates/ferrotick-ml/src/models/decision_tree.rs:15-19` stores only optional `max_depth`.
- `crates/ferrotick-ml/src/models/decision_tree.rs:31-34` only applies `max_depth`; no exposed controls for min samples/leaf/impurity.
- Default `DecisionTreeClassifier::default()` uses `None` (unbounded depth) at `crates/ferrotick-ml/src/models/decision_tree.rs:71-74`.

Impact:
- High variance risk, especially on noisy market data.

### 5) Medium: normalization exists but is not leakage-safe by design
Evidence:
- `crates/ferrotick-ml/src/training/dataset.rs:75-95` normalizes entire dataset in-place.
- No scaler fit/apply split abstraction to ensure train-only statistics are used for validation/test.

Impact:
- Easy to introduce data leakage by normalizing before split/CV.

### 6) Medium: model persistence is missing for SVM/DecisionTree
Evidence:
- `crates/ferrotick-ml/src/models/svm.rs` and `crates/ferrotick-ml/src/models/decision_tree.rs` contain no `save/load` API.
- `crates/ferrotick-ml/src/models/traits.rs` has no persistence methods.
- Existing persistence is for features only (`crates/ferrotick-ml/src/features/store.rs`).

Impact:
- Trained models cannot be reliably saved/restored for deployment or reproducible inference.

### 7) Low: `feature_importance` is a placeholder and not actual model-derived importance
Evidence:
- `crates/ferrotick-ml/src/models/decision_tree.rs:63-67` returns uniform weights for 12 features.

Impact:
- Can mislead downstream interpretation/evaluation.

### 8) Low: single-row `predict` methods allocate dummy labels with wrong semantic length
Evidence:
- `crates/ferrotick-ml/src/models/svm.rs:44` and `crates/ferrotick-ml/src/models/decision_tree.rs:52` use `Array1::from_elem(features.len(), false)` for a single-sample prediction path.

Impact:
- Works with current Linfa prediction traits (targets are regenerated from record rows), but this is misleading and error-prone.

### 9) Low: hyperparameter validation gaps
Evidence:
- `crates/ferrotick-ml/src/models/decision_tree.rs:15` accepts `max_depth: Option<usize>` without local validation (e.g., `Some(0)`).
- SVM wrapper exposes no configurable hyperparameters beyond fixed class weights (`crates/ferrotick-ml/src/models/svm.rs:27-29`).

Impact:
- Weak guardrails for invalid/degenerate configuration and weak tunability.

## Requested Verification Status

| Item | Status | Notes |
|---|---|---|
| SVM implementation is correct | PARTIAL | Core Linfa integration trains/predicts successfully; correctness risks remain around label definition and validation workflow. |
| Decision Tree implementation is correct | PARTIAL | Core Linfa integration works; overfitting controls and feature importance are weak/placeholder. |
| Training converges properly | PARTIAL | Training succeeds on current tests; no explicit convergence diagnostics or robust failure checks in wrapper layer. |
| Predictions are accurate | PASS (synthetic), PARTIAL (production realism) | Synthetic tests pass strongly; real-world predictive validity is undermined by label/time-series validation issues above. |
| Cross-validation is implemented correctly | FAIL (for time-series) | Current k-fold implementation leaks future data and lacks length checks on predictions. |
| Overfitting prevention | PARTIAL | Only optional `max_depth`; defaults and exposed controls are insufficient for robust regularization. |
| Feature normalization | PARTIAL | In-place normalization exists, but no train-only fit/apply normalization pipeline. |
| Hyperparameter validation | PARTIAL | Some library-level checks exist, but wrapper-level validation/tunability is limited. |
| Model persistence | FAIL | No model save/load path for SVM/DecisionTree. |
| Feature extraction works | PASS | Indicator/feature pipeline is implemented and tested; Phase 7 behavioral tests pass. |
| Label generation is correct | FAIL (for predictive labeling) | Uses backward returns as targets instead of future-horizon labels. |
| Model evaluation metrics are correct | PARTIAL | Metric formulas are standard, but silent truncation on length mismatch can invalidate scores. |

## Test Evidence

Executed:
- `cargo test -p ferrotick-ml`

Observed:
- All current ML crate tests passed (`24 passed, 0 failed`).
- Coverage does not currently include:
  - `cross_validate` temporal correctness and prediction-length invariants.
  - model persistence save/load behavior.
  - forward-label alignment tests.
