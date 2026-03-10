use ndarray::{Array1, Array2};
use std::path::Path;

use crate::MlResult;

pub trait Model {
    fn name(&self) -> &'static str;

    fn fit(&mut self, features: &Array2<f64>, targets: &Array1<f64>) -> MlResult<()>;

    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>>;
}

pub trait PersistentModel: Sized {
    fn save(&self, path: &Path) -> MlResult<()>;

    fn load(path: &Path) -> MlResult<Self>;
}
