use ferrotick_core::Bar;
use ferrotick_ml::features::indicators::compute_macd;

use crate::traits::strategy::{Order, OrderSide, Signal, SignalAction, Strategy};
use crate::{StrategyError, StrategyResult};

#[derive(Debug, Clone)]
pub struct MacdTrendStrategy {
    symbol: String,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    order_quantity: f64,
    closes: Vec<f64>,
    prev_macd: Option<f64>,
    prev_signal: Option<f64>,
}

impl MacdTrendStrategy {
    pub fn new(
        symbol: impl Into<String>,
        fast_period: usize,
        slow_period: usize,
        signal_period: usize,
        order_quantity: f64,
    ) -> StrategyResult<Self> {
        if fast_period == 0 || slow_period == 0 || signal_period == 0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "macd periods must all be > 0",
            )));
        }
        if fast_period >= slow_period {
            return Err(StrategyError::InvalidConfig(String::from(
                "macd fast_period must be < slow_period",
            )));
        }
        if !order_quantity.is_finite() || order_quantity <= 0.0 {
            return Err(StrategyError::InvalidConfig(String::from(
                "order_quantity must be finite and > 0",
            )));
        }
        Ok(Self {
            symbol: symbol.into(),
            fast_period,
            slow_period,
            signal_period,
            order_quantity,
            closes: Vec::new(),
            prev_macd: None,
            prev_signal: None,
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

impl Strategy for MacdTrendStrategy {
    fn name(&self) -> &str {
        "macd_trend"
    }

    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        self.closes.push(bar.close);
        if self.closes.len() < self.slow_period + self.signal_period {
            return None;
        }
        let macd_result = match compute_macd(
            &self.closes,
            self.fast_period,
            self.slow_period,
            self.signal_period,
        ) {
            Ok(m) => m,
            Err(err) => {
                return Some(self.signal(
                    bar,
                    SignalAction::Hold,
                    0.0,
                    format!("macd_error={err}"),
                ));
            }
        };
        let Some(Some(macd)) = macd_result.macd.last().copied() else {
            return None;
        };
        let Some(Some(signal)) = macd_result.signal.last().copied() else {
            return None;
        };
        let action = match (self.prev_macd, self.prev_signal) {
            (Some(prev_macd), Some(prev_signal)) if prev_macd <= prev_signal && macd > signal => {
                SignalAction::Buy
            }
            (Some(prev_macd), Some(prev_signal)) if prev_macd >= prev_signal && macd < signal => {
                SignalAction::Sell
            }
            _ => SignalAction::Hold,
        };
        self.prev_macd = Some(macd);
        self.prev_signal = Some(signal);
        let spread = ((macd - signal) / bar.close.max(1e-9)).abs();
        Some(self.signal(
            bar,
            action,
            spread,
            format!("macd={macd:.4}, signal={signal:.4}"),
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
        self.prev_macd = None;
        self.prev_signal = None;
    }
}
