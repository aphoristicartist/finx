use super::{Action, TradingEnvironment};

#[derive(Clone, Debug)]
pub struct RewardCalculator {
    config: RewardConfig,
}

#[derive(Clone, Debug)]
pub struct RewardConfig {
    pub transaction_cost_penalty: f64, // default 0.001
    pub risk_free_rate: f64,           // default 0.0
}

impl Default for RewardConfig {
    fn default() -> Self {
        Self {
            transaction_cost_penalty: 0.001,
            risk_free_rate: 0.0,
        }
    }
}

impl Default for RewardCalculator {
    fn default() -> Self {
        Self::new(RewardConfig::default())
    }
}

impl RewardCalculator {
    pub fn new(config: RewardConfig) -> Self {
        Self { config }
    }

    pub fn calculate(&self, prev_value: f64, curr_value: f64, action: Action) -> f64 {
        let portfolio_return = if prev_value.abs() > f64::EPSILON {
            (curr_value - prev_value) / prev_value
        } else {
            0.0
        };

        let action_penalty = match action {
            Action::Hold => 0.0,
            Action::Buy | Action::Sell => self.config.transaction_cost_penalty,
        };

        portfolio_return - action_penalty - self.config.risk_free_rate
    }

    pub fn sharpe_reward(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let excess_returns: Vec<f64> = returns
            .iter()
            .map(|value| *value - self.config.risk_free_rate)
            .collect();

        let mean = excess_returns.iter().sum::<f64>() / excess_returns.len() as f64;
        let variance = excess_returns
            .iter()
            .map(|value| {
                let centered = *value - mean;
                centered * centered
            })
            .sum::<f64>()
            / excess_returns.len() as f64;

        let std_dev = variance.sqrt();
        if std_dev <= f64::EPSILON {
            0.0
        } else {
            mean / std_dev
        }
    }

    pub fn calculate_portfolio_value(env: &TradingEnvironment) -> f64 {
        env.portfolio_value()
    }
}
