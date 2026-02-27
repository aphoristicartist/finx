use ferrotick_core::{Bar, UtcDateTime};
use ferrotick_strategies::dsl::parse_and_validate_strategy_yaml;
use ferrotick_strategies::strategies::{
    BollingerBandSqueezeStrategy, MacdTrendStrategy, MovingAverageCrossoverStrategy,
    RsiMeanReversionStrategy,
};
use ferrotick_strategies::traits::strategy::{SignalAction, Strategy};
use ferrotick_strategies::sizing::position::{FixedSizer, PercentSizer, PositionSizer, PositionSizingContext};

fn make_bar(close: f64, ts_days: i64) -> Bar {
    let ts = UtcDateTime::from_unix_timestamp(ts_days * 86400).expect("valid timestamp");
    Bar::new(
        ts,
        close,
        close,
        close,
        close,
        Some(1000),
        None,
    ).expect("valid bar")
}

// ============================================================================
// MA Crossover Tests
// ============================================================================

#[test]
fn test_ma_crossover_construction() {
    let strategy = MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 1.0);
    assert!(strategy.is_ok());
    let strategy = strategy.unwrap();
    assert_eq!(strategy.name(), "ma_crossover");
}

#[test]
fn test_ma_crossover_invalid_params() {
    // fast_period == 0
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 0, 10, 1.0).is_err());
    // slow_period == 0
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 5, 0, 1.0).is_err());
    // fast_period >= slow_period
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 10, 10, 1.0).is_err());
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 15, 10, 1.0).is_err());
    // invalid order_quantity
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 0.0).is_err());
    assert!(MovingAverageCrossoverStrategy::new("AAPL", 5, 10, -1.0).is_err());
}

#[test]
fn test_ma_crossover_warmup() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 1.0).unwrap();
    
    // Should return None during warmup
    for i in 0..9 {
        let bar = make_bar(100.0 + i as f64, i);
        assert!(strategy.on_bar(&bar).is_none());
    }
    
    // Should return a signal after warmup
    let bar = make_bar(110.0, 10);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
    let signal = signal.unwrap();
    assert_eq!(signal.strategy_name, "ma_crossover");
}

#[test]
fn test_ma_crossover_golden_cross() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    
    // Build initial data
    for i in 0..5 {
        let bar = make_bar(100.0, i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Now drop price to create death cross
    for i in 5..10 {
        let bar = make_bar(90.0, i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Now rally to create golden cross (fast > slow)
    let bar = make_bar(120.0, 10);
    let signal = strategy.on_bar(&bar);
    // Signal depends on previous state, should generate something
    assert!(signal.is_some());
}

// ============================================================================
// RSI Mean Reversion Tests
// ============================================================================

#[test]
fn test_rsi_construction() {
    let strategy = RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 70.0, 1.0);
    assert!(strategy.is_ok());
    let strategy = strategy.unwrap();
    assert_eq!(strategy.name(), "rsi_mean_reversion");
}

#[test]
fn test_rsi_invalid_params() {
    // period == 0
    assert!(RsiMeanReversionStrategy::new("AAPL", 0, 30.0, 70.0, 1.0).is_err());
    // oversold >= overbought
    assert!(RsiMeanReversionStrategy::new("AAPL", 14, 70.0, 70.0, 1.0).is_err());
    assert!(RsiMeanReversionStrategy::new("AAPL", 14, 80.0, 70.0, 1.0).is_err());
    // out of range
    assert!(RsiMeanReversionStrategy::new("AAPL", 14, -10.0, 70.0, 1.0).is_err());
    assert!(RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 110.0, 1.0).is_err());
    // invalid order_quantity
    assert!(RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 70.0, 0.0).is_err());
}

#[test]
fn test_rsi_warmup() {
    let mut strategy = RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 70.0, 1.0).unwrap();
    
    // Should return None during warmup
    for i in 0..13 {
        let bar = make_bar(100.0, i);
        assert!(strategy.on_bar(&bar).is_none());
    }
    
    // Should return a signal after warmup
    let bar = make_bar(100.0, 14);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
    let signal = signal.unwrap();
    assert_eq!(signal.strategy_name, "rsi_mean_reversion");
}

#[test]
fn test_rsi_signal_provenance() {
    let mut strategy = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();
    
    // Feed some price data
    for i in 0..10 {
        let bar = make_bar(100.0 + (i as f64 % 3.0), i);
        let _ = strategy.on_bar(&bar);
    }
    
    let bar = make_bar(105.0, 10);
    if let Some(signal) = strategy.on_bar(&bar) {
        assert_eq!(signal.strategy_name, "rsi_mean_reversion");
        assert!(!signal.strategy_name.is_empty());
    }
}

// ============================================================================
// MACD Trend Tests
// ============================================================================

#[test]
fn test_macd_construction() {
    let strategy = MacdTrendStrategy::new("AAPL", 12, 26, 9, 1.0);
    assert!(strategy.is_ok());
    let strategy = strategy.unwrap();
    assert_eq!(strategy.name(), "macd_trend");
}

#[test]
fn test_macd_invalid_params() {
    // zero periods
    assert!(MacdTrendStrategy::new("AAPL", 0, 26, 9, 1.0).is_err());
    assert!(MacdTrendStrategy::new("AAPL", 12, 0, 9, 1.0).is_err());
    assert!(MacdTrendStrategy::new("AAPL", 12, 26, 0, 1.0).is_err());
    // fast >= slow
    assert!(MacdTrendStrategy::new("AAPL", 26, 26, 9, 1.0).is_err());
    assert!(MacdTrendStrategy::new("AAPL", 30, 26, 9, 1.0).is_err());
    // invalid order_quantity
    assert!(MacdTrendStrategy::new("AAPL", 12, 26, 9, 0.0).is_err());
}

#[test]
fn test_macd_warmup() {
    let mut strategy = MacdTrendStrategy::new("AAPL", 5, 10, 3, 1.0).unwrap();
    
    // Warmup period is slow + signal - 1 = 10 + 3 - 1 = 12
    for i in 0..11 {
        let bar = make_bar(100.0, i);
        assert!(strategy.on_bar(&bar).is_none());
    }
    
    // Should return a signal after warmup
    let bar = make_bar(100.0, 12);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
    let signal = signal.unwrap();
    assert_eq!(signal.strategy_name, "macd_trend");
}

// ============================================================================
// Bollinger Band Squeeze Tests
// ============================================================================

#[test]
fn test_bb_squeeze_construction() {
    let strategy = BollingerBandSqueezeStrategy::new("AAPL", 20, 2.0, 1.0);
    assert!(strategy.is_ok());
    let strategy = strategy.unwrap();
    assert_eq!(strategy.name(), "bb_squeeze");
}

#[test]
fn test_bb_squeeze_invalid_params() {
    // period == 0
    assert!(BollingerBandSqueezeStrategy::new("AAPL", 0, 2.0, 1.0).is_err());
    // num_std <= 0
    assert!(BollingerBandSqueezeStrategy::new("AAPL", 20, 0.0, 1.0).is_err());
    assert!(BollingerBandSqueezeStrategy::new("AAPL", 20, -1.0, 1.0).is_err());
    // invalid order_quantity
    assert!(BollingerBandSqueezeStrategy::new("AAPL", 20, 2.0, 0.0).is_err());
}

#[test]
fn test_bb_squeeze_warmup() {
    let mut strategy = BollingerBandSqueezeStrategy::new("AAPL", 5, 2.0, 1.0).unwrap();
    
    // Should return None during warmup
    for i in 0..4 {
        let bar = make_bar(100.0, i);
        assert!(strategy.on_bar(&bar).is_none());
    }
    
    // Should return a signal after warmup
    let bar = make_bar(100.0, 5);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
    let signal = signal.unwrap();
    assert_eq!(signal.strategy_name, "bb_squeeze");
}

// ============================================================================
// DSL Parser Tests
// ============================================================================

#[test]
fn test_dsl_parse_valid_yaml() {
    let raw = r#"
name: rsi_mean_reversion
type: mean_reversion
timeframe: 1d
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
  value: 0.1
"#;
    let spec = parse_and_validate_strategy_yaml(raw);
    assert!(spec.is_ok());
    let spec = spec.unwrap();
    assert_eq!(spec.name, "rsi_mean_reversion");
}

#[test]
fn test_dsl_parse_range_value() {
    let raw = r#"
name: rsi_mean_reversion
type: mean_reversion
timeframe: 1d
entry_rules:
  - indicator: rsi
    period: 14
    operator: "<"
    value: [25, 35]
    action: buy
exit_rules:
  - indicator: rsi
    period: 14
    operator: ">"
    value: 70
    action: sell
position_sizing:
  method: percent
  value: 0.1
"#;
    let spec = parse_and_validate_strategy_yaml(raw);
    assert!(spec.is_ok());
}

#[test]
fn test_dsl_parse_with_optional_fields() {
    let raw = r#"
name: test_strategy
type: ma_crossover
timeframe: 1h
entry_rules:
  - name: fast_cross
    condition: golden
    indicator: sma
    period: 10
    operator: ">"
    value: 100
    action: buy
exit_rules: []
position_sizing:
  method: fixed
  value: 100
"#;
    let spec = parse_and_validate_strategy_yaml(raw);
    assert!(spec.is_ok());
}

#[test]
fn test_dsl_invalid_method() {
    let raw = r#"
name: test_strategy
type: ma_crossover
timeframe: 1h
entry_rules: []
exit_rules: []
position_sizing:
  method: invalid_method
  value: 100
"#;
    let result = parse_and_validate_strategy_yaml(raw);
    assert!(result.is_err());
}

#[test]
fn test_dsl_invalid_operator() {
    let raw = r#"
name: test_strategy
type: ma_crossover
timeframe: 1h
entry_rules:
  - indicator: rsi
    operator: "invalid"
    value: 30
    action: buy
exit_rules: []
position_sizing:
  method: percent
  value: 0.1
"#;
    let result = parse_and_validate_strategy_yaml(raw);
    assert!(result.is_err());
}

#[test]
fn test_dsl_invalid_action() {
    let raw = r#"
name: test_strategy
type: ma_crossover
timeframe: 1h
entry_rules:
  - indicator: rsi
    operator: "<"
    value: 30
    action: invalid_action
exit_rules: []
position_sizing:
  method: percent
  value: 0.1
"#;
    let result = parse_and_validate_strategy_yaml(raw);
    assert!(result.is_err());
}

// ============================================================================
// Position Sizing Tests
// ============================================================================

#[test]
fn test_percent_sizer() {
    let sizer = PercentSizer { percent: 0.02 }; // 2%
    let ctx = PositionSizingContext {
        equity: 100_000.0,
        price: 50.0,
        volatility: None,
        win_rate: None,
        win_loss_ratio: None,
    };
    let quantity = sizer.size(&ctx);
    // 2% of 100k = 2000, at $50/share = 40 shares
    assert!((quantity - 40.0).abs() < 0.001);
}

#[test]
fn test_fixed_sizer() {
    let sizer = FixedSizer { amount: 5000.0 };
    let ctx = PositionSizingContext {
        equity: 100_000.0,
        price: 50.0,
        volatility: None,
        win_rate: None,
        win_loss_ratio: None,
    };
    let quantity = sizer.size(&ctx);
    // $5000 / $50 = 100 shares
    assert!((quantity - 100.0).abs() < 0.001);
}

// ============================================================================
// Strategy Trait Tests (Send + Sync)
// ============================================================================

#[test]
fn test_strategy_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    
    assert_send_sync::<MovingAverageCrossoverStrategy>();
    assert_send_sync::<RsiMeanReversionStrategy>();
    assert_send_sync::<MacdTrendStrategy>();
    assert_send_sync::<BollingerBandSqueezeStrategy>();
}

// ============================================================================
// Memory Bounds Tests
// ============================================================================

#[test]
fn test_rsi_memory_bounds() {
    let mut strategy = RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 70.0, 1.0).unwrap();
    
    // Feed more than MAX_HISTORY bars
    for i in 0..1500 {
        let bar = make_bar(100.0 + (i as f64 % 10.0), i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Strategy should still work after 1500 bars
    let bar = make_bar(100.0, 1500);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
}

#[test]
fn test_macd_memory_bounds() {
    let mut strategy = MacdTrendStrategy::new("AAPL", 5, 10, 3, 1.0).unwrap();
    
    // Feed more than MAX_HISTORY bars
    for i in 0..1500 {
        let bar = make_bar(100.0 + (i as f64 % 10.0), i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Strategy should still work after 1500 bars
    let bar = make_bar(100.0, 1500);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
}

#[test]
fn test_bb_squeeze_memory_bounds() {
    let mut strategy = BollingerBandSqueezeStrategy::new("AAPL", 10, 2.0, 1.0).unwrap();
    
    // Feed more than MAX_HISTORY bars
    for i in 0..1500 {
        let bar = make_bar(100.0 + (i as f64 % 10.0), i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Strategy should still work after 1500 bars
    let bar = make_bar(100.0, 1500);
    let signal = strategy.on_bar(&bar);
    assert!(signal.is_some());
}

// ============================================================================
// Signal Generator Tests
// ============================================================================

#[test]
fn test_signal_generator_on_bar() {
    use ferrotick_strategies::signals::generator::SignalGenerator;
    
    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();
    
    let mut generator = SignalGenerator::new(vec![
        Box::new(strategy1),
        Box::new(strategy2),
    ]);
    
    // Feed bars until both strategies are warmed up
    for i in 0..10 {
        let bar = make_bar(100.0, i);
        let _ = generator.on_bar(&bar);
    }
    
    let bar = make_bar(100.0, 10);
    let signals = generator.on_bar(&bar);
    
    // Both strategies should produce signals with provenance
    for signal in &signals {
        assert!(!signal.strategy_name.is_empty());
        assert!(signal.strategy_name == "ma_crossover" || signal.strategy_name == "rsi_mean_reversion");
    }
}

#[test]
fn test_signal_generator_on_signal() {
    use ferrotick_strategies::signals::generator::SignalGenerator;
    use ferrotick_strategies::traits::strategy::Signal;
    
    let strategy = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let mut generator = SignalGenerator::new(vec![Box::new(strategy)]);
    
    let signal = Signal {
        symbol: "AAPL".to_string(),
        ts: "2024-01-01T00:00:00Z".to_string(),
        action: SignalAction::Buy,
        strength: 1.0,
        reason: "test".to_string(),
        strategy_name: "test".to_string(),
    };
    
    let orders = generator.on_signal(&signal);
    assert!(!orders.is_empty());
    assert_eq!(orders[0].symbol, "AAPL");
}

// ============================================================================
// Composite Signal Tests
// ============================================================================

#[test]
fn test_composite_majority() {
    use ferrotick_strategies::signals::composite::{CompositeMode, CompositeSignalGenerator};
    use ferrotick_strategies::signals::generator::SignalGenerator;
    
    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();
    
    let generator = SignalGenerator::new(vec![
        Box::new(strategy1),
        Box::new(strategy2),
    ]);
    
    let mut composite = CompositeSignalGenerator::new(generator, CompositeMode::Majority);
    
    // Feed bars until both strategies are warmed up
    for i in 0..10 {
        let bar = make_bar(100.0, i);
        let _ = composite.on_bar(&bar);
    }
    
    let bar = make_bar(100.0, 10);
    let signal = composite.on_bar(&bar);
    
    assert!(signal.is_some());
    let signal = signal.unwrap();
    assert!(signal.strategy_name.starts_with("composite"));
}

#[test]
fn test_composite_weighted_uses_strategy_name() {
    use ferrotick_strategies::signals::composite::{CompositeMode, CompositeSignalGenerator};
    use ferrotick_strategies::signals::generator::SignalGenerator;
    
    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();
    
    let generator = SignalGenerator::new(vec![
        Box::new(strategy1),
        Box::new(strategy2),
    ]);
    
    let mut composite = CompositeSignalGenerator::new(generator, CompositeMode::WeightedPerformance);
    
    // Set weights by strategy name (not index)
    composite.set_weight("ma_crossover", 0.7);
    composite.set_weight("rsi_mean_reversion", 0.3);
    
    // Feed bars
    for i in 0..10 {
        let bar = make_bar(100.0, i);
        let _ = composite.on_bar(&bar);
    }
    
    let bar = make_bar(100.0, 10);
    let signal = composite.on_bar(&bar);
    
    assert!(signal.is_some());
}

// ============================================================================
// Signal Reset Tests
// ============================================================================

#[test]
fn test_strategy_reset() {
    let mut strategy = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();
    
    // Feed some bars
    for i in 0..10 {
        let bar = make_bar(100.0, i);
        let _ = strategy.on_bar(&bar);
    }
    
    // Reset
    strategy.reset();
    
    // Should return None again after reset (warmup needed)
    let bar = make_bar(100.0, 10);
    assert!(strategy.on_bar(&bar).is_none());
}
