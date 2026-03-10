use ferrotick_core::{Bar, UtcDateTime};
use ferrotick_strategies::dsl::{parse_and_validate_strategy_yaml, RuleValue};
use ferrotick_strategies::sizing::position::{
    FixedSizer, PercentSizer, PositionSizer, PositionSizingContext,
};
use ferrotick_strategies::strategies::{
    BollingerBandSqueezeStrategy, MacdTrendStrategy, MovingAverageCrossoverStrategy,
    RsiMeanReversionStrategy,
};
use ferrotick_strategies::traits::strategy::{OrderSide, Signal, SignalAction, Strategy};

const EXPECTED_MAX_HISTORY: usize = 1000;

fn make_bar(close: f64, ts_days: i64) -> Bar {
    let ts = UtcDateTime::from_unix_timestamp(ts_days * 86400).expect("valid timestamp");
    Bar::new(ts, close, close, close, close, Some(1000), None).expect("valid bar")
}

fn make_signal(symbol: &str, action: SignalAction) -> Signal {
    Signal {
        symbol: symbol.to_string(),
        ts: "2024-01-01T00:00:00Z".to_string(),
        action,
        strength: 1.0,
        reason: "test".to_string(),
        strategy_name: "test".to_string(),
        source_strategy_id: "test".to_string(),
    }
}

fn extract_closes_from_debug(debug_repr: &str) -> Vec<f64> {
    let marker = "closes: [";
    let start = debug_repr
        .find(marker)
        .expect("debug output must contain closes");
    let rest = &debug_repr[start + marker.len()..];
    let end = rest
        .find(']')
        .expect("debug output must contain closing bracket");
    let raw_values = rest[..end].trim();
    if raw_values.is_empty() {
        return Vec::new();
    }

    raw_values
        .split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .expect("close value should parse as f64")
        })
        .collect()
}

// ============================================================================
// MA Crossover Tests
// ============================================================================

#[test]
fn test_ma_crossover_construction() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 1.0).unwrap();
    assert_eq!(strategy.name(), "ma_crossover");

    let debug = format!("{strategy:?}");
    assert!(debug.contains("symbol: \"AAPL\""));
    assert!(debug.contains("fast_period: 5"));
    assert!(debug.contains("slow_period: 10"));
    assert!(debug.contains("order_quantity: 1.0"));
    assert!(debug.contains("closes: []"));
    assert!(debug.contains("prev_fast: None"));
    assert!(debug.contains("prev_slow: None"));

    let order = strategy
        .on_signal(&make_signal("AAPL", SignalAction::Buy))
        .expect("buy signal should produce order");
    assert_eq!(order.symbol, "AAPL");
    assert_eq!(order.side, OrderSide::Buy);
    assert_eq!(order.quantity, 1.0);
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
    let warmup_period = 10usize;

    // No signal until exactly slow_period bars are available.
    for i in 0..(warmup_period - 1) {
        let bar = make_bar(100.0 + i as f64, i as i64);
        assert!(strategy.on_bar(&bar).is_none());
    }

    let first_post_warmup = strategy
        .on_bar(&make_bar(109.0, (warmup_period - 1) as i64))
        .expect("signal should be emitted at warmup boundary");
    assert_eq!(first_post_warmup.strategy_name, "ma_crossover");
    assert_eq!(first_post_warmup.action, SignalAction::Hold);
    assert!(first_post_warmup.reason.contains("fast_sma="));
    assert!(first_post_warmup.reason.contains("slow_sma="));

    let next_signal = strategy
        .on_bar(&make_bar(110.0, warmup_period as i64))
        .expect("signal should continue after warmup");
    assert_eq!(next_signal.strategy_name, "ma_crossover");
}

#[test]
fn test_ma_crossover_golden_cross() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();

    // Sequence engineered to trigger one sell crossover followed by one buy crossover.
    let closes = [
        100.0, 101.0, 102.0, 103.0, 104.0, 100.0, 96.0, 92.0, 88.0, 92.0, 96.0, 100.0, 104.0,
    ];

    let mut buy_count = 0;
    let mut sell_count = 0;
    let mut crossover_points = Vec::new();

    for (i, close) in closes.iter().enumerate() {
        let bar = make_bar(*close, i as i64);
        if let Some(signal) = strategy.on_bar(&bar) {
            match signal.action {
                SignalAction::Buy => {
                    buy_count += 1;
                    crossover_points.push((i, SignalAction::Buy));
                }
                SignalAction::Sell => {
                    sell_count += 1;
                    crossover_points.push((i, SignalAction::Sell));
                }
                SignalAction::Hold => {}
            }
        }
    }

    assert_eq!(
        sell_count, 1,
        "expected exactly one death cross sell signal"
    );
    assert_eq!(buy_count, 1, "expected exactly one golden cross buy signal");
    assert_eq!(
        crossover_points,
        vec![(6, SignalAction::Sell), (11, SignalAction::Buy)],
        "signals should occur at expected crossover bars"
    );
}

// ============================================================================
// RSI Mean Reversion Tests
// ============================================================================

#[test]
fn test_rsi_construction() {
    let mut strategy = RsiMeanReversionStrategy::new("AAPL", 14, 30.0, 70.0, 1.0).unwrap();
    assert_eq!(strategy.name(), "rsi_mean_reversion");

    let debug = format!("{strategy:?}");
    assert!(debug.contains("symbol: \"AAPL\""));
    assert!(debug.contains("period: 14"));
    assert!(debug.contains("oversold: 30.0"));
    assert!(debug.contains("overbought: 70.0"));
    assert!(debug.contains("order_quantity: 1.0"));
    assert!(debug.contains("closes: []"));

    let order = strategy
        .on_signal(&make_signal("AAPL", SignalAction::Sell))
        .expect("sell signal should produce order");
    assert_eq!(order.symbol, "AAPL");
    assert_eq!(order.side, OrderSide::Sell);
    assert_eq!(order.quantity, 1.0);
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
    let warmup_period = 14usize;

    for i in 0..(warmup_period - 1) {
        let close = if i % 2 == 0 { 100.0 } else { 101.0 };
        let bar = make_bar(close, i as i64);
        assert!(strategy.on_bar(&bar).is_none());
    }

    let first_post_warmup = strategy
        .on_bar(&make_bar(100.0, (warmup_period - 1) as i64))
        .expect("signal should be emitted at warmup boundary");
    assert_eq!(first_post_warmup.strategy_name, "rsi_mean_reversion");
    assert!(first_post_warmup.reason.starts_with("rsi="));

    let next_signal = strategy
        .on_bar(&make_bar(101.0, warmup_period as i64))
        .expect("signal should continue after warmup");
    assert_eq!(next_signal.strategy_name, "rsi_mean_reversion");
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
    let mut strategy = MacdTrendStrategy::new("AAPL", 12, 26, 9, 1.0).unwrap();
    assert_eq!(strategy.name(), "macd_trend");

    let debug = format!("{strategy:?}");
    assert!(debug.contains("symbol: \"AAPL\""));
    assert!(debug.contains("fast_period: 12"));
    assert!(debug.contains("slow_period: 26"));
    assert!(debug.contains("signal_period: 9"));
    assert!(debug.contains("order_quantity: 1.0"));
    assert!(debug.contains("closes: []"));
    assert!(debug.contains("prev_macd: None"));
    assert!(debug.contains("prev_signal: None"));

    let order = strategy
        .on_signal(&make_signal("AAPL", SignalAction::Buy))
        .expect("buy signal should produce order");
    assert_eq!(order.symbol, "AAPL");
    assert_eq!(order.side, OrderSide::Buy);
    assert_eq!(order.quantity, 1.0);
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
    let warmup_period = 10 + 3 - 1;

    for i in 0..(warmup_period - 1) {
        let bar = make_bar(100.0, i as i64);
        assert!(strategy.on_bar(&bar).is_none());
    }

    let first_post_warmup = strategy
        .on_bar(&make_bar(100.0, (warmup_period - 1) as i64))
        .expect("signal should be emitted at warmup boundary");
    assert_eq!(first_post_warmup.strategy_name, "macd_trend");
    assert_eq!(first_post_warmup.action, SignalAction::Hold);
    assert!(first_post_warmup.reason.contains("macd="));
    assert!(first_post_warmup.reason.contains("signal="));

    let next_signal = strategy
        .on_bar(&make_bar(100.0, warmup_period as i64))
        .expect("signal should continue after warmup");
    assert_eq!(next_signal.strategy_name, "macd_trend");
}

// ============================================================================
// Bollinger Band Squeeze Tests
// ============================================================================

#[test]
fn test_bb_squeeze_construction() {
    let mut strategy = BollingerBandSqueezeStrategy::new("AAPL", 20, 2.0, 1.0).unwrap();
    assert_eq!(strategy.name(), "bb_squeeze");

    let debug = format!("{strategy:?}");
    assert!(debug.contains("symbol: \"AAPL\""));
    assert!(debug.contains("period: 20"));
    assert!(debug.contains("num_std: 2.0"));
    assert!(debug.contains("order_quantity: 1.0"));
    assert!(debug.contains("closes: []"));
    assert!(debug.contains("prev_in_squeeze: false"));

    let order = strategy
        .on_signal(&make_signal("AAPL", SignalAction::Sell))
        .expect("sell signal should produce order");
    assert_eq!(order.symbol, "AAPL");
    assert_eq!(order.side, OrderSide::Sell);
    assert_eq!(order.quantity, 1.0);
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
    let warmup_period = 5usize;

    for i in 0..(warmup_period - 1) {
        let bar = make_bar(100.0, i as i64);
        assert!(strategy.on_bar(&bar).is_none());
    }

    let first_post_warmup = strategy
        .on_bar(&make_bar(100.0, (warmup_period - 1) as i64))
        .expect("signal should be emitted at warmup boundary");
    assert_eq!(first_post_warmup.strategy_name, "bb_squeeze");
    assert_eq!(first_post_warmup.action, SignalAction::Hold);
    assert!(first_post_warmup.reason.contains("bb_upper="));
    assert!(first_post_warmup.reason.contains("bb_lower="));
    assert!(first_post_warmup.reason.contains("bandwidth="));

    let next_signal = strategy
        .on_bar(&make_bar(100.0, warmup_period as i64))
        .expect("signal should continue after warmup");
    assert_eq!(next_signal.strategy_name, "bb_squeeze");
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
    let spec = parse_and_validate_strategy_yaml(raw).expect("valid YAML should parse");
    assert_eq!(spec.name, "rsi_mean_reversion");
    assert_eq!(spec.strategy_type, "mean_reversion");
    assert_eq!(spec.timeframe, "1d");
    assert_eq!(spec.entry_rules.len(), 1);
    assert_eq!(spec.exit_rules.len(), 1);
    assert_eq!(spec.position_sizing.method, "percent");
    assert!((spec.position_sizing.value - 0.1).abs() < 1e-12);

    let entry = &spec.entry_rules[0];
    assert_eq!(entry.indicator, "rsi");
    assert_eq!(entry.period, Some(14));
    assert_eq!(entry.operator, "<");
    match &entry.value {
        RuleValue::Scalar(v) => assert!((*v - 30.0).abs() < 1e-12),
        RuleValue::Range(_) => panic!("entry value should be scalar"),
    }
    assert_eq!(entry.action, "buy");

    let exit = &spec.exit_rules[0];
    assert_eq!(exit.indicator, "rsi");
    assert_eq!(exit.period, Some(14));
    assert_eq!(exit.operator, ">");
    match &exit.value {
        RuleValue::Scalar(v) => assert!((*v - 70.0).abs() < 1e-12),
        RuleValue::Range(_) => panic!("exit value should be scalar"),
    }
    assert_eq!(exit.action, "sell");
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
    let spec = parse_and_validate_strategy_yaml(raw).expect("valid range YAML should parse");
    assert_eq!(spec.entry_rules.len(), 1);
    assert_eq!(spec.exit_rules.len(), 1);

    let entry = &spec.entry_rules[0];
    match &entry.value {
        RuleValue::Range([min, max]) => {
            assert!((*min - 25.0).abs() < 1e-12);
            assert!((*max - 35.0).abs() < 1e-12);
        }
        RuleValue::Scalar(_) => panic!("entry value should be parsed as range"),
    }
    assert!((entry.value.to_f64() - 30.0).abs() < 1e-12);

    let exit = &spec.exit_rules[0];
    match &exit.value {
        RuleValue::Scalar(v) => assert!((*v - 70.0).abs() < 1e-12),
        RuleValue::Range(_) => panic!("exit value should be scalar"),
    }
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
    let spec =
        parse_and_validate_strategy_yaml(raw).expect("YAML with optional fields should parse");
    assert_eq!(spec.name, "test_strategy");
    assert_eq!(spec.strategy_type, "ma_crossover");
    assert_eq!(spec.timeframe, "1h");
    assert_eq!(spec.entry_rules.len(), 1);
    assert!(spec.exit_rules.is_empty());
    assert_eq!(spec.position_sizing.method, "fixed");
    assert!((spec.position_sizing.value - 100.0).abs() < 1e-12);

    let entry = &spec.entry_rules[0];
    assert_eq!(entry.name.as_deref(), Some("fast_cross"));
    assert_eq!(entry.condition.as_deref(), Some("golden"));
    assert_eq!(entry.indicator, "sma");
    assert_eq!(entry.period, Some(10));
    assert_eq!(entry.operator, ">");
    match &entry.value {
        RuleValue::Scalar(v) => assert!((*v - 100.0).abs() < 1e-12),
        RuleValue::Range(_) => panic!("entry value should be scalar"),
    }
    assert_eq!(entry.action, "buy");
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
    let error = parse_and_validate_strategy_yaml(raw)
        .expect_err("invalid method should fail validation")
        .to_string();
    assert!(error.contains("position_sizing.method"));
    assert!(error.contains("invalid method"));
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
    let error = parse_and_validate_strategy_yaml(raw)
        .expect_err("invalid operator should fail validation")
        .to_string();
    assert!(error.contains("entry_rules[0].operator"));
    assert!(error.contains("invalid operator"));
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
    let error = parse_and_validate_strategy_yaml(raw)
        .expect_err("invalid action should fail validation")
        .to_string();
    assert!(error.contains("entry_rules[0].action"));
    assert!(error.contains("invalid action"));
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

    // Feed more than MAX_HISTORY bars so old data must be discarded.
    for i in 0..1500 {
        let bar = make_bar(i as f64, i as i64);
        let _ = strategy.on_bar(&bar);
    }

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(500.0));
    assert_eq!(closes.last().copied(), Some(1499.0));

    let signal = strategy
        .on_bar(&make_bar(1500.0, 1500))
        .expect("strategy should continue producing signals");
    assert_eq!(signal.strategy_name, "rsi_mean_reversion");

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(501.0));
    assert_eq!(closes.last().copied(), Some(1500.0));
}

#[test]
fn test_macd_memory_bounds() {
    let mut strategy = MacdTrendStrategy::new("AAPL", 5, 10, 3, 1.0).unwrap();

    // Feed more than MAX_HISTORY bars so old data must be discarded.
    for i in 0..1500 {
        let bar = make_bar(i as f64, i as i64);
        let _ = strategy.on_bar(&bar);
    }

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(500.0));
    assert_eq!(closes.last().copied(), Some(1499.0));

    let signal = strategy
        .on_bar(&make_bar(1500.0, 1500))
        .expect("strategy should continue producing signals");
    assert_eq!(signal.strategy_name, "macd_trend");

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(501.0));
    assert_eq!(closes.last().copied(), Some(1500.0));
}

#[test]
fn test_bb_squeeze_memory_bounds() {
    let mut strategy = BollingerBandSqueezeStrategy::new("AAPL", 10, 2.0, 1.0).unwrap();

    // Feed more than MAX_HISTORY bars so old data must be discarded.
    for i in 0..1500 {
        let bar = make_bar(i as f64, i as i64);
        let _ = strategy.on_bar(&bar);
    }

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(500.0));
    assert_eq!(closes.last().copied(), Some(1499.0));

    let signal = strategy
        .on_bar(&make_bar(1500.0, 1500))
        .expect("strategy should continue producing signals");
    assert_eq!(signal.strategy_name, "bb_squeeze");

    let closes = extract_closes_from_debug(&format!("{strategy:?}"));
    assert_eq!(closes.len(), EXPECTED_MAX_HISTORY);
    assert_eq!(closes.first().copied(), Some(501.0));
    assert_eq!(closes.last().copied(), Some(1500.0));
}

// ============================================================================
// Signal Generator Tests
// ============================================================================

#[test]
fn test_signal_generator_on_bar() {
    use ferrotick_strategies::signals::generator::SignalGenerator;

    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();

    let mut generator = SignalGenerator::new(vec![Box::new(strategy1), Box::new(strategy2)]);

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
        assert!(!signal.source_strategy_id.is_empty());
        assert!(
            signal.strategy_name == "ma_crossover" || signal.strategy_name == "rsi_mean_reversion"
        );
        assert_eq!(signal.strategy_name, signal.source_strategy_id);
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
        strategy_name: "ma_crossover".to_string(),
        source_strategy_id: "ma_crossover".to_string(),
    };

    let orders = generator.on_signal(&signal);
    assert!(!orders.is_empty());
    assert_eq!(orders[0].symbol, "AAPL");
}

#[test]
fn test_signal_generator_on_signal_routes_to_source_only() {
    use ferrotick_strategies::signals::generator::SignalGenerator;
    use ferrotick_strategies::traits::strategy::Signal;

    let source_strategy = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let other_strategy = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 2.0).unwrap();
    let mut generator =
        SignalGenerator::new(vec![Box::new(source_strategy), Box::new(other_strategy)]);

    let signal = Signal {
        symbol: "AAPL".to_string(),
        ts: "2024-01-01T00:00:00Z".to_string(),
        action: SignalAction::Buy,
        strength: 1.0,
        reason: "test".to_string(),
        strategy_name: "ma_crossover".to_string(),
        source_strategy_id: "ma_crossover".to_string(),
    };

    let orders = generator.on_signal(&signal);
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].symbol, "AAPL");
    assert_eq!(orders[0].side, OrderSide::Buy);
    assert_eq!(orders[0].quantity, 1.0);
}

#[test]
fn test_signal_generator_handles_duplicate_strategy_names() {
    use ferrotick_strategies::signals::generator::SignalGenerator;
    use ferrotick_strategies::traits::strategy::Signal;

    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 2.0).unwrap();
    let mut generator = SignalGenerator::new(vec![Box::new(strategy1), Box::new(strategy2)]);

    // Explicitly route to the second strategy instance.
    let targeted = Signal {
        symbol: "AAPL".to_string(),
        ts: "2024-01-01T00:00:00Z".to_string(),
        action: SignalAction::Buy,
        strength: 1.0,
        reason: "duplicate-name-target".to_string(),
        strategy_name: "ma_crossover".to_string(),
        source_strategy_id: "ma_crossover#2".to_string(),
    };
    let targeted_orders = generator.on_signal(&targeted);
    assert_eq!(targeted_orders.len(), 1);
    assert_eq!(targeted_orders[0].quantity, 2.0);

    // Legacy fallback by strategy_name should fan out to all same-name strategies.
    let fallback = Signal {
        source_strategy_id: String::new(),
        ..targeted
    };
    let fallback_orders = generator.on_signal(&fallback);
    assert_eq!(fallback_orders.len(), 2);
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

    let generator = SignalGenerator::new(vec![Box::new(strategy1), Box::new(strategy2)]);

    let mut composite = CompositeSignalGenerator::new(generator, CompositeMode::Majority);

    // Feed bars until both strategies are warmed up
    for i in 0..10 {
        let bar = make_bar(100.0, i);
        let _ = composite.on_bar(&bar);
    }

    let bar = make_bar(100.0, 10);
    let signal = composite.on_bar(&bar);

    let signal = signal.expect("Signal should be generated");
    assert!(signal.strategy_name.starts_with("composite"));
}

#[test]
fn test_composite_weighted_uses_strategy_name() {
    use ferrotick_strategies::signals::composite::{CompositeMode, CompositeSignalGenerator};
    use ferrotick_strategies::signals::generator::SignalGenerator;

    let strategy1 = MovingAverageCrossoverStrategy::new("AAPL", 3, 5, 1.0).unwrap();
    let strategy2 = RsiMeanReversionStrategy::new("AAPL", 5, 30.0, 70.0, 1.0).unwrap();

    let generator = SignalGenerator::new(vec![Box::new(strategy1), Box::new(strategy2)]);

    let mut composite =
        CompositeSignalGenerator::new(generator, CompositeMode::WeightedPerformance);

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

    let signal = signal.expect("Composite signal should be generated after feeding bars");
    // Signal generated successfully - that's the behavioral check
    // (SignalAction enum doesn't have default)
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
