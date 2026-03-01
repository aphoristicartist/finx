use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuturesContract {
    pub symbol: String,
    pub underlying: String,
    pub expiry: String,
    pub contract_size: f64,
    pub tick_size: f64,
    pub margin_requirement: f64,
}

impl FuturesContract {
    pub fn new(
        symbol: String,
        underlying: String,
        expiry: String,
        contract_size: f64,
    ) -> Self {
        Self {
            symbol,
            underlying,
            expiry,
            contract_size,
            tick_size: 0.01,
            margin_requirement: 0.10,
        }
    }

    pub fn calculate_pnl(&self, entry_price: f64, current_price: f64, quantity: f64) -> f64 {
        (current_price - entry_price) * quantity * self.contract_size
    }

    pub fn calculate_margin(&self, price: f64, quantity: f64) -> f64 {
        price * quantity * self.contract_size * self.margin_requirement
    }
}
