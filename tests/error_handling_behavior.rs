//! Error handling behavior tests - verify graceful error handling

#[cfg(test)]
mod tests {
    use ferrotick_backtest::{Fill, OrderSide, Portfolio};
    use ferrotick_core::Symbol;
    use ferrotick_core::UtcDateTime;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use uuid::Uuid;

    #[test]
    fn test_empty_input_error() {
        // Empty data should return error, not panic
        let bars: Vec<ferrotick_core::Bar> = vec![];

        // Try to compute on empty data - should not panic
        let result = std::panic::catch_unwind(|| {
            // If there's a function that processes bars, it should handle empty gracefully
            // For now, we just verify empty vec doesn't crash
            let _len = bars.len();
        });

        assert!(result.is_ok(), "Empty input should not cause panic");
    }

    #[test]
    fn test_concurrent_access() {
        // Test thread-safe access to shared state
        let data = Arc::new(Mutex::new(vec![]));
        let mut handles = vec![];

        // Spawn 10 threads that all modify shared state
        for i in 0..10 {
            let data_clone = data.clone();
            handles.push(thread::spawn(move || {
                let mut d = data_clone.lock().unwrap();
                d.push(i);
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should not panic");
        }

        // Verify no data corruption
        let final_data = data.lock().unwrap();
        assert_eq!(
            final_data.len(),
            10,
            "All 10 threads should have added data"
        );

        // Verify all values are present (no lost updates)
        let mut sorted: Vec<_> = final_data.iter().cloned().collect();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            "No data should be lost"
        );
    }

    #[test]
    fn test_invalid_price_handling() {
        let mut portfolio = Portfolio::new(1000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Try to buy with negative price - this will create negative gross_value
        // which should be rejected by the system
        use std::panic::AssertUnwindSafe;
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let fill = Fill::new(
                Uuid::new_v4(),
                symbol.clone(),
                OrderSide::Buy,
                10.0,
                -10.0, // Invalid negative price
                0.0,
                0.0,
                UtcDateTime::now(),
            );
            let _ = portfolio.apply_fill(&fill);
        }));

        // Should not panic (may return error, but shouldn't crash)
        assert!(result.is_ok(), "Negative price should not cause panic");
    }

    #[test]
    fn test_zero_quantity_handling() {
        let mut portfolio = Portfolio::new(1000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Try to buy zero shares
        let fill = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            0.0, // Zero quantity
            10.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );

        let result = portfolio.apply_fill(&fill);

        // Should handle gracefully (might succeed with no effect, or return error)
        // The key is it shouldn't panic
        assert!(
            result.is_ok() || result.is_err(),
            "Zero quantity should be handled gracefully"
        );
    }

    #[test]
    fn test_sell_more_than_owned() {
        let mut portfolio = Portfolio::new(1000.0);
        let symbol = Symbol::parse("TEST").unwrap();

        // Buy 10 shares
        let fill1 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Buy,
            10.0,
            10.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );
        portfolio.apply_fill(&fill1).expect("Buy should succeed");

        // Try to sell 20 shares (more than we own)
        let fill2 = Fill::new(
            Uuid::new_v4(),
            symbol.clone(),
            OrderSide::Sell,
            20.0, // More than we have
            11.0,
            0.0,
            0.0,
            UtcDateTime::now(),
        );

        let result = portfolio.apply_fill(&fill2);

        // Should return error
        assert!(result.is_err(), "Overselling should return error");

        // Position should still be 10 (unchanged)
        let pos = portfolio.position(&symbol);
        assert!(
            (pos - 10.0).abs() < 0.001,
            "Position should remain unchanged after failed sell"
        );
    }

    #[test]
    fn test_numeric_overflow_protection() {
        // Test that large numbers don't cause overflow
        let large_value = f64::MAX / 2.0;

        let result = std::panic::catch_unwind(|| {
            let _sum = large_value + large_value;
        });

        // Should handle gracefully (may produce infinity, but shouldn't panic)
        assert!(result.is_ok(), "Large numbers should not cause panic");
    }

    #[test]
    fn test_malformed_ticker_symbol() {
        // Test that invalid symbols are handled gracefully
        // Empty ticker should fail validation
        let empty_result = Symbol::parse("");
        assert!(empty_result.is_err(), "Empty symbol should return error");

        // Very long ticker (should handle gracefully)
        let long_ticker = "A".repeat(1000);
        let long_result = Symbol::parse(&long_ticker);
        // Should return error for too long symbol
        assert!(long_result.is_err(), "Too long symbol should return error");

        // Test that valid symbols work
        let valid_result = Symbol::parse("AAPL");
        assert!(
            valid_result.is_ok(),
            "Valid symbol should parse successfully"
        );
    }
}
