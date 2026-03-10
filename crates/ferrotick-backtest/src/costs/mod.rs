pub mod fees;
pub mod slippage;

use serde::{Deserialize, Serialize};

pub use fees::FeeModel;
pub use slippage::SlippageModel;

/// Transaction cost configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionCosts {
    pub fee_model: FeeModel,
}

impl TransactionCosts {
    pub fn commission(&self, quantity: f64, price: f64) -> f64 {
        self.fee_model.compute(quantity, price)
    }
}
