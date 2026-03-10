use super::Dataset;
use crate::{MlError, MlResult};

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

/// Perform forward-chaining time-series cross-validation.
pub fn time_series_cross_validate<F>(
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
    let fold_size = total_rows / (k + 1);

    if fold_size == 0 {
        return Err(MlError::NoData(String::from(
            "insufficient data for cross-validation",
        )));
    }

    let mut metrics = Vec::with_capacity(k);

    for fold in 0..k {
        // Walk-forward split: train is always before test.
        let test_start = (fold + 1) * fold_size;
        let test_end = if fold == k - 1 {
            total_rows
        } else {
            test_start + fold_size
        };

        // Split into train and test
        let (train_features, test_features) =
            split_features(&dataset.features, test_start, test_end);
        let (train_targets, test_targets) = split_targets(&dataset.targets, test_start, test_end);
        let (train_timestamps, test_timestamps) =
            split_timestamps(&dataset.timestamps, test_start, test_end);

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

        // Convert test targets to boolean (clone to avoid borrow issues)
        let labels: Vec<bool> = test_dataset.targets.iter().map(|&t| t > 0.0).collect();

        // Calculate metrics
        let fold_metrics = ModelMetrics::from_predictions(&predictions, &labels);
        metrics.push(fold_metrics);
    }

    Ok(metrics)
}

fn split_features(
    features: &ndarray::Array2<f64>,
    start: usize,
    end: usize,
) -> (ndarray::Array2<f64>, ndarray::Array2<f64>) {
    use ndarray::s;

    let train = features.slice(s![..start, ..]).to_owned();
    let test = features.slice(s![start..end, ..]).to_owned();

    (train, test)
}

fn split_targets(
    targets: &ndarray::Array1<f64>,
    start: usize,
    end: usize,
) -> (ndarray::Array1<f64>, ndarray::Array1<f64>) {
    use ndarray::s;

    let train = targets.slice(s![..start]).to_owned();
    let test = targets.slice(s![start..end]).to_owned();

    (train, test)
}

fn split_timestamps(timestamps: &[String], start: usize, end: usize) -> (Vec<String>, Vec<String>) {
    let train = timestamps[..start].to_vec();
    let test = timestamps[start..end].to_vec();

    (train, test)
}
