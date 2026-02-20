//! # Historical Bars Analysis Example
//!
//! This example demonstrates how to fetch historical OHLCV bars
//! and perform basic technical analysis calculations.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example bars_analysis
//! ```
//!
//! ## What it demonstrates
//!
//! - Fetching historical price data
//! - Computing simple moving averages (SMA)
//! - Calculating price volatility
//! - Finding support and resistance levels

use ferrotick_core::{PolygonAdapter, BarsRequest, DataSource};

/// Calculate Simple Moving Average
fn calculate_sma(prices: &[f64], period: usize) -> Option<f64> {
    if prices.len() < period {
        return None;
    }
    let sum: f64 = prices.iter().rev().take(period).sum();
    Some(sum / period as f64)
}

/// Calculate standard deviation (volatility measure)
fn calculate_std_dev(prices: &[f64]) -> f64 {
    if prices.is_empty() {
        return 0.0;
    }
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    let variance = prices.iter()
        .map(|p| (p - mean).powi(2))
        .sum::<f64>() / prices.len() as f64;
    variance.sqrt()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let adapter = PolygonAdapter::default();

    println!("📊 Fetching 30 days of AAPL daily bars...");

    // Fetch 30 days of daily bars
    let request = BarsRequest {
        symbol: "AAPL".parse()?,
        interval: "1d".parse()?,
        limit: 30,
    };

    let response = adapter.bars(request).await?;

    if let Some(series) = response.bars.first() {
        println!("\n📈 Analysis for {} ({} bars)", series.symbol, series.bars.len());
        println!("{}", "=".repeat(50));

        // Extract closing prices
        let closes: Vec<f64> = series.bars.iter().map(|b| b.close).collect();

        // Calculate SMAs
        let sma_5 = calculate_sma(&closes, 5);
        let sma_10 = calculate_sma(&closes, 10);
        let sma_20 = calculate_sma(&closes, 20);

        println!("\n📊 Moving Averages:");
        println!("  SMA 5:  ${:.2}", sma_5.unwrap_or(0.0));
        println!("  SMA 10: ${:.2}", sma_10.unwrap_or(0.0));
        println!("  SMA 20: ${:.2}", sma_20.unwrap_or(0.0));

        // Calculate volatility
        let volatility = calculate_std_dev(&closes);
        println!("\n📉 Volatility (Std Dev): ${:.2}", volatility);

        // Find high and low
        let high = series.bars.iter().map(|b| b.high).fold(f64::MIN, f64::max);
        let low = series.bars.iter().map(|b| b.low).fold(f64::MAX, f64::min);
        
        println!("\n🎯 Price Range:");
        println!("  30-day High: ${:.2}", high);
        println!("  30-day Low:  ${:.2}", low);
        println!("  Range:       ${:.2} ({:.1}%)", 
            high - low,
            ((high - low) / low) * 100.0
        );

        // Support and resistance (simplified)
        let latest_close = closes.last().unwrap_or(&0.0);
        println!("\n📍 Current Status:");
        println!("  Latest Close: ${:.2}", latest_close);
        
        if let Some(sma_20) = sma_20 {
            if *latest_close > sma_20 {
                println!("  📈 Trading above 20-day SMA (bullish)");
            } else {
                println!("  📉 Trading below 20-day SMA (bearish)");
            }
        }

        // Recent bars table
        println!("\n📅 Recent Bars:");
        println!("┌────────────┬────────┬────────┬────────┬────────┬──────────┐");
        println!("│ Date       │ Open   │ High   │ Low    │ Close  │ Volume   │");
        println!("├────────────┼────────┼────────┼────────┼────────┼──────────┤");
        
        for bar in series.bars.iter().rev().take(5) {
            let vol = bar.volume
                .map(|v| format!("{:,}", v))
                .unwrap_or_else(|| "N/A".to_string());
            println!(
                "│ {} │ {:7.2} │ {:7.2} │ {:7.2} │ {:7.2} │ {:>8} │",
                bar.ts.format("%Y-%m-%d"),
                bar.open, bar.high, bar.low, bar.close, vol
            );
        }
        println!("└────────────┴────────┴────────┴────────┴────────┴──────────┘");
    }

    Ok(())
}
