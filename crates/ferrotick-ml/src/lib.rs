pub mod error;
pub mod features;
pub mod models;
pub mod rl;
pub mod training;

pub use error::MlError;
pub use features::{FeatureConfig, FeatureEngineer, FeatureRow, FeatureStore, IndicatorSelection};
pub use models::{DecisionTreeClassifier, Model, SVMClassifier};
pub use rl::{
    Action, ActionKey, Agent, BalanceBucket, Environment, Position, PositionBucket,
    PriceChangeBucket, QTableAgent, QTableConfig, RandomAgent, RewardCalculator, RewardConfig,
    StateKey, StepResult, TradingEnvironment, TradingState,
};
pub use training::{cross_validate, Dataset, DatasetBuilder, ModelMetrics, TargetColumn};

pub type MlResult<T> = Result<T, MlError>;
