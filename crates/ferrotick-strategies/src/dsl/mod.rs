pub mod parser;
pub mod validator;

use std::path::Path;

use ferrotick_core::Bar;

use crate::sizing::position::{build_sizer, PositionSizingConfig, PositionSizingMethod};
use crate::strategies::{
    BollingerBandSqueezeStrategy, MacdTrendStrategy, MovingAverageCrossoverStrategy,
    RsiMeanReversionStrategy,
};
use crate::traits::strategy::{Order, OrderExecutionContext, Signal, Strategy};
use crate::{StrategyError, StrategyResult};

pub use parser::{IndicatorRule, RuleValue, StrategySpec};
pub use validator::{validate_strategy_spec, ValidationIssue};

pub fn parse_and_validate_file(path: &Path) -> StrategyResult<StrategySpec> {
    let raw = std::fs::read_to_string(path).map_err(StrategyError::Io)?;
    parse_and_validate_strategy_yaml(&raw)
}

pub fn parse_and_validate_strategy_yaml(raw: &str) -> StrategyResult<StrategySpec> {
    let spec = parser::parse_strategy_yaml(raw)?;
    let issues = validate_strategy_spec(&spec);
    if !issues.is_empty() {
        return Err(StrategyError::ValidationErrors(
            issues
                .into_iter()
                .map(|issue| format!("{}: {}", issue.field, issue.message))
                .collect(),
        ));
    }
    Ok(spec)
}

pub fn build_strategy(spec: &StrategySpec, symbol: &str) -> StrategyResult<Box<dyn Strategy>> {
    let default_qty = spec.position_sizing.value;
    let base_strategy: Box<dyn Strategy> = match spec.strategy_type.as_str() {
        "trend_following" | "ma_crossover" => {
            let fast = extract_period(&spec.entry_rules, "fast_period", 10);
            let slow = extract_period(&spec.entry_rules, "slow_period", 20);
            Ok(Box::new(MovingAverageCrossoverStrategy::new(
                symbol,
                fast,
                slow,
                default_qty,
            )?) as Box<dyn Strategy>)
        }
        "mean_reversion" | "rsi_mean_reversion" => {
            let period = extract_period(&spec.entry_rules, "period", 14);
            let oversold = extract_value(&spec.entry_rules, "<", 30.0);
            let overbought = extract_value(&spec.exit_rules, ">", 70.0);
            Ok(Box::new(RsiMeanReversionStrategy::new(
                symbol,
                period,
                oversold,
                overbought,
                default_qty,
            )?) as Box<dyn Strategy>)
        }
        "macd_trend" => {
            let fast = extract_period(&spec.entry_rules, "fast_period", 12);
            let slow = extract_period(&spec.entry_rules, "slow_period", 26);
            let signal = extract_period(&spec.entry_rules, "signal_period", 9);
            Ok(Box::new(MacdTrendStrategy::new(
                symbol,
                fast,
                slow,
                signal,
                default_qty,
            )?) as Box<dyn Strategy>)
        }
        "bb_squeeze" => {
            let period = extract_period(&spec.entry_rules, "period", 20);
            let num_std = extract_indicator_value(&spec.entry_rules, "num_std", 2.0);
            Ok(Box::new(BollingerBandSqueezeStrategy::new(
                symbol,
                period,
                num_std,
                default_qty,
            )?) as Box<dyn Strategy>)
        }
        _ => Err(StrategyError::InvalidConfig(format!(
            "unknown strategy type: {}",
            spec.strategy_type
        ))),
    }?;

    let sizing = DslPositionSizing::from_spec(spec)?;
    Ok(Box::new(DslPositionSizedStrategy::new(
        base_strategy,
        sizing,
    )))
}

pub fn build_position_sizer(
    spec: &StrategySpec,
) -> StrategyResult<Box<dyn crate::sizing::position::PositionSizer>> {
    let method = match spec.position_sizing.method.as_str() {
        "fixed" => PositionSizingMethod::Fixed,
        "percent" => PositionSizingMethod::Percent,
        "risk" | "volatility" => PositionSizingMethod::Volatility,
        "kelly" => PositionSizingMethod::Kelly,
        other => {
            return Err(StrategyError::InvalidConfig(format!(
                "unknown position sizing method: {other}"
            )))
        }
    };
    Ok(build_sizer(PositionSizingConfig {
        method,
        value: spec.position_sizing.value,
    }))
}

fn extract_period(rules: &[IndicatorRule], name: &str, default: usize) -> usize {
    rules
        .iter()
        .find(|r| r.indicator == name || r.indicator.contains(name.split('_').next().unwrap_or("")))
        .and_then(|r| r.period)
        .unwrap_or(default)
}

fn extract_value(rules: &[IndicatorRule], operator: &str, default: f64) -> f64 {
    rules
        .iter()
        .find(|r| r.operator == operator)
        .map(|r| r.value.to_f64())
        .unwrap_or(default)
}

fn extract_indicator_value(rules: &[IndicatorRule], indicator: &str, default: f64) -> f64 {
    rules
        .iter()
        .find(|r| r.indicator.eq_ignore_ascii_case(indicator))
        .map(|r| r.value.to_f64())
        .unwrap_or(default)
}

#[derive(Debug, Clone, Copy)]
enum DslPositionSizingMethod {
    Fixed,
    Percent,
    Risk,
    Kelly,
}

#[derive(Debug, Clone, Copy)]
struct DslPositionSizing {
    method: DslPositionSizingMethod,
    value: f64,
    stop_loss: Option<f64>,
}

impl DslPositionSizing {
    fn from_spec(spec: &StrategySpec) -> StrategyResult<Self> {
        let method = match spec.position_sizing.method.as_str() {
            "fixed" => DslPositionSizingMethod::Fixed,
            "percent" => DslPositionSizingMethod::Percent,
            "risk" | "volatility" => DslPositionSizingMethod::Risk,
            "kelly" => DslPositionSizingMethod::Kelly,
            other => {
                return Err(StrategyError::InvalidConfig(format!(
                    "unknown position sizing method: {other}"
                )))
            }
        };

        Ok(Self {
            method,
            value: spec.position_sizing.value,
            stop_loss: spec.risk_management.as_ref().and_then(|rm| rm.stop_loss),
        })
    }

    fn calculate_quantity(&self, ctx: &OrderExecutionContext) -> Option<f64> {
        let portfolio_value = ctx.portfolio_value;
        let price = ctx.price;
        if !portfolio_value.is_finite()
            || portfolio_value <= 0.0
            || !price.is_finite()
            || price <= 0.0
        {
            return None;
        }

        let quantity = match self.method {
            DslPositionSizingMethod::Fixed => self.value / price,
            DslPositionSizingMethod::Percent => (portfolio_value * self.value / 100.0) / price,
            DslPositionSizingMethod::Risk => {
                let stop_loss_price = self.stop_loss_price(price)?;
                let risk_per_unit = price - stop_loss_price;
                if !risk_per_unit.is_finite() || risk_per_unit <= 0.0 {
                    return None;
                }
                (portfolio_value * self.value / 100.0) / risk_per_unit
            }
            DslPositionSizingMethod::Kelly => (portfolio_value * self.value) / price,
        };

        if quantity.is_finite() && quantity > 0.0 {
            Some(quantity)
        } else {
            None
        }
    }

    fn stop_loss_price(&self, price: f64) -> Option<f64> {
        let stop_loss = self.stop_loss?;
        if !stop_loss.is_finite() || stop_loss <= 0.0 {
            return None;
        }
        if stop_loss < 1.0 {
            Some(price * (1.0 - stop_loss))
        } else if stop_loss < price {
            Some(stop_loss)
        } else {
            None
        }
    }
}

struct DslPositionSizedStrategy {
    inner: Box<dyn Strategy>,
    sizing: DslPositionSizing,
}

impl DslPositionSizedStrategy {
    fn new(inner: Box<dyn Strategy>, sizing: DslPositionSizing) -> Self {
        Self { inner, sizing }
    }
}

impl Strategy for DslPositionSizedStrategy {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        self.inner.on_bar(bar)
    }

    fn on_signal(&mut self, signal: &Signal) -> Option<Order> {
        self.inner.on_signal(signal)
    }

    fn on_signal_with_context(
        &mut self,
        signal: &Signal,
        ctx: &OrderExecutionContext,
    ) -> Option<Order> {
        let mut order = self.inner.on_signal_with_context(signal, ctx)?;
        if let Some(quantity) = self.sizing.calculate_quantity(ctx) {
            order.quantity = quantity;
        }
        Some(order)
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}
