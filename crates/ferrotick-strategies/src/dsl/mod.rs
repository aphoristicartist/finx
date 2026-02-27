pub mod parser;
pub mod validator;

use std::path::Path;

use crate::sizing::position::{build_sizer, PositionSizingConfig, PositionSizingMethod};
use crate::strategies::{
    BollingerBandSqueezeStrategy, MacdTrendStrategy, MovingAverageCrossoverStrategy,
    RsiMeanReversionStrategy,
};
use crate::traits::strategy::Strategy;
use crate::{StrategyError, StrategyResult};

pub use parser::{IndicatorRule, StrategySpec};
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
    match spec.strategy_type.as_str() {
        "trend_following" | "ma_crossover" => {
            let fast = extract_period(&spec.entry_rules, "fast_period", 10);
            let slow = extract_period(&spec.entry_rules, "slow_period", 20);
            let qty = spec.position_sizing.value;
            Ok(Box::new(MovingAverageCrossoverStrategy::new(
                symbol, fast, slow, qty,
            )?))
        }
        "mean_reversion" | "rsi_mean_reversion" => {
            let period = extract_period(&spec.entry_rules, "period", 14);
            let oversold = extract_value(&spec.entry_rules, "<", 30.0);
            let overbought = extract_value(&spec.exit_rules, ">", 70.0);
            let qty = spec.position_sizing.value;
            Ok(Box::new(RsiMeanReversionStrategy::new(
                symbol, period, oversold, overbought, qty,
            )?))
        }
        "macd_trend" => {
            let fast = extract_period(&spec.entry_rules, "fast_period", 12);
            let slow = extract_period(&spec.entry_rules, "slow_period", 26);
            let signal = extract_period(&spec.entry_rules, "signal_period", 9);
            let qty = spec.position_sizing.value;
            Ok(Box::new(MacdTrendStrategy::new(
                symbol, fast, slow, signal, qty,
            )?))
        }
        "bb_squeeze" => {
            let period = extract_period(&spec.entry_rules, "period", 20);
            let num_std = extract_value(&spec.entry_rules, "num_std", 2.0);
            let qty = spec.position_sizing.value;
            Ok(Box::new(BollingerBandSqueezeStrategy::new(
                symbol, period, num_std, qty,
            )?))
        }
        _ => Err(StrategyError::InvalidConfig(format!(
            "unknown strategy type: {}",
            spec.strategy_type
        ))),
    }
}

pub fn build_position_sizer(
    spec: &StrategySpec,
) -> StrategyResult<Box<dyn crate::sizing::position::PositionSizer>> {
    let method = match spec.position_sizing.method.as_str() {
        "fixed" => PositionSizingMethod::Fixed,
        "percent" => PositionSizingMethod::Percent,
        "volatility" => PositionSizingMethod::Volatility,
        "kelly" => PositionSizingMethod::Kelly,
        _ => PositionSizingMethod::Percent,
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
        .map(|r| r.value)
        .unwrap_or(default)
}
