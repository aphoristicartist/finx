pub mod error;
pub mod features;
pub mod models;
pub mod training;

pub use error::MlError;
pub use features::{FeatureConfig, FeatureEngineer, FeatureRow, FeatureStore, IndicatorSelection};
pub use models::Model;
pub use training::{Dataset, DatasetBuilder, TargetColumn};

pub type MlResult<T> = Result<T, MlError>;
