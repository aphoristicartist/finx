use std::collections::VecDeque;

use ferrotick_core::Bar;

use crate::traits::strategy::{Order, OrderSide, Signal, SignalAction, Strategy};
use crate::{StrategyError, StrategyResult};

#[derive(Debug, Clone)]
pub struct MovingAverageCrossoverStrategy {
    symbol: String,
    fast_period: usize,
    slow_period: usize,
    order_quantity: f64,
    closes: VecDeque<f64>,
    prev_fast: Option<f64>,
    prev_slow: Option<f64>,
}

impl MovingAverageCrossoverStrategy {
    pub fn new(
        symbol: impl Into<String>,
        fast_period: usize,
        slow_period: usize,
        order_quantity: f64,
    ) -> StrategyResult<Self> {
        if fast_period == 0 || slow_period == 0 || fast_period >= slow_period {
            return Err(StrategyError::InvalidConfig(String::from(
                "ma_crossover requires fast_period > 0, slow_period > 0, fast_period < slow_period",
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
            order_quantity,
            closes: VecDeque::new(),
            prev_fast: None,
            prev_slow: None,
        })
    }

    fn sma(closes: &VecDeque<f64>, period: usize) -> f64 {
        closes.iter().rev().take(period).sum::<f64>() / period as f64
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

impl Strategy for MovingAverageCrossoverStrategy {
    fn name(&self) -> &str {
        "ma_crossover"
    }

    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        self.closes.push_back(bar.close);
        while self.closes.len() > self.slow_period {
            self.closes.pop_front();
        }
        if self.closes.len() < self.slow_period {
            return None;
        }
        let fast = Self::sma(&self.closes, self.fast_period);
        let slow = Self::sma(&self.closes, self.slow_period);
        let action = match (self.prev_fast, self.prev_slow) {
            (Some(prev_fast), Some(prev_slow)) if prev_fast <= prev_slow && fast > slow => {
                SignalAction::Buy
            }
            (Some(prev_fast), Some(prev_slow)) if prev_fast >= prev_slow && fast < slow => {
                SignalAction::Sell
            }
            _ => SignalAction::Hold,
        };
        self.prev_fast = Some(fast);
        self.prev_slow = Some(slow);
        let spread = ((fast - slow) / bar.close.max(1e-9)).abs();
        let reason = format!("fast_sma={fast:.4}, slow_sma={slow:.4}");
        Some(self.signal(bar, action, spread, reason))
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
        self.prev_fast = None;
        self.prev_slow = None;
    }
}
