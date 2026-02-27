use serde::Deserialize;

use crate::{StrategyError, StrategyResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrategySpec {
    pub name: String,
    #[serde(rename = "type")]
    pub strategy_type: String,
    pub timeframe: String,
    #[serde(default)]
    pub entry_rules: Vec<IndicatorRule>,
    #[serde(default)]
    pub exit_rules: Vec<IndicatorRule>,
    pub position_sizing: PositionSizingSpec,
    #[serde(default)]
    pub risk_management: Option<RiskManagementSpec>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndicatorRule {
    pub indicator: String,
    #[serde(default)]
    pub period: Option<usize>,
    pub operator: String,
    pub value: f64,
    pub action: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionSizingSpec {
    pub method: String,
    pub value: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskManagementSpec {
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
}

pub fn parse_strategy_yaml(raw: &str) -> StrategyResult<StrategySpec> {
    serde_yaml::from_str(raw).map_err(|e| StrategyError::YamlParse(e.to_string()))
}
