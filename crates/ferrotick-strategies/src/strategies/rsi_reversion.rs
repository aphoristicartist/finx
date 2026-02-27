use ferrotick_core::Bar;
use ferrotick_ml::features::indicators::compute_rsi;

use crate::traits::strategy::{Order, OrderSide, Signal, SignalAction, Strategy};
use crate::{StrategyError, StrategyResult};

#[derive(Debug, Clone)]
pub struct RsiMeanReversionStrategy {
    symbol: String,
    period: usize,
    oversold: f64,
    overbought: f64,
    order_quantity: f64,
    closes: Vec<f64>,
}

impl RsiMeanReversionStrategy {
    pub fn new(
        symbol: impl Into<String>,
        period: usize,
        oversold: f64,
        overbought: f64,
        order_quantity: f64,
    ) -> StrategyResult<Self> {
        if period == 0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "rsi period must be > 0",
            )));
        }
        if !(0.0..=100.0).contains(&oversold)
            || !(0.0..=100.0).contains(&overbought)
            || oversold >= overbought
        {
            return Err(StrategyError::InvalidConfig(String::from(
                "rsi thresholds must satisfy 0 <= oversold < overbought <= 100",
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
            oversold,
            overbought,
            order_quantity,
            closes: Vec::new(),
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
        }
    }
}

impl Strategy for RsiMeanReversionStrategy {
    fn name(&self) -> &str {
        "rsi_mean_reversion"
    }

    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        self.closes.push(bar.close);
        if self.closes.len() < self.period {
            return None;
        }
        let rsi_series = match compute_rsi(&self.closes, self.period) {
            Ok(values) => values,
            Err(err) => {
                return Some(self.signal(bar, SignalAction::Hold, 0.0, format!("rsi_error={err}")))
            }
        };
        let Some(Some(rsi)) = rsi_series.last().copied() else {
            return None;
        };
        let action = if rsi < self.oversold {
            SignalAction::Buy
        } else if rsi > self.overbought {
            SignalAction::Sell
        } else {
            SignalAction::Hold
        };
        let strength = if matches!(action, SignalAction::Buy) {
            ((self.oversold - rsi) / 100.0).abs()
        } else if matches!(action, SignalAction::Sell) {
            ((rsi - self.overbought) / 100.0).abs()
        } else {
            0.0
        };
        Some(self.signal(bar, action, strength, format!("rsi={rsi:.4}")))
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
    }
}
