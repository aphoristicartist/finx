use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signal types for trading decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    Buy = 1,
    Sell = -1,
    Hold = 0,
}

impl From<i8> for Signal {
    fn from(value: i8) -> Self {
        match value {
            1 => Signal::Buy,
            -1 => Signal::Sell,
            _ => Signal::Hold,
        }
    }
}

impl From<Signal> for i8 {
    fn from(signal: Signal) -> Self {
        signal as i8
    }
}

/// Trait for signal generators
pub trait SignalGenerator: Send + Sync {
    fn generate_signals(
        &self,
        params: &HashMap<String, f64>,
    ) -> Result<Array1<i8>, crate::BacktestError>;
    fn name(&self) -> &str;
}
