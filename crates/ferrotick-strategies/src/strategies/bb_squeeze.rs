use std::collections::VecDeque;

use ferrotick_core::Bar;
use ferrotick_ml::features::indicators::compute_bollinger;

use crate::traits::strategy::{Order, OrderSide, Signal, SignalAction, Strategy};
use crate::{StrategyError, StrategyResult};

const MAX_HISTORY: usize = 1000;

#[derive(Debug, Clone)]
pub struct BollingerBandSqueezeStrategy {
    symbol: String,
    period: usize,
    num_std: f64,
    order_quantity: f64,
    closes: VecDeque<f64>,
    prev_in_squeeze: bool,
}

impl BollingerBandSqueezeStrategy {
    pub fn new(
        symbol: impl Into<String>,
        period: usize,
        num_std: f64,
        order_quantity: f64,
    ) -> StrategyResult<Self> {
        if period == 0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "bb_squeeze period must be > 0",
            )));
        }
        if !num_std.is_finite() || num_std <= 0.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "bb_squeeze num_std must be > 0",
            )));
        }
        if !order_quantity.is_finite() || order_quantity <= 0.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "order_quantity must be finite and > 0",
            )));
        }
        Ok(Self {
            symbol: symbol.into(),
            period,
            num_std,
            order_quantity,
            closes: VecDeque::with_capacity(MAX_HISTORY),
            prev_in_squeeze: false,
        })
    }

    fn signal(
        &self,
        bar: &Bar,
        action: SignalAction,
        strength: f64,
        reason: impl Into<String>,
    ) -> Signal {
        Signal {
            symbol: self.symbol.clone(),
            ts: bar.ts.format_rfc3339(),
            action,
            strength: strength.clamp(0.0, 1.0),
            reason: reason.into(),
            strategy_name: self.name().to_string(),
        }
    }
}

impl Strategy for BollingerBandSqueezeStrategy {
    fn name(&self) -> &str {
        "bb_squeeze"
    }

    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        self.closes.push_back(bar.close);

        // Bound the history to prevent O(N²)
        if self.closes.len() > MAX_HISTORY {
            self.closes.pop_front();
        }

        if self.closes.len() < self.period {
            return None;
        }

        // Use make_contiguous for compute_bollinger
        let closes: Vec<f64> = self.closes.make_contiguous().to_vec();

        let bb = match compute_bollinger(&closes, self.period, self.num_std) {
            Ok(bb) => bb,
            Err(err) => {
                return Some(self.signal(bar, SignalAction::Hold, 0.0, format!("bb_error={err}")));
            }
        };
        let Some(Some(upper)) = bb.upper.last().copied() else {
            return None;
        };
        let Some(Some(lower)) = bb.lower.last().copied() else {
            return None;
        };
        // Use SMA for middle band
        let middle = (upper + lower) / 2.0;
        let bandwidth = (upper - lower) / middle.max(1e-9);
        let in_squeeze = bandwidth < 0.05;
        let action = if self.prev_in_squeeze && !in_squeeze {
            if bar.close > upper {
                SignalAction::Buy
            } else if bar.close < lower {
                SignalAction::Sell
            } else {
                SignalAction::Hold
            }
        } else {
            SignalAction::Hold
        };
        self.prev_in_squeeze = in_squeeze;
        let strength = if in_squeeze { 0.0 } else { bandwidth };
        Some(self.signal(
            bar,
            action,
            strength,
            format!("bb_upper={upper:.4}, bb_lower={lower:.4}, bandwidth={bandwidth:.4}"),
        ))
    }

    fn on_signal(&mut self, signal: &Signal) -> Option<Order> {
        match signal.action {
            SignalAction::Buy => Some(Order::market(
                signal.symbol.clone(),
                OrderSide::Buy,
                self.order_quantity,
                signal.reason.clone(),
            )),
            SignalAction::Sell => Some(Order::market(
                signal.symbol.clone(),
                OrderSide::Sell,
                self.order_quantity,
                signal.reason.clone(),
            )),
            SignalAction::Hold => None,
        }
    }

    fn reset(&mut self) {
        self.closes.clear();
        self.prev_in_squeeze = false;
    }
}
