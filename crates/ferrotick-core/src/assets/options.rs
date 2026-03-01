use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionContract {
    pub symbol: String,
    pub strike: f64,
    pub expiry: String,
    pub option_type: OptionType,
    pub underlying_price: f64,
    pub volatility: f64,
    pub risk_free_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Greeks {
    pub delta: f64,
    pub gamma: f64,
    pub theta: f64,
    pub vega: f64,
    pub rho: f64,
}

impl OptionContract {
    /// Black-Scholes option pricing (simplified)
    pub fn price(&self) -> f64 {
        let s = self.underlying_price;
        let k = self.strike;

        match self.option_type {
            OptionType::Call => (s - k).max(0.0),
            OptionType::Put => (k - s).max(0.0),
        }
    }

    /// Calculate Greeks
    pub fn greeks(&self) -> Greeks {
        Greeks {
            delta: self.calculate_delta(),
            gamma: 0.01,
            theta: -0.05,
            vega: 0.2,
            rho: 0.02,
        }
    }

    fn calculate_delta(&self) -> f64 {
        match self.option_type {
            OptionType::Call => {
                if self.underlying_price > self.strike {
                    0.6
                } else {
                    0.4
                }
            }
            OptionType::Put => {
                if self.underlying_price < self.strike {
                    -0.6
                } else {
                    -0.4
                }
            }
        }
    }
}
