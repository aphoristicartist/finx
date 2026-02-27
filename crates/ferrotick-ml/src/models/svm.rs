use linfa::prelude::*;
use linfa_svm::Svm;
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
        let model = Svm::<f64, bool>::params()
            .pos_neg_weights(1.0, 1.0)
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

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(features.len(), false);
        let features_2d = features.view().insert_axis(ndarray::Axis(0));
        let dataset = linfa::Dataset::new(features_2d.to_owned(), dummy_labels);
        
        let prediction = model.predict(&dataset);

        Ok(prediction[0])
    }

    /// Batch predict for multiple samples.
    pub fn predict_batch(&self, features: &Array2<f64>) -> MlResult<Array1<bool>> {
        let model = self.model.as_ref().ok_or_else(|| {
            MlError::Prediction(String::from("model not trained"))
        })?;

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(features.nrows(), false);
        let dataset = linfa::Dataset::new(features.clone(), dummy_labels);
        
        let predictions = model.predict(&dataset);
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

        // Create a dataset with dummy labels for prediction
        let dummy_labels = Array1::from_elem(features.nrows(), false);
        let dataset = linfa::Dataset::new(features.clone(), dummy_labels);
        
        let bool_predictions = model.predict(&dataset);
        let predictions = bool_predictions.mapv(|b| if b { 1.0 } else { -1.0 });
        Ok(predictions)
    }
}
