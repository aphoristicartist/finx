use ferrotick_core::Bar;

use crate::traits::strategy::{Order, Signal, Strategy};

pub struct SignalGenerator {
    strategies: Vec<Box<dyn Strategy>>,
}

impl SignalGenerator {
    pub fn new(strategies: Vec<Box<dyn Strategy>>) -> Self {
        Self { strategies }
    }

    pub fn strategy_names(&self) -> Vec<String> {
        self.strategies
            .iter()
            .map(|s| s.name().to_string())
            .collect()
    }

    pub fn on_bar(&mut self, bar: &Bar) -> Vec<Signal> {
        self.strategies
            .iter_mut()
            .filter_map(|strategy| strategy.on_bar(bar))
            .collect()
    }

    pub fn on_signal(&mut self, signal: &Signal) -> Vec<Order> {
        self.strategies
            .iter_mut()
            .filter_map(|strategy| strategy.on_signal(signal))
            .collect()
    }

    pub fn reset(&mut self) {
        for strategy in &mut self.strategies {
            strategy.reset();
        }
    }
}
