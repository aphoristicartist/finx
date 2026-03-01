use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForexPair {
    pub base_currency: String,
    pub quote_currency: String,
    pub exchange_rate: f64,
    pub pip_value: f64,
    pub lot_size: f64,
}

impl ForexPair {
    pub fn new(base: String, quote: String, rate: f64) -> Self {
        Self {
            base_currency: base,
            quote_currency: quote,
            exchange_rate: rate,
            pip_value: 0.0001,
            lot_size: 100_000.0,
        }
    }

    pub fn calculate_pip_value(&self, lot_count: f64) -> f64 {
        self.pip_value * self.lot_size * lot_count
    }

    pub fn convert(&self, amount: f64, from_base: bool) -> f64 {
        if from_base {
            amount * self.exchange_rate
        } else {
            amount / self.exchange_rate
        }
    }
}
