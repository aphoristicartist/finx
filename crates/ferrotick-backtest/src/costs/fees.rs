use serde::{Deserialize, Serialize};

/// Commission/fee models used by the backtest engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum FeeModel {
    None,
    Flat { amount: f64 },
    PerShare { amount: f64 },
    Bps { bps: f64 },
}

impl Default for FeeModel {
    fn default() -> Self {
        Self::None
    }
}

impl FeeModel {
    pub fn compute(&self, quantity: f64, price: f64) -> f64 {
        let qty = quantity.abs();
        let px = price.abs();

        match self {
            FeeModel::None => 0.0,
            FeeModel::Flat { amount } => amount.max(0.0),
            FeeModel::PerShare { amount } => amount.max(0.0) * qty,
            FeeModel::Bps { bps } => (bps.max(0.0) / 10_000.0) * qty * px,
        }
    }
}
