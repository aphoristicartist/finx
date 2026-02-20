//! # Basic Quote Example
//!
//! This is the simplest possible example demonstrating how to fetch
//! a stock quote using Ferrotick.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example basic_quote
//! ```
//!
//! ## Prerequisites
//!
//! Set your Polygon API key (or use the demo key for testing):
//!
//! ```bash
//! export POLYGON_API_KEY=your_key_here
//! ```

use ferrotick_core::{PolygonAdapter, QuoteRequest, DataSource};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the Polygon adapter with default configuration
    // This reads the POLYGON_API_KEY environment variable
    let adapter = PolygonAdapter::default();

    // Create a quote request for AAPL
    let request = QuoteRequest {
        symbols: vec!["AAPL".parse()?],
    };

    // Fetch the quote
    println!("📊 Fetching quote for AAPL...");
    let response = adapter.quote(request).await?;

    // Print the result
    if let Some(quote) = response.quotes.first() {
        println!("✅ Symbol: {}", quote.symbol);
        println!("💰 Price: ${:.2}", quote.price);
        println!("📉 Bid: ${:.2}", quote.bid.unwrap_or(0.0));
        println!("📈 Ask: ${:.2}", quote.ask.unwrap_or(0.0));
        println!("📦 Volume: {:?}", quote.volume);
        println!("🕐 As of: {}", quote.as_of);
    }

    Ok(())
}
