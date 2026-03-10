use ndarray::{s, Array1, Array2};

use crate::features::FeatureRow;
use crate::{MlError, MlResult};

#[derive(Debug, Clone, Copy)]
pub enum TargetColumn {
    Return1d,
    Return5d,
    Return20d,
}

pub struct Dataset {
    pub feature_names: Vec<String>,
    pub features: Array2<f64>,
    pub targets: Array1<f64>,
    pub timestamps: Vec<String>,
}

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

pub struct DatasetBuilder;

impl Default for DatasetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DatasetBuilder {
    pub const fn new() -> Self {
        Self
    }

    pub fn build(&self, rows: &[FeatureRow], target: TargetColumn) -> MlResult<Dataset> {
        let feature_names = vec![
            String::from("rsi"),
            String::from("macd"),
            String::from("macd_signal"),
            String::from("bb_upper"),
            String::from("bb_lower"),
            String::from("atr"),
            String::from("rolling_mean_20"),
            String::from("rolling_std_20"),
            String::from("lag_1"),
            String::from("lag_2"),
            String::from("lag_3"),
            String::from("rolling_momentum"),
        ];

        let mut matrix: Vec<f64> = Vec::new();
        let mut targets: Vec<f64> = Vec::new();
        let mut timestamps: Vec<String> = Vec::new();

        for row in rows {
            let feature_values = [
                row.rsi,
                row.macd,
                row.macd_signal,
                row.bb_upper,
                row.bb_lower,
                row.atr,
                row.rolling_mean_20,
                row.rolling_std_20,
                row.lag_1,
                row.lag_2,
                row.lag_3,
                row.rolling_momentum,
            ];

            let target_value = match target {
                TargetColumn::Return1d => row.return_1d,
                TargetColumn::Return5d => row.return_5d,
                TargetColumn::Return20d => row.return_20d,
            };

            if let Some(target_value) = target_value {
                if feature_values.iter().all(Option::is_some) {
                    for value in feature_values.iter().flatten() {
                        matrix.push(*value);
                    }
                    targets.push(target_value);
                    timestamps.push(row.timestamp.clone());
                }
            }
        }

        if targets.is_empty() {
            return Err(MlError::NoData(String::from(
                "no rows with complete features and target",
            )));
        }

        let features = Array2::from_shape_vec((targets.len(), feature_names.len()), matrix)
            .map_err(|err| MlError::Compute(err.to_string()))?;

        Ok(Dataset {
            feature_names,
            features,
            targets: Array1::from_vec(targets),
            timestamps,
        })
    }
}
