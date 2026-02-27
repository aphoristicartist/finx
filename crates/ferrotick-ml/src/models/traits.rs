use ndarray::{Array1, Array2};

use crate::MlResult;

pub trait Model {
    fn name(&self) -> &'static str;

    fn fit(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()>;

    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>>;
}
