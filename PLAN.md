# Task: Implement Ferrotick Phase 12 - AI-Powered Features

## Objective
Create the `ferrotick-ai` crate with LLM integration for natural language strategy development and backtest reporting.

## Requirements
1. Create `ferrotick-ai` crate with proper Cargo.toml dependencies
2. Implement OpenAI client wrapper using async-openai
3. Implement strategy compiler (NL → StrategySpec JSON)
4. Implement backtest reporter (PerformanceMetrics → natural language explanation)
5. Add prompt templates for finance/trading contexts
6. Add output validation and JSON sanitization
7. Add basic unit tests
8. Update workspace Cargo.toml to include new crate

## Step-by-Step Implementation

### Step 1: Create crate structure
**Action:** Create new Rust crate
```bash
cd /tmp/tri-goose/crates && cargo new ferrotick-ai --lib
```

### Step 2: Update workspace Cargo.toml
**File:** `/tmp/tri-goose/Cargo.toml`
**Action:** Add `ferrotick-ai` to workspace members
**Location:** In the `members` array
```toml
[workspace]
members = [
  "crates/ferrotick-core",
  "crates/ferrotick-cli",
  "crates/ferrotick-warehouse",
  "crates/ferrotick-agent",
  "crates/ferrotick-ml",
  "crates/ferrotick-backtest",
  "crates/ferrotick-optimization",
  "crates/ferrotick-ai",
  "crates/ferrotick-strategies",
]
```

### Step 3: Create ferrotick-ai Cargo.toml
**File:** `/tmp/tri-goose/crates/ferrotick-ai/Cargo.toml`
**Action:** Replace entire file
```toml
[package]
name = "ferrotick-ai"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-backtest = { path = "../ferrotick-backtest" }
ferrotick-strategies = { path = "../ferrotick-strategies" }
async-openai = "0.20"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
regex = "1.10"

[dev-dependencies]
tokio-test = "0.4"
```

### Step 4: Create error.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/error.rs`
**Action:** Create new file
```rust
use thiserror::Error;

/// Errors that can occur in AI operations.
#[derive(Debug, Error)]
pub enum AIError {
    #[error("OpenAI API error: {0}")]
    OpenAI(#[from] async_openai::error::OpenAIError),

    #[error("JSON parsing error: {0}")]
    Parsing(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Result type for AI operations.
pub type AIResult<T> = Result<T, AIError>;
```

### Step 5: Create llm/mod.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/llm/mod.rs`
**Action:** Create new file
```rust
mod openai;

use async_trait::async_trait;
pub use openai::OpenAIClient;

use crate::error::AIResult;

/// Trait for LLM clients.
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Complete a prompt with optional system message.
    async fn complete(&self, prompt: &str, system: Option<&str>) -> AIResult<String>;
}
```

### Step 6: Create llm/openai.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/llm/openai.rs`
**Action:** Create new file
```rust
use async_openai::{
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;

use super::LLMClient;
use crate::error::{AIError, AIResult};

/// OpenAI API client wrapper.
pub struct OpenAIClient {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
}

impl OpenAIClient {
    /// Create a new OpenAI client with optional API key override.
    pub fn new(api_key: Option<String>, model: String) -> Self {
        let client = if let Some(key) = api_key {
            let config = async_openai::config::OpenAIConfig::new().with_api_key(key);
            Client::with_config(config)
        } else {
            Client::new()
        };
        Self { client, model }
    }

    /// Create client with default model (gpt-4o).
    pub fn with_default_model() -> Self {
        Self::new(None, "gpt-4o".to_string())
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> AIResult<String> {
        let mut messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        if let Some(sys) = system {
            messages.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(sys)
                    .build()
                    .map_err(|e| AIError::Parsing(e.to_string()))?
                    .into(),
            );
        }

        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()
                .map_err(|e| AIError::Parsing(e.to_string()))?
                .into(),
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(|e| AIError::Parsing(e.to_string()))?;

        let response = self.client.chat().create(request).await?;

        response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .ok_or_else(|| AIError::Parsing("No response content".to_string()))
    }
}
```

### Step 7: Create compiler/mod.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/compiler/mod.rs`
**Action:** Create new file
```rust
mod strategy;

pub use strategy::StrategyCompiler;
```

### Step 8: Create compiler/strategy.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/compiler/strategy.rs`
**Action:** Create new file
```rust
use ferrotick_strategies::dsl::{IndicatorRule, PositionSizingSpec, RuleValue, StrategySpec};

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

/// Raw strategy spec for LLM output parsing (uses String for value to be more flexible).
#[derive(Debug, Clone, serde::Deserialize)]
struct RawStrategySpec {
    name: String,
    #[serde(rename = "type")]
    strategy_type: String,
    timeframe: String,
    #[serde(default)]
    entry_rules: Vec<RawIndicatorRule>,
    #[serde(default)]
    exit_rules: Vec<RawIndicatorRule>,
    position_sizing: PositionSizingSpec,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RawIndicatorRule {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    condition: Option<String>,
    pub indicator: String,
    #[serde(default)]
    period: Option<usize>,
    pub operator: String,
    pub value: serde_json::Value,
    pub action: String,
}
```

### Step 9: Create reporting/mod.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/reporting/mod.rs`
**Action:** Create new file
```rust
mod backtest;
pub mod templates;

pub use backtest::BacktestReporter;
```

### Step 10: Create reporting/backtest.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/reporting/backtest.rs`
**Action:** Create new file
```rust
use ferrotick_backtest::metrics::PerformanceMetrics;

use crate::error::AIResult;
use crate::llm::LLMClient;

/// Generates natural language explanations of backtest results.
pub struct BacktestReporter {
    llm: Box<dyn LLMClient>,
}

impl BacktestReporter {
    /// Create a new backtest reporter with the given LLM client.
    pub fn new(llm: Box<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// Generate a natural language explanation of backtest metrics.
    pub async fn explain(&self, metrics: &PerformanceMetrics) -> AIResult<String> {
        let prompt = format!(
            r#"Explain these backtest results in plain English:

Total Return: {:.2}%
Annualized Return: {:.2}%
Volatility: {:.2}%
Sharpe Ratio: {:.2}
Max Drawdown: {:.2}%

Provide:
1. **Overall Assessment** (2-3 sentences on strategy performance)
2. **Strengths** (bullet points)
3. **Weaknesses** (bullet points)
4. **Recommendations** (actionable improvements)

Keep the explanation concise and practical."#,
            metrics.total_return() * 100.0,
            metrics.annualized_return() * 100.0,
            metrics.volatility() * 100.0,
            metrics.sharpe_ratio(0.02), // Assume 2% risk-free rate
            metrics.max_drawdown() * 100.0,
        );

        self.llm
            .complete(
                &prompt,
                Some("You are a quantitative trading expert. Be concise and practical."),
            )
            .await
    }

    /// Generate a risk analysis of the strategy.
    pub async fn analyze_risk(&self, metrics: &PerformanceMetrics) -> AIResult<String> {
        let prompt = format!(
            r#"Analyze the risk profile of this trading strategy:

Sharpe Ratio: {:.2}
Max Drawdown: {:.2}%
Volatility: {:.2}%
VaR (95%): {:.2}%
CVaR (95%): {:.2}%

Provide:
1. **Risk Assessment** (overall risk level: Low/Medium/High)
2. **Key Risks** (identify the main sources of risk)
3. **Mitigation Strategies** (specific recommendations)

Be concise and actionable."#,
            metrics.sharpe_ratio(0.02),
            metrics.max_drawdown() * 100.0,
            metrics.volatility() * 100.0,
            metrics.var(0.95) * 100.0,
            metrics.cvar(0.95) * 100.0,
        );

        self.llm
            .complete(
                &prompt,
                Some("You are a risk management expert. Be concise and actionable."),
            )
            .await
    }
}
```

### Step 11: Create reporting/templates.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/reporting/templates.rs`
**Action:** Create new file
```rust
/// Prompt template for strategy creation.
pub const STRATEGY_PROMPT: &str = r#"
Create a trading strategy based on the following description: {description}

Output a YAML strategy specification with:
- name: strategy name (snake_case)
- type: strategy type (one of: trend_following, mean_reversion, macd_trend, bb_squeeze)
- timeframe: trading timeframe
- entry_rules: array of entry conditions
- exit_rules: array of exit conditions
- position_sizing: {{ method, value }}
"#;

/// Prompt template for backtest explanation.
pub const BACKTEST_PROMPT: &str = r#"
Explain these backtest results in plain English:

{metrics}

Provide assessment, strengths, weaknesses, and recommendations.
"#;

/// Prompt template for risk analysis.
pub const RISK_PROMPT: &str = r#"
Analyze the risk profile of this strategy:

Sharpe Ratio: {sharpe}
Max Drawdown: {drawdown}
Volatility: {volatility}

Identify key risks and mitigation strategies.
"#;

/// Prompt template for strategy optimization suggestions.
pub const OPTIMIZATION_PROMPT: &str = r#"
Given the current strategy parameters and backtest results:
{current_params}
{results}

Suggest parameter optimizations to improve:
1. Risk-adjusted returns (Sharpe ratio)
2. Maximum drawdown
3. Win rate

Provide specific parameter ranges to explore.
"#;
```

### Step 12: Create validation/mod.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/validation/mod.rs`
**Action:** Create new file
```rust
mod sanitizer;

pub use sanitizer::OutputSanitizer;
```

### Step 13: Create validation/sanitizer.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/validation/sanitizer.rs`
**Action:** Create new file
```rust
use regex::Regex;

use crate::error::{AIError, AIResult};

/// Sanitizes and validates LLM outputs.
pub struct OutputSanitizer;

impl OutputSanitizer {
    /// Extract and clean JSON from LLM output.
    pub fn sanitize_json(input: &str) -> AIResult<String> {
        // Remove markdown code blocks
        let re = Regex::new(r"```(?:json)?\s*(.*?)\s*```").unwrap();
        let cleaned = re.replace(input, "$1");

        // Find JSON object boundaries
        let start = cleaned
            .find('{')
            .ok_or_else(|| AIError::Parsing("No JSON object found".into()))?;
        let end = cleaned
            .rfind('}')
            .ok_or_else(|| AIError::Parsing("No JSON object found".into()))?;

        Ok(cleaned[start..=end].to_string())
    }

    /// Extract and clean YAML from LLM output.
    pub fn sanitize_yaml(input: &str) -> AIResult<String> {
        // Remove markdown code blocks
        let re = Regex::new(r"```(?:yaml|yml)?\s*(.*?)\s*```").unwrap();
        let mut cleaned = re.replace(input, "$1").to_string();

        // If no code blocks, use the raw input
        if cleaned.is_empty() {
            cleaned = input.to_string();
        }

        // Trim whitespace
        cleaned = cleaned.trim().to_string();

        if cleaned.is_empty() {
            return Err(AIError::Parsing("No YAML content found".into()));
        }

        Ok(cleaned)
    }

    /// Validate that a string is valid JSON and contains expected structure.
    pub fn validate_json_structure(json: &str, required_keys: &[&str]) -> AIResult<()> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| AIError::Parsing(e.to_string()))?;

        if !value.is_object() {
            return Err(AIError::Validation("JSON must be an object".into()));
        }

        for key in required_keys {
            if !value.as_object().map_or(false, |obj| obj.contains_key(*key)) {
                return Err(AIError::Validation(format!(
                    "Missing required key: {}",
                    key
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_json_with_code_block() {
        let input = r#"Here's the JSON:
```json
{"name": "test", "value": 42}
```
That's it!"#;

        let cleaned = OutputSanitizer::sanitize_json(input).unwrap();
        assert!(cleaned.starts_with('{'));
        assert!(cleaned.ends_with('}'));
        assert!(cleaned.contains("\"name\""));
    }

    #[test]
    fn test_sanitize_json_without_code_block() {
        let input = r#"Some text {"name": "test"} more text"#;

        let cleaned = OutputSanitizer::sanitize_json(input).unwrap();
        assert_eq!(cleaned, r#"{"name": "test"}"#);
    }

    #[test]
    fn test_sanitize_yaml_with_code_block() {
        let input = r#"```yaml
name: test
value: 42
```"#;

        let cleaned = OutputSanitizer::sanitize_yaml(input).unwrap();
        assert!(cleaned.contains("name: test"));
    }

    #[test]
    fn test_validate_json_structure() {
        let json = r#"{"name": "test", "value": 42}"#;
        assert!(OutputSanitizer::validate_json_structure(json, &["name", "value"]).is_ok());
        assert!(OutputSanitizer::validate_json_structure(json, &["missing"]).is_err());
    }
}
```

### Step 14: Create main lib.rs
**File:** `/tmp/tri-goose/crates/ferrotick-ai/src/lib.rs`
**Action:** Replace entire file
```rust
//! AI-powered features for Ferrotick.
//!
//! This crate provides LLM integration for natural language strategy development
//! and backtest reporting.

pub mod compiler;
pub mod error;
pub mod llm;
pub mod reporting;
pub mod validation;

pub use compiler::StrategyCompiler;
pub use error::{AIError, AIResult};
pub use llm::{LLMClient, OpenAIClient};
pub use reporting::{templates, BacktestReporter};
pub use validation::OutputSanitizer;

// Re-export for convenience
pub use ferrotick_strategies::dsl::StrategySpec;
```

### Step 15: Add async-trait dependency
**File:** `/tmp/tri-goose/crates/ferrotick-ai/Cargo.toml`
**Action:** Add async-trait to dependencies
```toml
async-trait = "0.1"
serde_yaml = "0.9"
```

### Step 16: Run cargo check
```bash
cd /tmp/tri-goose && cargo check -p ferrotick-ai
```

### Step 17: Run tests
```bash
cd /tmp/tri-goose && cargo test -p ferrotick-ai
```

### Step 18: Run workspace check
```bash
cd /tmp/tri-goose && cargo check --workspace
```

## Acceptance Criteria
- [ ] `cargo check --workspace` passes
- [ ] `cargo test -p ferrotick-ai` passes
- [ ] `ferrotick-ai` crate compiles with all dependencies

## Out of Scope
- Anthropic client (P1)
- Ollama client (P1)
- Code generation (P1)
- CLI commands (P1)
