use ferrotick_core::Bar;
use serde::{Deserialize, Serialize};

use crate::portfolio::OrderSide;

/// Slippage models used during simulated execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum SlippageModel {
    None,
    FixedBps {
        bps: f64,
    },
    VolumeShare {
        max_volume_share: f64,
        max_impact_bps: f64,
    },
}

impl Default for SlippageModel {
    fn default() -> Self {
        Self::None
    }
}

impl SlippageModel {
    pub fn execution_price(&self, side: OrderSide, bar: &Bar, quantity: f64) -> f64 {
        let close = bar.close;
        if close <= 0.0 || !close.is_finite() {
            return close;
        }

        let bps = self.effective_bps(bar, quantity);
        let signed_bps = match side {
            OrderSide::Buy => bps,
            OrderSide::Sell => -bps,
        };

        close * (1.0 + signed_bps / 10_000.0)
    }

    pub fn slippage_amount(&self, side: OrderSide, bar: &Bar, quantity: f64) -> f64 {
        let execution = self.execution_price(side, bar, quantity);
        (execution - bar.close).abs() * quantity.abs()
    }

    fn effective_bps(&self, bar: &Bar, quantity: f64) -> f64 {
        match self {
            SlippageModel::None => 0.0,
            SlippageModel::FixedBps { bps } => bps.max(0.0),
            SlippageModel::VolumeShare {
                max_volume_share,
                max_impact_bps,
            } => {
                let volume = bar.volume.unwrap_or(0) as f64;
                if volume <= 0.0 {
                    return max_impact_bps.max(0.0);
                }

                let max_share = max_volume_share.max(1e-9);
                let share = (quantity.abs() / volume).clamp(0.0, max_share);
                let utilization = share / max_share;
                max_impact_bps.max(0.0) * utilization
            }
        }
    }
}
