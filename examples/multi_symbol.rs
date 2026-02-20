//! # Multi-Symbol Quote Example
//!
//! This example demonstrates how to fetch quotes for multiple symbols
//! in a single request.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example multi_symbol
//! ```
//!
//! ## Output
//!
//! ```text
//! 📊 Fetching quotes for 5 symbols...
//! ✅ AAPL: $178.52 (Volume: 52,847,392)
//! ✅ MSFT: $402.56 (Volume: 21,234,567)
//! ✅ GOOGL: $141.80 (Volume: 18,456,789)
//! ...
//! ```

use ferrotick_core::{PolygonAdapter, QuoteRequest, DataSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the adapter
    let adapter = PolygonAdapter::default();

    // Define the symbols we want to fetch
    let symbols = vec![
        "AAPL".parse()?,
        "MSFT".parse()?,
        "GOOGL".parse()?,
        "AMZN".parse()?,
        "META".parse()?,
    ];

    println!("📊 Fetching quotes for {} symbols...", symbols.len());

    // Create and send the request
    let request = QuoteRequest { symbols };
    let response = adapter.quote(request).await?;

    // Display results in a formatted table
    println!();
    println!("┌────────┬──────────┬──────────────┬──────────┐");
    println!("│ Symbol │ Price    │ Volume       │ Currency │");
    println!("├────────┼──────────┼──────────────┼──────────┤");

    for quote in &response.quotes {
        let volume = quote.volume
            .map(|v| format!("{:,}", v))
            .unwrap_or_else(|| "N/A".to_string());
        
        println!(
            "│ {:6} │ ${:7.2} │ {:12} │ {:8} │",
            quote.symbol.as_str(),
            quote.price,
            volume,
            quote.currency
        );
    }

    println!("└────────┴──────────┴──────────────┴──────────┘");
    println!();
    println!("✅ Fetched {} quotes in {}ms", 
        response.quotes.len(),
        response.latency_ms.unwrap_or(0)
    );

    Ok(())
}
