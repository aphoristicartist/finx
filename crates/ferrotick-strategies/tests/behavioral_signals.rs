//! Behavioral tests for trading strategies - Phase 9 Strategy Framework
//!
//! These tests verify that strategies generate appropriate signals in response
//! to market conditions, not just that they run without errors.

use ferrotick_core::{Bar, UtcDateTime};
use ferrotick_strategies::traits::strategy::SignalAction;
use ferrotick_strategies::{MovingAverageCrossoverStrategy, RsiMeanReversionStrategy, Strategy};

/// Helper to create a bar with specific close price
fn bar_with_close(close: f64) -> Bar {
    let ts = UtcDateTime::parse("2024-01-01T00:00:00Z").expect("valid timestamp");
    Bar {
        ts,
        open: close - 0.5,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: Some(1000),
        vwap: None,
    }
}

/// Helper to create a bar with timestamp
fn bar_with_close_and_ts(close: f64, day: u32) -> Bar {
    let ts = UtcDateTime::parse(&format!("2024-01-{:02}T00:00:00Z", day)).expect("valid timestamp");
    Bar {
        ts,
        open: close - 0.5,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: Some(1000),
        vwap: None,
    }
}

#[test]
fn test_ma_crossover_generates_buy_signal_at_golden_cross() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Create declining prices (fast MA < slow MA)
    for i in 0..10 {
        let close = 100.0 - i as f64;
        let bar = bar_with_close(close);
        strategy.on_bar(&bar);
    }

    // Create sharp rise to trigger golden cross (fast MA crosses above slow MA)
    let mut buy_signal_found = false;
    for i in 0..10 {
        let close = 95.0 + i as f64 * 3.0; // Sharp rise
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            if signal.action == SignalAction::Buy {
                buy_signal_found = true;
                break;
            }
        }
    }

    // BEHAVIOR: Should generate at least one Buy signal at golden cross
    assert!(
        buy_signal_found,
        "MA crossover should generate Buy signal when fast MA crosses above slow MA"
    );
}

#[test]
fn test_ma_crossover_generates_sell_signal_at_death_cross() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Create rising prices (fast MA > slow MA)
    for i in 0..10 {
        let close = 100.0 + i as f64;
        let bar = bar_with_close(close);
        strategy.on_bar(&bar);
    }

    // Create sharp decline to trigger death cross (fast MA crosses below slow MA)
    let mut sell_signal_found = false;
    for i in 0..10 {
        let close = 110.0 - i as f64 * 3.0; // Sharp decline
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            if signal.action == SignalAction::Sell {
                sell_signal_found = true;
                break;
            }
        }
    }

    // BEHAVIOR: Should generate at least one Sell signal at death cross
    assert!(
        sell_signal_found,
        "MA crossover should generate Sell signal when fast MA crosses below slow MA"
    );
}

#[test]
fn test_ma_crossover_requires_warmup() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Feed fewer bars than slow_period
    for i in 0..9 {
        let close = 100.0 + i as f64;
        let bar = bar_with_close(close);
        let signal = strategy.on_bar(&bar);

        // BEHAVIOR: Should return None during warmup period
        assert!(
            signal.is_none(),
            "Strategy should return None during warmup (bar {} < slow_period 10)",
            i + 1
        );
    }

    // Feed the 10th bar - should now produce a signal
    let bar = bar_with_close(110.0);
    let signal = strategy.on_bar(&bar);

    // BEHAVIOR: Should produce a signal after warmup
    assert!(
        signal.is_some(),
        "Strategy should produce signal after warmup period"
    );
}

#[test]
fn test_rsi_strategy_buys_at_oversold() {
    let mut strategy = RsiMeanReversionStrategy::new("TEST", 14, 30.0, 70.0, 100.0)
        .expect("strategy should be created");

    // Create oversold condition (RSI < 30) with continuous decline
    let mut buy_signal_found = false;
    for i in 0..30 {
        let close = 100.0 - i as f64 * 2.0; // Sharp decline
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            if signal.action == SignalAction::Buy {
                buy_signal_found = true;
                break;
            }
        }
    }

    // BEHAVIOR: Should generate Buy signal when RSI < oversold threshold
    assert!(
        buy_signal_found,
        "RSI strategy should generate Buy signal when oversold (RSI < 30)"
    );
}

#[test]
fn test_rsi_strategy_sells_at_overbought() {
    let mut strategy = RsiMeanReversionStrategy::new("TEST", 14, 30.0, 70.0, 100.0)
        .expect("strategy should be created");

    // Create overbought condition (RSI > 70) with continuous rise
    let mut sell_signal_found = false;
    for i in 0..30 {
        let close = 100.0 + i as f64 * 2.0; // Sharp rise
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            if signal.action == SignalAction::Sell {
                sell_signal_found = true;
                break;
            }
        }
    }

    // BEHAVIOR: Should generate Sell signal when RSI > overbought threshold
    assert!(
        sell_signal_found,
        "RSI strategy should generate Sell signal when overbought (RSI > 70)"
    );
}

#[test]
fn test_rsi_strategy_requires_warmup() {
    let mut strategy = RsiMeanReversionStrategy::new("TEST", 14, 30.0, 70.0, 100.0)
        .expect("strategy should be created");

    // Feed fewer bars than RSI period
    for i in 0..13 {
        let close = 100.0 + i as f64;
        let bar = bar_with_close(close);
        let signal = strategy.on_bar(&bar);

        // BEHAVIOR: Should return None during warmup
        assert!(
            signal.is_none(),
            "RSI strategy should return None during warmup (bar {} < period 14)",
            i + 1
        );
    }
}

#[test]
fn test_strategy_signal_contains_symbol() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 100.0)
        .expect("strategy should be created");

    // Warm up the strategy
    for i in 0..15 {
        let close = 100.0 + i as f64;
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            // BEHAVIOR: Signal should contain the correct symbol
            assert_eq!(
                signal.symbol, "AAPL",
                "Signal symbol should match strategy symbol"
            );
            break;
        }
    }
}

#[test]
fn test_strategy_signal_contains_timestamp() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Warm up and get a signal
    for i in 0..15 {
        let bar = bar_with_close_and_ts(100.0 + i as f64, (i % 28) + 1);
        if let Some(signal) = strategy.on_bar(&bar) {
            // BEHAVIOR: Signal should contain a timestamp
            assert!(!signal.ts.is_empty(), "Signal should have a timestamp");
            break;
        }
    }
}

#[test]
fn test_strategy_signal_has_valid_strength() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Warm up and get a signal
    for i in 0..15 {
        let close = 100.0 + i as f64;
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            // BEHAVIOR: Signal strength should be in [0, 1] range
            assert!(
                signal.strength >= 0.0 && signal.strength <= 1.0,
                "Signal strength should be in [0, 1] range, got {}",
                signal.strength
            );
            break;
        }
    }
}

#[test]
fn test_ma_crossover_rejects_invalid_config() {
    // fast_period = 0
    let result = MovingAverageCrossoverStrategy::new("TEST", 0, 10, 100.0);
    assert!(result.is_err(), "Should reject fast_period = 0");

    // slow_period = 0
    let result = MovingAverageCrossoverStrategy::new("TEST", 5, 0, 100.0);
    assert!(result.is_err(), "Should reject slow_period = 0");

    // fast_period >= slow_period
    let result = MovingAverageCrossoverStrategy::new("TEST", 10, 10, 100.0);
    assert!(result.is_err(), "Should reject fast_period >= slow_period");

    let result = MovingAverageCrossoverStrategy::new("TEST", 15, 10, 100.0);
    assert!(result.is_err(), "Should reject fast_period > slow_period");

    // order_quantity <= 0
    let result = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 0.0);
    assert!(result.is_err(), "Should reject order_quantity = 0");

    let result = MovingAverageCrossoverStrategy::new("TEST", 5, 10, -1.0);
    assert!(result.is_err(), "Should reject negative order_quantity");
}

#[test]
fn test_rsi_strategy_rejects_invalid_config() {
    // period = 0
    let result = RsiMeanReversionStrategy::new("TEST", 0, 30.0, 70.0, 100.0);
    assert!(result.is_err(), "Should reject period = 0");

    // oversold >= overbought
    let result = RsiMeanReversionStrategy::new("TEST", 14, 70.0, 70.0, 100.0);
    assert!(result.is_err(), "Should reject oversold >= overbought");

    let result = RsiMeanReversionStrategy::new("TEST", 14, 80.0, 70.0, 100.0);
    assert!(result.is_err(), "Should reject oversold > overbought");

    // thresholds out of range
    let result = RsiMeanReversionStrategy::new("TEST", 14, -10.0, 70.0, 100.0);
    assert!(result.is_err(), "Should reject negative oversold threshold");

    let result = RsiMeanReversionStrategy::new("TEST", 14, 30.0, 110.0, 100.0);
    assert!(result.is_err(), "Should reject overbought > 100");

    // order_quantity <= 0
    let result = RsiMeanReversionStrategy::new("TEST", 14, 30.0, 70.0, 0.0);
    assert!(result.is_err(), "Should reject order_quantity = 0");
}

#[test]
fn test_strategy_hold_action_in_neutral_market() {
    let mut strategy = MovingAverageCrossoverStrategy::new("TEST", 5, 10, 100.0)
        .expect("strategy should be created");

    // Create sideways market (prices oscillate)
    let mut hold_count = 0;
    let mut signal_count = 0;

    for i in 0..30 {
        let close = 100.0 + (i as f64 * 0.3).sin() * 2.0;
        let bar = bar_with_close(close);
        if let Some(signal) = strategy.on_bar(&bar) {
            signal_count += 1;
            if signal.action == SignalAction::Hold {
                hold_count += 1;
            }
        }
    }

    // BEHAVIOR: In sideways market, should mostly generate Hold signals
    if signal_count > 0 {
        let hold_percentage = (hold_count as f64 / signal_count as f64) * 100.0;
        assert!(
            hold_percentage > 50.0,
            "Hold signals should dominate in sideways market, but only {:.1}% were Hold",
            hold_percentage
        );
    }
}
