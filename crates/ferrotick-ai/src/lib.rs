//! AI-powered features for Ferrotick.
//!
//! This crate provides LLM integration for natural language strategy development
//! and backtest reporting.

pub mod compiler;
pub mod error;
pub mod llm;
pub mod reporting;
pub mod validation;

pub use compiler::StrategyCompiler;
pub use error::{AIError, AIResult};
pub use llm::{LLMClient, OpenAIClient};
pub use reporting::{templates, BacktestReporter};
pub use validation::OutputSanitizer;

// Re-export for convenience
pub use ferrotick_strategies::dsl::StrategySpec;
