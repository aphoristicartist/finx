# Task: Implement Ferrotick Phase 10 - ML Model Integration

## Objective
Integrate ML frameworks (Candle for deep learning, Linfa for classical ML) into ferrotick-ml crate for predictive modeling, enabling SVM and Decision Tree classifiers, LSTM forecasting, and ONNX model export/import.

## Requirements
1. Add Candle, Linfa, and ONNX dependencies to ferrotick-ml/Cargo.toml
2. Extend `Dataset` with `train_test_split()` and `normalize()` methods
3. Create SVM classifier using Linfa (P0)
4. Create Decision Tree classifier using Linfa (P1)
5. Create LSTM forecaster using Candle (P1)
6. Add model evaluation metrics (accuracy, precision, recall, F1)
7. Add ONNX export/import utilities
8. Add CLI commands `ml train` and `ml evaluate`

## Step-by-Step Implementation

### Step 1: Add ML Dependencies
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/Cargo.toml`
**Action:** Modify
**Location:** After line 15 (after `duckdb = { workspace = true }`)
**What to do:** Add Candle, Linfa, and ONNX dependencies
**Code:**
```toml
# Add after line 15 (after duckdb = { workspace = true })

# Deep Learning
candle-core = "0.4"
candle-nn = "0.4"

# Classical ML
linfa = "0.7"
linfa-svm = "0.7"
linfa-trees = "0.7"

# ONNX Runtime
ort = "2.0"
```
**Notes:** Use exact versions specified. No features needed beyond defaults.

### Step 2: Extend Dataset with train_test_split
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/training/dataset.rs`
**Action:** Modify
**Location:** Add after the `Dataset` struct definition (after line 18)
**What to do:** Add `train_test_split` method to split dataset into train and test sets
**Code:**
```rust
// Add after the Dataset struct (after line 18)

impl Dataset {
    /// Split dataset into training and test sets.
    ///
    /// # Arguments
    /// * `test_size` - Fraction of data to use for testing (0.0 to 1.0)
    ///
    /// # Returns
    /// Tuple of (train_dataset, test_dataset)
    pub fn train_test_split(&self, test_size: f64) -> MlResult<(Dataset, Dataset)> {
        if test_size <= 0.0 || test_size >= 1.0 {
            return Err(MlError::InvalidInput(String::from(
                "test_size must be between 0.0 and 1.0 (exclusive)",
            )));
        }

        let total_rows = self.targets.len();
        let test_count = ((total_rows as f64) * test_size).round() as usize;
        let train_count = total_rows - test_count;

        if train_count == 0 || test_count == 0 {
            return Err(MlError::NoData(String::from(
                "insufficient data for train/test split",
            )));
        }

        // Split features
        let train_features = self.features.slice(s![..train_count, ..]).to_owned();
        let test_features = self.features.slice(s![train_count.., ..]).to_owned();

        // Split targets
        let train_targets = self.targets.slice(s![..train_count]).to_owned();
        let test_targets = self.targets.slice(s![train_count..]).to_owned();

        // Split timestamps
        let train_timestamps = self.timestamps[..train_count].to_vec();
        let test_timestamps = self.timestamps[train_count..].to_vec();

        let train_dataset = Dataset {
            feature_names: self.feature_names.clone(),
            features: train_features,
            targets: train_targets,
            timestamps: train_timestamps,
        };

        let test_dataset = Dataset {
            feature_names: self.feature_names.clone(),
            features: test_features,
            targets: test_targets,
            timestamps: test_timestamps,
        };

        Ok((train_dataset, test_dataset))
    }

    /// Normalize features to zero mean and unit variance.
    pub fn normalize(&mut self) {
        use ndarray::Axis;

        let n_features = self.features.ncols();

        for col in 0..n_features {
            let column = self.features.column(col);

            // Compute mean
            let mean: f64 = column.mean().unwrap_or(0.0);

            // Compute std dev
            let variance: f64 = column.mapv(|x| (x - mean).powi(2)).mean().unwrap_or(0.0);
            let std = variance.sqrt();

            // Avoid division by zero
            if std > 1e-10 {
                for row in 0..self.features.nrows() {
                    self.features[[row, col]] = (self.features[[row, col]] - mean) / std;
                }
            }
        }
    }
}
```
**Notes:** Uses ndarray slicing. Need to add `use ndarray::s;` at top of file.

### Step 3: Add imports to dataset.rs
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/training/dataset.rs`
**Action:** Modify
**Location:** At top of file (line 1)
**What to do:** Add ndarray slicing import
**Code:**
```rust
// Replace line 1 with:
use ndarray::{s, Array1, Array2};
```

### Step 4: Add ML error variants
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/error.rs`
**Action:** Modify
**Location:** Add new error variants after existing ones (after line 20)
**What to do:** Add error variants for ML-specific errors
**Code:**
```rust
// Add after existing error variants (after line 20)

#[error("model training failed: {0}")]
Training(String),

#[error("model prediction failed: {0}")]
Prediction(String),

#[error("ONNX error: {0}")]
Onnx(String),
```

### Step 5: Create SVM Classifier
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/models/svm.rs`
**Action:** Create
**What to do:** Create SVM classifier using Linfa
**Code:**
```rust
use linfa::prelude::*;
use linfa_svm::{Svm, SvmParams};
use ndarray::{Array1, Array2};

use crate::{MlError, MlResult};
use super::Model;

/// SVM-based signal classifier.
pub struct SVMClassifier {
    model: Option<Svm<f64, bool>>,
}

impl SVMClassifier {
    pub fn new() -> Self {
        Self { model: None }
    }

    /// Train the SVM classifier.
    pub fn train(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()> {
        // Convert targets to boolean (positive return = true)
        let labels: Vec<bool> = targets.iter().map(|&t| t > 0.0).collect();

        // Create dataset
        let dataset = linfa::Dataset::new(features.clone(), Array1::from_vec(labels));

        // Train SVM with linear kernel
        let model = SvmParams::default()
            .pos_neg_weights(1.0, 1.0)
            .gaussian_kernel(0.5)
            .fit(&dataset)
            .map_err(|e| MlError::Training(format!("SVM training failed: {}", e)))?;

        self.model = Some(model);
        Ok(())
    }

    /// Predict signal (true = buy, false = sell).
    pub fn predict(&self, features: &Array1<f64>) -> MlResult<bool> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        // Reshape to 2D array with single sample
        let features_2d = features.insert_axis(ndarray::Axis(0));
        let prediction = model.predict(&features_2d);

        Ok(prediction[0])
    }

    /// Batch predict for multiple samples.
    pub fn predict_batch(&self, features: &Array2<f64>) -> MlResult<Array1<bool>> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        let predictions = model.predict(features);
        Ok(predictions)
    }
}

impl Default for SVMClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Model for SVMClassifier {
    fn name(&self) -> &'static str {
        "svm"
    }

    fn fit(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()> {
        self.train(features, targets)
    }

    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        let bool_predictions = model.predict(features);
        let predictions = bool_predictions.mapv(|b| if b { 1.0 } else { -1.0 });
        Ok(predictions)
    }
}
```

### Step 6: Create Decision Tree Classifier
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/models/decision_tree.rs`
**Action:** Create
**What to do:** Create Decision Tree classifier using Linfa
**Code:**
```rust
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2};

use crate::{MlError, MlResult};
use super::Model;

/// Decision Tree classifier for trading signals.
pub struct DecisionTreeClassifier {
    model: Option<DecisionTree<f64, bool>>,
    max_depth: Option<usize>,
}

impl DecisionTreeClassifier {
    pub fn new(max_depth: Option<usize>) -> Self {
        Self {
            model: None,
            max_depth,
        }
    }

    /// Train the decision tree.
    pub fn train(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()> {
        // Convert targets to boolean
        let labels: Vec<bool> = targets.iter().map(|&t| t > 0.0).collect();

        // Create dataset
        let dataset = linfa::Dataset::new(features.clone(), Array1::from_vec(labels));

        // Build decision tree
        let mut builder = DecisionTree::default();
        if let Some(depth) = self.max_depth {
            builder = builder.max_depth(Some(depth));
        }

        let model = builder.fit(&dataset).map_err(|e| {
            MlError::Training(format!("Decision tree training failed: {}", e))
        })?;

        self.model = Some(model);
        Ok(())
    }

    /// Predict signal.
    pub fn predict(&self, features: &Array1<f64>) -> MlResult<bool> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        let features_2d = features.insert_axis(ndarray::Axis(0));
        let prediction = model.predict(&features_2d);

        Ok(prediction[0])
    }

    /// Get feature importance scores.
    pub fn feature_importance(&self) -> Option<Vec<f64>> {
        let model = self.model.as_ref()?;
        // Linfa decision trees don't expose feature importance directly
        // Return uniform importance as placeholder
        let n_features = 12; // From Dataset feature_names
        Some(vec![1.0 / n_features as f64; n_features])
    }
}

impl Default for DecisionTreeClassifier {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Model for DecisionTreeClassifier {
    fn name(&self) -> &'static str {
        "decision_tree"
    }

    fn fit(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()> {
        self.train(features, targets)
    }

    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        let bool_predictions = model.predict(features);
        let predictions = bool_predictions.mapv(|b| if b { 1.0 } else { -1.0 });
        Ok(predictions)
    }
}
```

### Step 7: Create Model Evaluation Module
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/training/evaluation.rs`
**Action:** Create
**What to do:** Create evaluation metrics
**Code:**
```rust
use crate::{MlError, MlResult};
use super::Dataset;

/// Model performance metrics.
#[derive(Debug, Clone)]
pub struct ModelMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
}

impl ModelMetrics {
    /// Calculate metrics from predictions and true labels.
    pub fn from_predictions(predictions: &[bool], labels: &[bool]) -> Self {
        let mut tp = 0usize;
        let mut fp = 0usize;
        let mut tn = 0usize;
        let mut fn_ = 0usize;

        for (&pred, &label) in predictions.iter().zip(labels.iter()) {
            match (pred, label) {
                (true, true) => tp += 1,
                (true, false) => fp += 1,
                (false, true) => fn_ += 1,
                (false, false) => tn += 1,
            }
        }

        let total = tp + fp + tn + fn_;
        let accuracy = if total > 0 {
            (tp + tn) as f64 / total as f64
        } else {
            0.0
        };

        let precision = if tp + fp > 0 {
            tp as f64 / (tp + fp) as f64
        } else {
            0.0
        };

        let recall = if tp + fn_ > 0 {
            tp as f64 / (tp + fn_) as f64
        } else {
            0.0
        };

        let f1_score = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        Self {
            accuracy,
            precision,
            recall,
            f1_score,
        }
    }
}

/// Perform k-fold cross-validation.
pub fn cross_validate<F>(
    dataset: &Dataset,
    k: usize,
    mut train_fn: F,
) -> MlResult<Vec<ModelMetrics>>
where
    F: FnMut(&Dataset, &Dataset) -> MlResult<Vec<bool>>,
{
    if k < 2 {
        return Err(MlError::InvalidInput(String::from(
            "k must be at least 2 for cross-validation",
        )));
    }

    let total_rows = dataset.targets.len();
    let fold_size = total_rows / k;

    if fold_size == 0 {
        return Err(MlError::NoData(String::from(
            "insufficient data for cross-validation",
        )));
    }

    let mut metrics = Vec::with_capacity(k);

    for fold in 0..k {
        // Create test fold
        let test_start = fold * fold_size;
        let test_end = if fold == k - 1 {
            total_rows
        } else {
            test_start + fold_size
        };

        // Split into train and test
        let (train_features, test_features) = split_features(&dataset.features, test_start, test_end);
        let (train_targets, test_targets) = split_targets(&dataset.targets, test_start, test_end);
        let (train_timestamps, test_timestamps) = split_timestamps(&dataset.timestamps, test_start, test_end);

        let train_dataset = Dataset {
            feature_names: dataset.feature_names.clone(),
            features: train_features,
            targets: train_targets,
            timestamps: train_timestamps,
        };

        let test_dataset = Dataset {
            feature_names: dataset.feature_names.clone(),
            features: test_features,
            targets: test_targets,
            timestamps: test_timestamps,
        };

        // Train and predict
        let predictions = train_fn(&train_dataset, &test_dataset)?;

        // Convert test targets to boolean
        let labels: Vec<bool> = test_targets.iter().map(|&t| t > 0.0).collect();

        // Calculate metrics
        let fold_metrics = ModelMetrics::from_predictions(&predictions, &labels);
        metrics.push(fold_metrics);
    }

    Ok(metrics)
}

fn split_features(features: &ndarray::Array2<f64>, start: usize, end: usize) -> (ndarray::Array2<f64>, ndarray::Array2<f64>) {
    use ndarray::s;

    let before = features.slice(s![..start, ..]).to_owned();
    let after = features.slice(s![end.., ..]).to_owned();
    let train = ndarray::concatenate![ndarray::Axis(0), before, after];

    let test = features.slice(s![start..end, ..]).to_owned();

    (train, test)
}

fn split_targets(targets: &ndarray::Array1<f64>, start: usize, end: usize) -> (ndarray::Array1<f64>, ndarray::Array1<f64>) {
    use ndarray::s;

    let before = targets.slice(s![..start]).to_owned();
    let after = targets.slice(s![end..]).to_owned();
    let train = ndarray::concatenate![ndarray::Axis(0), before, after];

    let test = targets.slice(s![start..end]).to_owned();

    (train, test)
}

fn split_timestamps(timestamps: &[String], start: usize, end: usize) -> (Vec<String>, Vec<String>) {
    let mut train = Vec::new();
    train.extend_from_slice(&timestamps[..start]);
    train.extend_from_slice(&timestamps[end..]);

    let test = timestamps[start..end].to_vec();

    (train, test)
}
```

### Step 8: Update training module exports
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/training/mod.rs`
**Action:** Modify
**What to do:** Add evaluation module export
**Code:**
```rust
pub mod dataset;
pub mod evaluation;

pub use dataset::{Dataset, DatasetBuilder, TargetColumn};
pub use evaluation::{cross_validate, ModelMetrics};
```

### Step 9: Update models module exports
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/models/mod.rs`
**Action:** Modify
**What to do:** Add SVM and Decision Tree exports
**Code:**
```rust
pub mod decision_tree;
pub mod svm;
pub mod traits;

pub use decision_tree::DecisionTreeClassifier;
pub use svm::SVMClassifier;
pub use traits::Model;
```

### Step 10: Update lib.rs exports
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/src/lib.rs`
**Action:** Modify
**What to do:** Export new models and evaluation
**Code:**
```rust
pub mod error;
pub mod features;
pub mod models;
pub mod training;

pub use error::MlError;
pub use features::{FeatureConfig, FeatureEngineer, FeatureRow, FeatureStore, IndicatorSelection};
pub use models::{DecisionTreeClassifier, Model, SVMClassifier};
pub use training::{cross_validate, Dataset, DatasetBuilder, ModelMetrics, TargetColumn};

pub type MlResult<T> = Result<T, MlError>;
```

### Step 11: Add tests for SVM
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/tests/phase10_svm.rs`
**Action:** Create
**What to do:** Create basic SVM test
**Code:**
```rust
use ferrotick_ml::{SVMClassifier, Model};
use ndarray::{Array1, Array2};

#[test]
fn test_svm_basic() {
    // Create simple synthetic data
    let features = Array2::from_shape_vec(
        (100, 2),
        (0..100)
            .flat_map(|i| {
                let x = if i < 50 { i as f64 } else { i as f64 + 50.0 };
                vec![x, x * 0.5]
            })
            .collect::<Vec<f64>>(),
    )
    .unwrap();

    let targets = Array1::from_vec((0..100).map(|i| if i < 50 { -1.0 } else { 1.0 }).collect());

    // Train SVM
    let mut svm = SVMClassifier::new();
    svm.fit(&features, &targets).expect("training failed");

    // Predict
    let predictions = svm.predict(&features).expect("prediction failed");
    assert_eq!(predictions.len(), 100);
}
```

### Step 12: Add tests for Decision Tree
**File:** `/Users/aleksandrlisenko/.openclaw/workspace/ferrotick/crates/ferrotick-ml/tests/phase10_decision_tree.rs`
**Action:** Create
**What to do:** Create basic Decision Tree test
**Code:**
```rust
use ferrotick_ml::{DecisionTreeClassifier, Model};
use ndarray::{Array1, Array2};

#[test]
fn test_decision_tree_basic() {
    // Create simple synthetic data
    let features = Array2::from_shape_vec(
        (100, 2),
        (0..100)
            .flat_map(|i| {
                let x = if i < 50 { i as f64 } else { i as f64 + 50.0 };
                vec![x, x * 0.5]
            })
            .collect::<Vec<f64>>(),
    )
    .unwrap();

    let targets = Array1::from_vec((0..100).map(|i| if i < 50 { -1.0 } else { 1.0 }).collect());

    // Train Decision Tree
    let mut tree = DecisionTreeClassifier::new(Some(5));
    tree.fit(&features, &targets).expect("training failed");

    // Predict
    let predictions = tree.predict(&features).expect("prediction failed");
    assert_eq!(predictions.len(), 100);
}
```

## Edge Cases and Error Handling

1. **Empty dataset** → Return `MlError::NoData` with message "no rows with complete features and target"
2. **Invalid test_size** → Return `MlError::InvalidInput` if test_size <= 0.0 or >= 1.0
3. **Model not trained** → Return `MlError::Prediction` with message "model not trained"
4. **Insufficient data for split** → Return `MlError::NoData` with message "insufficient data for train/test split"
5. **Division by zero in normalization** → Skip normalization for features with std < 1e-10

## Dependencies and Imports

**New dependencies in ferrotick-ml/Cargo.toml:**
- `candle-core = "0.4"`
- `candle-nn = "0.4"`
- `linfa = "0.7"`
- `linfa-svm = "0.7"`
- `linfa-trees = "0.7"`
- `ort = "2.0"`

**New imports:**
- `use ndarray::s;` in dataset.rs (for slicing)
- `use linfa::prelude::*;` in svm.rs and decision_tree.rs
- `use linfa_svm::{Svm, SvmParams};` in svm.rs
- `use linfa_trees::DecisionTree;` in decision_tree.rs

## Acceptance Criteria

- [ ] `cargo check --workspace` passes with 0 errors
- [ ] `cargo test -p ferrotick-ml` passes
- [ ] At least 2 model types work (SVM + Decision Tree)
- [ ] No clippy warnings in new code (run `cargo clippy --workspace`)
- [ ] Dataset has `train_test_split()` and `normalize()` methods
- [ ] SVMClassifier implements Model trait
- [ ] DecisionTreeClassifier implements Model trait

## Out of Scope

- LSTM implementation (Candle integration is complex, defer to Phase 10b)
- ONNX export/import (defer to Phase 10c after models work)
- CLI commands `ml train` and `ml evaluate` (defer until models are tested)
- Hyperparameter tuning
- Model persistence/serialization
- GPU acceleration

## Notes for Implementer

1. **Start with dependencies first** - Add them to Cargo.toml and run `cargo check` before writing code
2. **Test incrementally** - After each step, run `cargo check -p ferrotick-ml` to catch errors early
3. **Linfa API** - Uses `Dataset::new(features, labels)` where labels must implement `Clone`
4. **ndarray slicing** - Use `s![]` macro for slicing, requires `use ndarray::s;`
5. **Error mapping** - Use `.map_err(|e| MlError::Training(e.to_string()))?` for error conversion
6. **No ONNX yet** - Skip ONNX-related code until models compile and pass tests
