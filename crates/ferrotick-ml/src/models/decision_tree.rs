use linfa::prelude::*;
use linfa_trees::DecisionTree;
use ndarray::{Array1, Array2};

use super::Model;
use crate::{MlError, MlResult};

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
        let mut params = linfa_trees::DecisionTree::params();
        if let Some(depth) = self.max_depth {
            params = params.max_depth(Some(depth));
        }

        let model = params
            .fit(&dataset)
            .map_err(|e| MlError::Training(format!("Decision tree training failed: {}", e)))?;

        self.model = Some(model);
        Ok(())
    }

    /// Predict signal.
    pub fn predict(&self, features: &Array1<f64>) -> MlResult<bool> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| MlError::Prediction(String::from("model not trained")))?;

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(features.len(), false);
        let features_2d = features.view().insert_axis(ndarray::Axis(0));
        let dataset = linfa::Dataset::new(features_2d.to_owned(), dummy_labels);

        let prediction = model.predict(&dataset);

        Ok(prediction[0])
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
