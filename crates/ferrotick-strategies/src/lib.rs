pub mod dsl;
pub mod error;
pub mod signals;
pub mod sizing;
pub mod strategies;
pub mod traits;

pub use dsl::{parse_and_validate_file, validate_strategy_spec, ValidationIssue};
pub use error::StrategyError;
pub use strategies::{
    built_in_strategies, BollingerBandSqueezeStrategy, MacdTrendStrategy,
    MovingAverageCrossoverStrategy, RsiMeanReversionStrategy,
};
pub use traits::{Order, OrderExecutionContext, OrderSide, Signal, SignalAction, Strategy};

pub type StrategyResult<T> = Result<T, StrategyError>;
