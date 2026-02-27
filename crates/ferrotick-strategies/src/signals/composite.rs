use std::collections::HashMap;

use ferrotick_core::Bar;

use crate::signals::generator::SignalGenerator;
use crate::traits::strategy::{Signal, SignalAction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeMode {
    Majority,
    Unanimous,
    WeightedPerformance,
}

pub struct CompositeSignalGenerator {
    generator: SignalGenerator,
    mode: CompositeMode,
    performance_weights: HashMap<String, f64>,
}

impl CompositeSignalGenerator {
    pub fn new(generator: SignalGenerator, mode: CompositeMode) -> Self {
        Self {
            generator,
            mode,
            performance_weights: HashMap::new(),
        }
    }

    pub fn set_weight(&mut self, strategy_name: impl Into<String>, weight: f64) {
        self.performance_weights
            .insert(strategy_name.into(), weight);
    }

    pub fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        let signals = self.generator.on_bar(bar);
        if signals.is_empty() {
            return None;
        }
        Some(self.combine(&signals))
    }

    fn combine(&self, signals: &[Signal]) -> Signal {
        let first = &signals[0];
        match self.mode {
            CompositeMode::Majority => self.combine_majority(signals, first),
            CompositeMode::Unanimous => self.combine_unanimous(signals, first),
            CompositeMode::WeightedPerformance => self.combine_weighted(signals, first),
        }
    }

    fn combine_majority(&self, signals: &[Signal], seed: &Signal) -> Signal {
        let mut buy = 0usize;
        let mut sell = 0usize;
        for signal in signals {
            match signal.action {
                SignalAction::Buy => buy += 1,
                SignalAction::Sell => sell += 1,
                SignalAction::Hold => {}
            }
        }
        let action = if buy > sell {
            SignalAction::Buy
        } else if sell > buy {
            SignalAction::Sell
        } else {
            SignalAction::Hold
        };
        let strength = (buy.max(sell) as f64) / (signals.len() as f64);
        Signal {
            symbol: seed.symbol.clone(),
            ts: seed.ts.clone(),
            action,
            strength,
            reason: format!("majority buy={buy}, sell={sell}, total={}", signals.len()),
            strategy_name: "composite_majority".to_string(),
        }
    }

    fn combine_unanimous(&self, signals: &[Signal], seed: &Signal) -> Signal {
        let first_action = signals[0].action;
        let all_same = signals.iter().all(|s| s.action == first_action);
        let action = if all_same {
            first_action
        } else {
            SignalAction::Hold
        };
        Signal {
            symbol: seed.symbol.clone(),
            ts: seed.ts.clone(),
            action,
            strength: if all_same { 1.0 } else { 0.0 },
            reason: format!("unanimous all_same={all_same}"),
            strategy_name: "composite_unanimous".to_string(),
        }
    }

    fn combine_weighted(&self, signals: &[Signal], seed: &Signal) -> Signal {
        let mut weighted_score = 0.0;
        let mut weight_sum = 0.0;
        for signal in signals.iter() {
            let weight = self
                .performance_weights
                .get(&signal.strategy_name)
                .copied()
                .unwrap_or(1.0)
                .max(0.0);
            let score = match signal.action {
                SignalAction::Buy => 1.0,
                SignalAction::Sell => -1.0,
                SignalAction::Hold => 0.0,
            };
            weighted_score += score * weight;
            weight_sum += weight;
        }
        let normalized = if weight_sum <= 0.0 {
            0.0
        } else {
            weighted_score / weight_sum
        };
        let action = if normalized > 0.2 {
            SignalAction::Buy
        } else if normalized < -0.2 {
            SignalAction::Sell
        } else {
            SignalAction::Hold
        };
        Signal {
            symbol: seed.symbol.clone(),
            ts: seed.ts.clone(),
            action,
            strength: normalized.abs().clamp(0.0, 1.0),
            reason: format!("weighted_score={normalized:.4}"),
            strategy_name: "composite_weighted".to_string(),
        }
    }
}
