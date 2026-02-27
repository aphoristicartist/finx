pub mod costs;
pub mod engine;
pub mod error;
pub mod metrics;
pub mod portfolio;

pub use costs::{FeeModel, SlippageModel, TransactionCosts};
pub use engine::{
    BacktestConfig, BacktestEngine, BacktestEvent, BacktestReport, BarEvent, SignalAction,
    SignalEvent, Strategy,
};
pub use error::BacktestError;
pub use metrics::{EquityPoint, MetricsReport, PerformanceMetrics};
pub use portfolio::{
    CashLedger, Fill, Order, OrderSide, OrderStatus, OrderType, Portfolio, Position,
};

/// Result type for backtesting operations.
pub type BacktestResult<T> = Result<T, BacktestError>;
