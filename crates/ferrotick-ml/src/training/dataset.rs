use ndarray::{Array1, Array2};

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

pub struct DatasetBuilder;

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

            if feature_values.iter().all(Option::is_some) && target_value.is_some() {
                for value in feature_values.iter().flatten() {
                    matrix.push(*value);
                }
                targets.push(target_value.expect("checked is_some"));
                timestamps.push(row.timestamp.clone());
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
