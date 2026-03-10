//! State management tests - verify portfolio state transitions

#[cfg(test)]
mod tests {
    use ferrotick_backtest::{Fill, OrderSide, Portfolio};
    use ferrotick_core::Symbol;
    use ferrotick_core::UtcDateTime;
    use uuid::Uuid;

    #[test]
    fn test_cash_never_negative() {
        let mut portfolio = Portfolio::new(1000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Attempt to buy more than we can afford
        // This should fail gracefully, not make cash negative
        let fill = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            100.0, // quantity
            20.0,  // price (cost = 2000, but only have 1000)
            0.0,   // fees
            0.0,   // slippage
            UtcDateTime::now(),
        );

        let result = portfolio.apply_fill(&fill);

        // Cash should never go negative
        let cash = portfolio.cash();
        assert!(cash >= 0.0, "Cash must never be negative, got {}", cash);

        // The purchase should have failed
        assert!(result.is_err(), "Oversized purchase should fail");
    }

    #[test]
    fn test_position_tracking() {
        let mut portfolio = Portfolio::new(10000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Buy 100 shares at $10
        let fill1 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            100.0,
            10.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill1).expect("Buy should succeed");

        let pos1 = portfolio.position(&symbol);
        assert!(
            (pos1 - 100.0).abs() < 0.001,
            "Position should be 100 after first buy, got {}",
            pos1
        );

        // Sell 30 shares at $11
        let fill2 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Sell,
            30.0,
            11.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill2).expect("Sell should succeed");

        let pos2 = portfolio.position(&symbol);
        assert!(
            (pos2 - 70.0).abs() < 0.001,
            "Position should be 70 after sell, got {}",
            pos2
        );

        // Buy 50 more shares at $12
        let fill3 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            50.0,
            12.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio
            .apply_fill(&fill3)
            .expect("Second buy should succeed");

        let pos3 = portfolio.position(&symbol);
        assert!(
            (pos3 - 120.0).abs() < 0.001,
            "Final position should be 120, got {}",
            pos3
        );
    }

    #[test]
    fn test_cash_updates_on_trade() {
        let mut portfolio = Portfolio::new(10000.0);
        let symbol = Symbol::parse("AAPL").unwrap();
        let initial_cash = portfolio.cash();

        // Buy shares
        let fill1 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            10.0,
            150.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill1).expect("Buy should succeed");

        let cash_after_buy = portfolio.cash();

        // Cash should decrease by cost of shares
        assert!(
            cash_after_buy < initial_cash,
            "Cash should decrease after buy"
        );
        let expected_cash = initial_cash - (10.0 * 150.0);
        assert!(
            (cash_after_buy - expected_cash).abs() < 0.01,
            "Cash should be {}, got {}",
            expected_cash,
            cash_after_buy
        );

        // Sell shares
        let fill2 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Sell,
            10.0,
            160.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill2).expect("Sell should succeed");

        let cash_after_sell = portfolio.cash();

        // Cash should increase from sale proceeds
        assert!(
            cash_after_sell > cash_after_buy,
            "Cash should increase after sell"
        );
    }

    #[test]
    fn test_portfolio_value_calculation() {
        let mut portfolio = Portfolio::new(10000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Buy shares
        let fill = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            100.0,
            10.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill).expect("Buy should succeed");

        // Update price to $12
        portfolio.update_price(&symbol, 12.0);

        // Get portfolio equity
        let equity = portfolio.equity();

        // Equity = cash + position_value
        // Cash = 10000 - (100 * 10) = 9000
        // Position = 100 * 12 = 1200
        // Total = 10200
        assert!(
            equity > 10000.0,
            "Portfolio equity should reflect gains, got {}",
            equity
        );
    }
}
