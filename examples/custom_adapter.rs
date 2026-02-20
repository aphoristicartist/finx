//! # Custom Data Source Adapter Example
//!
//! This example demonstrates how to implement a custom data source
//! by implementing the `DataSource` trait.
//!
//! ## When to implement a custom adapter
//!
//! - Adding support for a new data provider
//! - Creating a mock adapter for testing
//! - Implementing a local data source
//!
//! ## Required trait methods
//!
//! The `DataSource` trait requires implementing:
//! - `id()` - Unique provider identifier
//! - `capabilities()` - Supported endpoints
//! - `quote()` - Fetch quotes
//! - `bars()` - Fetch OHLCV bars
//! - `fundamentals()` - Fetch fundamentals
//! - `search()` - Search instruments
//! - `health()` - Health check

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ferrotick_core::{
    BarsRequest, BarSeries, CapabilitySet, DataSource, Endpoint, FundamentalsBatch,
    FundamentalsRequest, HealthState, HealthStatus, ProviderId, Quote, QuoteBatch,
    QuoteRequest, SearchBatch, SearchRequest, SourceError, Symbol, UtcDateTime,
    Interval, Bar,
};

/// A mock adapter for demonstration purposes.
///
/// This adapter returns static data and is useful for testing
/// or as a starting point for implementing real adapters.
pub struct MockAdapter {
    id: ProviderId,
    capabilities: CapabilitySet,
    score: u16,
}

impl MockAdapter {
    /// Create a new mock adapter with the specified score.
    pub fn new(score: u16) -> Self {
        Self {
            id: ProviderId::new("mock"),
            capabilities: CapabilitySet::full(),
            score,
        }
    }

    /// Create a mock adapter with limited capabilities.
    pub fn with_capabilities(capabilities: CapabilitySet) -> Self {
        Self {
            id: ProviderId::new("mock"),
            capabilities,
            score: 50,
        }
    }
}

impl DataSource for MockAdapter {
    fn id(&self) -> ProviderId {
        self.id.clone()
    }

    fn capabilities(&self) -> CapabilitySet {
        self.capabilities
    }

    fn quote<'a>(
        &'a self,
        req: QuoteRequest,
    ) -> Pin<Box<dyn Future<Output = Result<QuoteBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            // Check capability
            if !self.capabilities.supports(Endpoint::Quote) {
                return Err(SourceError::unsupported_endpoint(Endpoint::Quote));
            }

            // Generate mock quotes
            let quotes: Vec<Quote> = req
                .symbols
                .into_iter()
                .map(|symbol| Quote {
                    symbol,
                    price: 100.0,
                    bid: Some(99.50),
                    ask: Some(100.50),
                    volume: Some(1_000_000),
                    currency: "USD".to_string(),
                    as_of: UtcDateTime::now(),
                })
                .collect();

            Ok(QuoteBatch { quotes })
        })
    }

    fn bars<'a>(
        &'a self,
        req: BarsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<BarSeries, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            // Check capability
            if !self.capabilities.supports(Endpoint::Bars) {
                return Err(SourceError::unsupported_endpoint(Endpoint::Bars));
            }

            // Generate mock bars
            let bars: Vec<Bar> = (0..req.limit.min(100))
                .map(|i| {
                    let base = 100.0 + (i as f64 * 0.5);
                    Bar {
                        ts: UtcDateTime::now(), // In real code, this would be actual timestamps
                        open: base,
                        high: base + 1.0,
                        low: base - 1.0,
                        close: base + 0.5,
                        volume: Some(500_000),
                        vwap: Some(base + 0.25),
                    }
                })
                .collect();

            // Note: In real implementation, you'd create Bars properly with validation
            Ok(BarSeries::new(req.symbol, Interval::Daily, bars))
        })
    }

    fn fundamentals<'a>(
        &'a self,
        _req: FundamentalsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<FundamentalsBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            // For brevity, return unsupported for this example
            Err(SourceError::unsupported_endpoint(Endpoint::Fundamentals))
        })
    }

    fn search<'a>(
        &'a self,
        _req: SearchRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SearchBatch, SourceError>> + Send + 'a>> {
        Box::pin(async move {
            // For brevity, return unsupported for this example
            Err(SourceError::unsupported_endpoint(Endpoint::Search))
        })
    }

    fn health<'a>(&'a self) -> Pin<Box<dyn Future<Output = HealthStatus> + Send + 'a>> {
        Box::pin(async move {
            HealthStatus::healthy(self.score)
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Testing custom MockAdapter\n");

    // Create the adapter
    let adapter = MockAdapter::new(75);

    // Check capabilities
    println!("📊 Adapter capabilities:");
    println!("   ID: {}", adapter.id());
    println!("   Quote: {}", adapter.capabilities().quote);
    println!("   Bars: {}", adapter.capabilities().bars);
    println!("   Fundamentals: {}", adapter.capabilities().fundamentals);
    println!("   Search: {}", adapter.capabilities().search);

    // Check health
    let health = adapter.health().await;
    println!("\n❤️  Health status:");
    println!("   State: {:?}", health.state);
    println!("   Score: {}", health.score);

    // Fetch a quote
    println!("\n📈 Fetching quote...");
    let request = QuoteRequest::new(vec![Symbol::new("AAPL"), Symbol::new("MSFT")])?;
    let response = adapter.quote(request).await?;

    println!("   Received {} quotes:", response.quotes.len());
    for quote in &response.quotes {
        println!(
            "   - {} ${:.2} (bid: ${:.2}, ask: ${:.2})",
            quote.symbol,
            quote.price,
            quote.bid.unwrap_or(0.0),
            quote.ask.unwrap_or(0.0)
        );
    }

    // Test with limited capabilities
    println!("\n🔧 Testing limited capabilities...");
    let limited = MockAdapter::with_capabilities(
        CapabilitySet::new(true, false, false, false)
    );
    
    println!("   Capabilities: {:?}", limited.capabilities().supported_endpoints());
    
    // Try bars (should fail)
    let bars_request = BarsRequest::new(Symbol::new("AAPL"), Interval::Daily, 10)?;
    match limited.bars(bars_request).await {
        Ok(_) => println!("   ❌ Unexpected success!"),
        Err(e) => println!("   ✅ Expected error: {}", e.message()),
    }

    println!("\n✅ Custom adapter example complete!");
    Ok(())
}
