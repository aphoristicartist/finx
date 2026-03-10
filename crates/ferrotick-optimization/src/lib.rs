//! # Ferrotick Optimization
//!
//! Strategy parameter optimization with grid search and walk-forward validation.
//!
//! ## Overview
//!
//! This crate provides tools for optimizing trading strategy parameters:
//!
//! - **Grid Search**: Exhaustive parameter space exploration
//! - **Walk-Forward Validation**: Out-of-sample testing to prevent overfitting
//! - **Result Storage**: Persist optimization results for analysis
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use ferrotick_optimization::{GridSearchOptimizer, WalkForwardValidator};
//!
//! # fn main() {
//! // Create a grid search optimizer
//! let mut optimizer = GridSearchOptimizer::new();
//! optimizer
//!     .add_param("short_period", vec![5.0, 10.0, 20.0])
//!     .add_param("long_period", vec![20.0, 50.0, 100.0]);
//!
//! // Inspect parameter space size
//! let combinations = optimizer.total_combinations();
//! assert_eq!(combinations, 9);
//!
//! // Configure walk-forward validation
//! let validator = WalkForwardValidator::new(0.7, 0.2);
//! let _ = validator;
//! # }
//! ```

pub mod error;
pub mod grid_search;
pub mod storage;
pub mod walk_forward;

pub use error::{OptimizationError, OptimizationResult};
pub use grid_search::{GridSearchOptimizer, OptimizationReport, ParamRange, ParamResult};
pub use storage::{OptimizationRun, OptimizationStorage};
pub use walk_forward::{WalkForwardSummary, WalkForwardValidator, WalkForwardWindow};

// Re-export commonly used types from dependencies
pub use ferrotick_backtest::{BacktestConfig, BacktestReport};
pub use ferrotick_core::Bar;
