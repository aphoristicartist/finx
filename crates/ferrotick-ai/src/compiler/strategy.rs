use ferrotick_strategies::dsl::{StrategySpec};

use crate::error::{AIError, AIResult};
use crate::llm::LLMClient;
use crate::validation::OutputSanitizer;

/// Compiles natural language descriptions into structured strategy specifications.
pub struct StrategyCompiler {
    llm: Box<dyn LLMClient>,
}

impl StrategyCompiler {
    /// Create a new strategy compiler with the given LLM client.
    pub fn new(llm: Box<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// Compile a natural language description into a StrategySpec.
    pub async fn compile(&self, description: &str) -> AIResult<StrategySpec> {
        let prompt = format!(
            r#"Convert the following trading strategy description into a structured strategy specification.

Description: {}

Output a YAML object with:
- name: strategy name (snake_case)
- type: strategy type (one of: trend_following, mean_reversion, macd_trend, bb_squeeze)
- timeframe: trading timeframe (e.g., "1h", "4h", "1d")
- entry_rules: array of entry conditions (each with indicator, operator, value, action)
- exit_rules: array of exit conditions
- position_sizing: {{ method: "fixed"|"percent"|"volatility"|"kelly", value: number }}

Example for RSI mean reversion:
```yaml
name: rsi_mean_reversion
type: mean_reversion
timeframe: "4h"
entry_rules:
  - indicator: rsi
    period: 14
    operator: "<"
    value: 30
    action: buy
exit_rules:
  - indicator: rsi
    period: 14
    operator: ">"
    value: 70
    action: sell
position_sizing:
  method: percent
  value: 0.02
```

Output ONLY the YAML, no additional text."#,
            description
        );

        let response = self
            .llm
            .complete(&prompt, Some("You are a trading strategy expert."))
            .await?;

        // Sanitize and parse the response
        let cleaned = OutputSanitizer::sanitize_yaml(&response)?;
        let spec: StrategySpec = serde_yaml::from_str(&cleaned)
            .map_err(|e| AIError::Parsing(format!("Failed to parse strategy spec: {}", e)))?;

        // Validate spec
        self.validate_spec(&spec)?;

        Ok(spec)
    }

    fn validate_spec(&self, spec: &StrategySpec) -> AIResult<()> {
        if spec.name.is_empty() {
            return Err(AIError::Validation("Strategy name cannot be empty".into()));
        }
        if spec.strategy_type.is_empty() {
            return Err(AIError::Validation("Strategy type cannot be empty".into()));
        }
        Ok(())
    }
}
