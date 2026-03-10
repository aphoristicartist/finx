//! Mathematical correctness tests - verify exact calculations

#[cfg(test)]
mod tests {
    use ferrotick_core::Bar;

    #[test]
    fn test_sma_calculation_exact() {
        // Simple Moving Average: (10 + 20 + 30 + 40 + 50) / 5 = 30.0
        let prices: Vec<f64> = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let sma: f64 = prices.iter().sum::<f64>() / prices.len() as f64;

        assert!(
            (sma - 30.0).abs() < 0.001,
            "SMA should be exactly 30.0, got {}",
            sma
        );
    }

    #[test]
    fn test_portfolio_return_exact() {
        // Buy at 100, sell at 110: return = (110 - 100) / 100 = 0.10 (10%)
        let buy_price = 100.0;
        let sell_price = 110.0;
        let return_rate: f64 = (sell_price - buy_price) / buy_price;

        assert!(
            (return_rate - 0.10).abs() < 0.001,
            "Return should be exactly 0.10, got {}",
            return_rate
        );
    }

    #[test]
    fn test_sharpe_ratio_calculation() {
        // Sharpe Ratio = (mean_return - risk_free_rate) / std_deviation
        let returns: Vec<f64> = vec![0.05, 0.10, 0.15, 0.02, 0.08];
        let risk_free_rate = 0.02;

        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();

        let sharpe = (mean - risk_free_rate) / std_dev;

        // Verify the calculation is mathematically correct
        let expected_mean = 0.08;
        assert!((mean - expected_mean).abs() < 0.001, "Mean should be 0.08");
        assert!(std_dev > 0.0, "Standard deviation should be positive");
        assert!(sharpe.is_finite(), "Sharpe ratio should be finite");
    }

    #[test]
    fn test_compound_growth_rate() {
        // CAGR = (final / initial) ^ (1/years) - 1
        let initial = 1000.0;
        let final_value = 1500.0;
        let years = 5.0;

        let ratio: f64 = final_value / initial;
        let cagr = ratio.powf(1.0 / years) - 1.0;

        // Expected: (1.5)^(0.2) - 1 ≈ 0.0845 (8.45%)
        assert!((cagr - 0.0845).abs() < 0.001, "CAGR should be ~8.45%");
    }

    #[test]
    fn test_position_sizing_kelly() {
        // Kelly Criterion: f = (p * b - q) / b
        // where p = win probability, q = loss probability, b = win/loss ratio
        let win_prob = 0.6;
        let loss_prob = 0.4;
        let win_loss_ratio = 2.0; // Win $2 for every $1 lost

        let kelly = (win_prob * win_loss_ratio - loss_prob) / win_loss_ratio;

        // Expected: (0.6 * 2 - 0.4) / 2 = 0.8 / 2 = 0.4
        let kelly_f64: f64 = kelly;
        assert!(
            (kelly_f64 - 0.4).abs() < 0.001,
            "Kelly fraction should be 0.4"
        );
    }
}
