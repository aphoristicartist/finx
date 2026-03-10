use std::collections::HashMap;

use ferrotick_core::Bar;

use crate::traits::strategy::{Order, OrderExecutionContext, Signal, Strategy};

pub struct SignalGenerator {
    strategies: Vec<Box<dyn Strategy>>,
    strategy_ids: Vec<String>,
    strategy_indexes: HashMap<String, usize>,
    strategy_name_indexes: HashMap<String, Vec<usize>>,
}

impl SignalGenerator {
    pub fn new(strategies: Vec<Box<dyn Strategy>>) -> Self {
        let mut name_counts: HashMap<String, usize> = HashMap::new();
        for strategy in &strategies {
            *name_counts.entry(strategy.name().to_string()).or_insert(0) += 1;
        }

        let mut strategy_ids = Vec::with_capacity(strategies.len());
        let mut strategy_indexes = HashMap::with_capacity(strategies.len());
        let mut strategy_name_indexes: HashMap<String, Vec<usize>> = HashMap::new();
        let mut seen_duplicates: HashMap<String, usize> = HashMap::new();

        for (idx, strategy) in strategies.iter().enumerate() {
            let strategy_name = strategy.name().to_string();
            strategy_name_indexes
                .entry(strategy_name.clone())
                .or_default()
                .push(idx);

            let strategy_id = if name_counts.get(&strategy_name).copied().unwrap_or(0) > 1 {
                let next = seen_duplicates.entry(strategy_name.clone()).or_insert(0);
                *next += 1;
                format!("{strategy_name}#{next}")
            } else {
                strategy_name.clone()
            };

            strategy_ids.push(strategy_id.clone());
            strategy_indexes.insert(strategy_id, idx);
        }

        Self {
            strategies,
            strategy_ids,
            strategy_indexes,
            strategy_name_indexes,
        }
    }

    pub fn strategy_names(&self) -> Vec<String> {
        self.strategies
            .iter()
            .map(|s| s.name().to_string())
            .collect()
    }

    pub fn on_bar(&mut self, bar: &Bar) -> Vec<Signal> {
        let mut signals = Vec::new();
        for (idx, strategy) in self.strategies.iter_mut().enumerate() {
            if let Some(mut signal) = strategy.on_bar(bar) {
                signal.source_strategy_id = self.strategy_ids[idx].clone();
                signals.push(signal);
            }
        }
        signals
    }

    pub fn on_signal(&mut self, signal: &Signal) -> Vec<Order> {
        self.on_signal_with_context(signal, &OrderExecutionContext::default())
    }

    pub fn on_signal_with_context(
        &mut self,
        signal: &Signal,
        ctx: &OrderExecutionContext,
    ) -> Vec<Order> {
        let source_strategy_id = if signal.source_strategy_id.is_empty() {
            &signal.strategy_name
        } else {
            &signal.source_strategy_id
        };

        if let Some(&idx) = self.strategy_indexes.get(source_strategy_id) {
            return self
                .strategies
                .get_mut(idx)
                .and_then(|strategy| strategy.on_signal_with_context(signal, ctx))
                .into_iter()
                .collect();
        }

        let fallback_indexes = self
            .strategy_name_indexes
            .get(source_strategy_id)
            .cloned()
            .unwrap_or_default();
        let mut orders = Vec::new();
        for idx in fallback_indexes {
            if let Some(strategy) = self.strategies.get_mut(idx) {
                if let Some(order) = strategy.on_signal_with_context(signal, ctx) {
                    orders.push(order);
                }
            }
        }
        orders
    }

    pub fn reset(&mut self) {
        for strategy in &mut self.strategies {
            strategy.reset();
        }
    }
}
