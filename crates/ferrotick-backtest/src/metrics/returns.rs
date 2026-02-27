/// Compute simple returns from a sequence of equity values.
pub fn simple_returns(equity_values: &[f64]) -> Vec<f64> {
    if equity_values.len() < 2 {
        return Vec::new();
    }

    equity_values
        .windows(2)
        .map(|window| {
            let previous = window[0];
            let current = window[1];
            if previous.abs() <= f64::EPSILON {
                0.0
            } else {
                (current - previous) / previous
            }
        })
        .collect()
}

/// Total return over the full series.
pub fn total_return(equity_values: &[f64]) -> f64 {
    if equity_values.len() < 2 {
        return 0.0;
    }

    let first = equity_values[0];
    let last = equity_values[equity_values.len() - 1];

    if first.abs() <= f64::EPSILON {
        0.0
    } else {
        (last - first) / first
    }
}

/// Annualized return using trading-days convention.
pub fn annualized_return(total_return: f64, periods: usize, trading_days_per_year: f64) -> f64 {
    if periods == 0 || trading_days_per_year <= 0.0 {
        return 0.0;
    }

    let years = periods as f64 / trading_days_per_year;
    if years <= 0.0 {
        return 0.0;
    }

    (1.0 + total_return).powf(1.0 / years) - 1.0
}

/// Arithmetic mean.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Sample standard deviation (N-1 denominator).
pub fn sample_std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let mu = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - mu;
            delta * delta
        })
        .sum::<f64>()
        / (values.len() as f64 - 1.0);

    variance.sqrt()
}

/// Annualized volatility from periodic returns.
pub fn annualized_volatility(returns: &[f64], trading_days_per_year: f64) -> f64 {
    if returns.is_empty() || trading_days_per_year <= 0.0 {
        return 0.0;
    }

    sample_std_dev(returns) * trading_days_per_year.sqrt()
}
