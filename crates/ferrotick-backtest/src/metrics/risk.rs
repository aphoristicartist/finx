use crate::metrics::{drawdown, returns, EquityPoint};

/// Performance and risk metrics derived from an equity curve.
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    returns: Vec<f64>,
    equity_curve: Vec<f64>,
    trading_days_per_year: f64,
}

impl PerformanceMetrics {
    pub fn from_equity_curve(equity_curve: &[EquityPoint], trading_days_per_year: f64) -> Self {
        let equity_values: Vec<f64> = equity_curve.iter().map(|point| point.equity).collect();
        let returns = returns::simple_returns(&equity_values);

        Self {
            returns,
            equity_curve: equity_values,
            trading_days_per_year,
        }
    }

    pub fn total_return(&self) -> f64 {
        returns::total_return(&self.equity_curve)
    }

    pub fn annualized_return(&self) -> f64 {
        returns::annualized_return(
            self.total_return(),
            self.returns.len(),
            self.trading_days_per_year,
        )
    }

    pub fn volatility(&self) -> f64 {
        returns::annualized_volatility(&self.returns, self.trading_days_per_year)
    }

    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> f64 {
        let vol = self.volatility();
        if vol <= f64::EPSILON {
            0.0
        } else {
            (self.annualized_return() - risk_free_rate) / vol
        }
    }

    pub fn sortino_ratio(&self, risk_free_rate: f64) -> f64 {
        let downside = self.downside_deviation();
        if downside <= f64::EPSILON {
            0.0
        } else {
            (self.annualized_return() - risk_free_rate) / downside
        }
    }

    fn downside_deviation(&self) -> f64 {
        if self.returns.is_empty() || self.trading_days_per_year <= 0.0 {
            return 0.0;
        }

        let downside_sq_sum: f64 = self
            .returns
            .iter()
            .map(|r| if *r < 0.0 { r * r } else { 0.0 })
            .sum();

        (downside_sq_sum / self.returns.len() as f64).sqrt() * self.trading_days_per_year.sqrt()
    }

    pub fn max_drawdown(&self) -> f64 {
        drawdown::max_drawdown_from_values(&self.equity_curve)
    }

    /// Historical VaR at confidence level (e.g., 0.95).
    pub fn var(&self, confidence: f64) -> f64 {
        if self.returns.is_empty() {
            return 0.0;
        }

        let confidence = confidence.clamp(0.0, 1.0);
        let mut sorted = self.returns.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));

        let tail_prob = 1.0 - confidence;
        let index = (tail_prob * (sorted.len() as f64 - 1.0)).floor() as usize;
        sorted[index.min(sorted.len() - 1)]
    }

    /// Historical CVaR / Expected Shortfall at confidence level.
    pub fn cvar(&self, confidence: f64) -> f64 {
        if self.returns.is_empty() {
            return 0.0;
        }

        let threshold = self.var(confidence);
        let tail: Vec<f64> = self
            .returns
            .iter()
            .copied()
            .filter(|ret| *ret <= threshold)
            .collect();

        if tail.is_empty() {
            threshold
        } else {
            tail.iter().sum::<f64>() / tail.len() as f64
        }
    }
}
