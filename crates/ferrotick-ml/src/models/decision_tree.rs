use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{Model, PersistentModel};
use crate::{MlError, MlResult};

#[derive(Clone)]
struct TrainingSnapshot {
    features: Array2<f64>,
    targets: Array1<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializableMatrix {
    nrows: usize,
    ncols: usize,
    values: Vec<f64>,
}

impl SerializableMatrix {
    fn from_array2(array: &Array2<f64>) -> Self {
        Self {
            nrows: array.nrows(),
            ncols: array.ncols(),
            values: array.iter().copied().collect(),
        }
    }

    fn into_array2(self) -> MlResult<Array2<f64>> {
        let expected_len = self.nrows.checked_mul(self.ncols).ok_or_else(|| {
            MlError::InvalidInput(String::from("serialized matrix shape overflow"))
        })?;

        if self.values.len() != expected_len {
            return Err(MlError::InvalidInput(format!(
                "serialized matrix length mismatch: expected {}, got {}",
                expected_len,
                self.values.len()
            )));
        }

        Array2::from_shape_vec((self.nrows, self.ncols), self.values)
            .map_err(|e| MlError::InvalidInput(format!("invalid serialized matrix: {}", e)))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializableTrainingSnapshot {
    features: SerializableMatrix,
    targets: Vec<f64>,
}

impl SerializableTrainingSnapshot {
    fn from_training_data(training_data: &TrainingSnapshot) -> Self {
        Self {
            features: SerializableMatrix::from_array2(&training_data.features),
            targets: training_data.targets.to_vec(),
        }
    }

    fn into_training_data(self) -> MlResult<TrainingSnapshot> {
        let features = self.features.into_array2()?;
        if self.targets.len() != features.nrows() {
            return Err(MlError::InvalidInput(format!(
                "serialized target length mismatch: expected {}, got {}",
                features.nrows(),
                self.targets.len()
            )));
        }

        Ok(TrainingSnapshot {
            features,
            targets: Array1::from_vec(self.targets),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializableDecisionTree {
    max_depth: Option<usize>,
    training_data: SerializableTrainingSnapshot,
}

/// Decision Tree classifier for trading signals.
pub struct DecisionTreeClassifier {
    model: Option<DecisionTree<f64, bool>>,
    max_depth: Option<usize>,
    training_data: Option<TrainingSnapshot>,
}

impl DecisionTreeClassifier {
    pub fn new(max_depth: Option<usize>) -> Self {
        Self {
            model: None,
            max_depth,
            training_data: None,
        }
    }

    /// Train the decision tree.
    pub fn train(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()> {
        // Convert targets to boolean
        let labels: Vec<bool> = targets.iter().map(|&t| t > 0.0).collect();

        // Create dataset
        let dataset = linfa::Dataset::new(features.clone(), Array1::from_vec(labels));

        // Build decision tree
        let mut params = linfa_trees::DecisionTree::params();
        if let Some(depth) = self.max_depth {
            params = params.max_depth(Some(depth));
        }

        let model = params
            .fit(&dataset)
            .map_err(|e| MlError::Training(format!("Decision tree training failed: {}", e)))?;

        self.model = Some(model);
        self.training_data = Some(TrainingSnapshot {
            features: features.clone(),
            targets: targets.clone(),
        });
        Ok(())
    }

    /// Predict signal.
    pub fn predict(&self, features: &Array1<f64>) -> MlResult<bool> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| MlError::Prediction(String::from("model not trained")))?;

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(1, false);
        let features_2d = features.view().insert_axis(ndarray::Axis(0));
        let dataset = linfa::Dataset::new(features_2d.to_owned(), dummy_labels);

        let prediction = model.predict(&dataset);

        Ok(prediction[0])
    }

    /// Persist the model to disk by storing the training snapshot as JSON.
    pub fn save(&self, path: &Path) -> Result<(), MlError> {
        if self.model.is_none() {
            return Err(MlError::InvalidInput(String::from(
                "cannot save untrained model",
            )));
        }

        let training_data = self.training_data.as_ref().ok_or_else(|| {
            MlError::InvalidInput(String::from("missing training data for model persistence"))
        })?;

        let serializable = SerializableDecisionTree {
            max_depth: self.max_depth,
            training_data: SerializableTrainingSnapshot::from_training_data(training_data),
        };

        let json = serde_json::to_string_pretty(&serializable)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load the model from disk and reconstruct the trained classifier.
    pub fn load(path: &Path) -> Result<Self, MlError> {
        let json = std::fs::read_to_string(path)?;
        let serializable: SerializableDecisionTree = serde_json::from_str(&json)?;
        let training_data = serializable.training_data.into_training_data()?;

        let mut classifier = Self::new(serializable.max_depth);
        classifier.train(&training_data.features, &training_data.targets)?;
        Ok(classifier)
    }

    /// Get feature importance scores.
    pub fn feature_importance(&self) -> Option<Vec<f64>> {
        let _model = self.model.as_ref()?;
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
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| MlError::Prediction(String::from("model not trained")))?;

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(features.nrows(), false);
        let dataset = linfa::Dataset::new(features.clone(), dummy_labels);

        let bool_predictions = model.predict(&dataset);
        let predictions = bool_predictions.mapv(|b| if b { 1.0 } else { -1.0 });
        Ok(predictions)
    }
}

impl PersistentModel for DecisionTreeClassifier {
    fn save(&self, path: &Path) -> MlResult<()> {
        DecisionTreeClassifier::save(self, path)
    }

    fn load(path: &Path) -> MlResult<Self> {
        DecisionTreeClassifier::load(path)
    }
}
