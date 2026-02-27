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
