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
