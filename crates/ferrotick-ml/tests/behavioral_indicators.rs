//! Behavioral tests for technical indicators - Phase 7 Feature Engineering
//!
//! These tests verify that indicators produce expected behavioral outputs,
//! not just that they run without errors.

use ferrotick_core::{Bar, UtcDateTime};
use ferrotick_ml::features::indicators::{compute_bollinger, compute_rsi};

/// Helper to create bars with specific close prices
fn bars_from_closes(closes: &[f64]) -> Vec<Bar> {
    closes
        .iter()
        .enumerate()
        .map(|(i, &close)| {
            let day = (i % 28) + 1;
            let ts = UtcDateTime::parse(&format!("2024-01-{:02}T00:00:00Z", day))
                .expect("valid timestamp");
            Bar {
                ts,
                open: close - 0.5,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: Some(1000),
                vwap: None,
            }
        })
        .collect()
}

#[test]
fn test_rsi_oversold_for_continuous_decline() {
    // Create 20 bars with continuous decline - RSI should be very low
    let closes: Vec<f64> = (0..20).map(|i| 100.0 - i as f64 * 2.0).collect();

    let rsi_values = compute_rsi(&closes, 14).expect("RSI computation should succeed");

    // The last RSI value should be defined (after warmup period)
    let last_rsi = rsi_values
        .last()
        .unwrap()
        .expect("RSI should be defined after warmup");

    // BEHAVIOR: RSI should be oversold (< 30) for continuous decline
    assert!(
        last_rsi < 30.0,
        "RSI should be oversold (< 30) for continuous decline, but was {:.2}",
        last_rsi
    );
    assert!(
        last_rsi > 0.0,
        "RSI should be positive, but was {:.2}",
        last_rsi
    );
}

#[test]
fn test_rsi_overbought_for_continuous_rise() {
    // Create 20 bars with continuous rise - RSI should be very high
    let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64 * 2.0).collect();

    let rsi_values = compute_rsi(&closes, 14).expect("RSI computation should succeed");

    // The last RSI value should be defined (after warmup period)
    let last_rsi = rsi_values
        .last()
        .unwrap()
        .expect("RSI should be defined after warmup");

    // BEHAVIOR: RSI should be overbought (> 70) for continuous rise
    assert!(
        last_rsi > 70.0,
        "RSI should be overbought (> 70) for continuous rise, but was {:.2}",
        last_rsi
    );
    assert!(
        last_rsi < 100.0,
        "RSI should be < 100, but was {:.2}",
        last_rsi
    );
}

#[test]
fn test_rsi_returns_none_during_warmup() {
    let closes: Vec<f64> = (0..10).map(|i| 100.0 + i as f64).collect();

    let rsi_values = compute_rsi(&closes, 14).expect("RSI computation should succeed");

    // BEHAVIOR: First period-1 values should be None during warmup
    for (i, value) in rsi_values.iter().enumerate().take(13) {
        assert!(
            value.is_none(),
            "RSI[{}] should be None during warmup, but was {:?}",
            i,
            value
        );
    }
}

#[test]
fn test_rsi_neutral_for_sideways_market() {
    // Create oscillating prices around 100 - RSI should be near 50
    let closes: Vec<f64> = (0..30)
        .map(|i| {
            let oscillation = (i as f64 * 0.5).sin() * 2.0;
            100.0 + oscillation
        })
        .collect();

    let rsi_values = compute_rsi(&closes, 14).expect("RSI computation should succeed");

    let last_rsi = rsi_values
        .last()
        .unwrap()
        .expect("RSI should be defined after warmup");

    // BEHAVIOR: RSI should be close to 50 (neutral) for sideways market
    assert!(
        (last_rsi - 50.0).abs() < 20.0,
        "RSI should be near 50 for sideways market, but was {:.2}",
        last_rsi
    );
}

#[test]
fn test_bollinger_bands_contain_prices() {
    let closes: Vec<f64> = (0..50)
        .map(|i| 100.0 + (i as f64 * 0.1).sin() * 5.0)
        .collect();

    let bands = compute_bollinger(&closes, 20, 2.0).expect("Bollinger computation should succeed");

    // BEHAVIOR: Most prices should fall within Bollinger bands
    let mut within_bands = 0;
    let mut total_valid = 0;

    for (i, (&close, (upper, lower))) in closes
        .iter()
        .zip(bands.upper.iter().zip(bands.lower.iter()))
        .enumerate()
    {
        if let (Some(u), Some(l)) = (upper, lower) {
            total_valid += 1;
            if close >= *l && close <= *u {
                within_bands += 1;
            }
        }
    }

    // At least 70% of prices should be within bands (realistic for 2 std dev)
    let percentage = (within_bands as f64 / total_valid as f64) * 100.0;
    assert!(
        percentage >= 70.0,
        "At least 70% of prices should be within Bollinger bands, but only {:.1}% were",
        percentage
    );
}

#[test]
fn test_bollinger_upper_above_lower() {
    let closes: Vec<f64> = (0..30).map(|i| 100.0 + i as f64 * 0.5).collect();

    let bands = compute_bollinger(&closes, 14, 2.0).expect("Bollinger computation should succeed");

    // BEHAVIOR: Upper band should always be above lower band
    for (i, (upper, lower)) in bands.upper.iter().zip(bands.lower.iter()).enumerate() {
        if let (Some(u), Some(l)) = (upper, lower) {
            assert!(
                u > l,
                "Upper band ({}) should be > lower band ({}) at index {}",
                u,
                l,
                i
            );
        }
    }
}

#[test]
fn test_rsi_bounded_between_0_and_100() {
    // Create extreme price movements
    let closes: Vec<f64> = (0..50)
        .map(|i| {
            if i < 25 {
                100.0 + i as f64 * 5.0 // Sharp rise
            } else {
                225.0 - (i - 25) as f64 * 5.0 // Sharp fall
            }
        })
        .collect();

    let rsi_values = compute_rsi(&closes, 14).expect("RSI computation should succeed");

    // BEHAVIOR: All RSI values should be in [0, 100] range
    for (i, rsi_opt) in rsi_values.iter().enumerate() {
        if let Some(rsi) = rsi_opt {
            assert!(
                *rsi >= 0.0 && *rsi <= 100.0,
                "RSI[{}] = {:.2} is out of bounds [0, 100]",
                i,
                rsi
            );
        }
    }
}
