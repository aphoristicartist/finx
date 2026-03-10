#[cfg(test)]
mod integration_tests {
    use ferrotick_backtest::{BacktestConfig, BacktestEngine};
    use ferrotick_core::{Bar, Symbol, UtcDateTime};
    use ferrotick_strategies::{
        MovingAverageCrossoverStrategy, OrderSide, SignalAction, Strategy as StratStrategy,
    };

    fn make_bars(count: usize) -> Vec<Bar> {
        let mut bars = Vec::with_capacity(count);
        for i in 0..count {
            let ts = UtcDateTime::parse(format!("2024-01-{:02}T00:00:00Z", (i % 28) + 1).as_str())
                .unwrap();
            let close = 100.0 + i as f64 * 0.5;
            bars.push(
                Bar::new(
                    ts,
                    close - 1.0,
                    close + 1.0,
                    close - 2.0,
                    close,
                    Some(1_000),
                    None,
                )
                .unwrap(),
            );
        }
        bars
    }

    #[test]
    fn test_cross_crate_compilation() {
        // Test that all major crates can be imported and used together

        // Core functionality
        let bars = make_bars(50);
        assert_eq!(bars.len(), 50);

        // Strategy functionality
        let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
            .expect("valid MA crossover configuration should construct");
        assert_eq!(strategy.name(), "ma_crossover");
        let debug = format!("{strategy:?}");
        assert!(debug.contains("symbol: \"TEST\""));
        assert!(debug.contains("fast_period: 5"));
        assert!(debug.contains("slow_period: 10"));
        assert!(debug.contains("order_quantity: 100.0"));
        assert!(debug.contains("closes: []"));
        assert!(debug.contains("prev_fast: None"));
        assert!(debug.contains("prev_slow: None"));

        for bar in &bars[..9] {
            assert!(
                strategy.on_bar(bar).is_none(),
                "strategy should warm up before slow_period bars are available"
            );
        }

        let warmup_signal = strategy
            .on_bar(&bars[9])
            .expect("strategy should emit a signal at slow_period");
        assert_eq!(warmup_signal.symbol, "TEST");
        assert_eq!(warmup_signal.strategy_name, "ma_crossover");
        assert_eq!(warmup_signal.action, SignalAction::Hold);
        assert!(
            strategy.on_signal(&warmup_signal).is_none(),
            "hold signal should not produce an order"
        );

        let mut buy_signal = warmup_signal.clone();
        buy_signal.action = SignalAction::Buy;
        buy_signal.reason = String::from("integration buy check");
        let buy_order = strategy
            .on_signal(&buy_signal)
            .expect("buy signal should produce an order");
        assert_eq!(buy_order.symbol, "TEST");
        assert_eq!(buy_order.side, OrderSide::Buy);
        assert_eq!(buy_order.quantity, 100.0);
        assert_eq!(buy_order.reason, "integration buy check");

        // Backtest functionality
        let config = BacktestConfig::default();
        let engine = BacktestEngine::new(config);
        assert_eq!(engine.config().initial_capital, 100_000.0);
    }

    #[test]
    fn test_core_types_work_across_crates() {
        // Test that core types work consistently across crates
        let symbol = Symbol::parse("AAPL").unwrap();
        assert_eq!(symbol.as_str(), "AAPL");

        let bars = make_bars(20);
        assert!(bars[0].close > 0.0);

        // Test that we can create backtest config
        let config = BacktestConfig {
            initial_capital: 50_000.0,
            ..Default::default()
        };
        assert_eq!(config.initial_capital, 50_000.0);
    }
}
