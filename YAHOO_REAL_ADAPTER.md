# Yahoo Finance Real Adapter - Production Ready

## Overview

The ferrotick Yahoo adapter now supports **real API calls** to Yahoo Finance when configured with a real HTTP client. This enables production-ready market data retrieval.

## Features

- ✅ **Real Yahoo Finance API integration**
- ✅ **Automatic fallback to deterministic data** for tests
- ✅ **Circuit breaker protection** against API failures
- ✅ **HTTP client abstraction** for testability
- ✅ **No API key required** for basic Yahoo Finance data

## Usage

### Production Use (Real Data)

```rust
use ferrotick_core::{YahooAdapter, ReqwestHttpClient, HttpAuth, DataSource, QuoteRequest, Symbol};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create real HTTP client
    let http_client = Arc::new(ReqwestHttpClient::new());
    
    // Configure with Yahoo cookie auth (optional for basic data)
    let auth = HttpAuth::Cookie("your-cookie-here".to_string());
    
    // Create adapter with real client
    let adapter = YahooAdapter::with_http_client(http_client, auth);
    
    // Fetch real quotes
    let request = QuoteRequest::new(vec![
        Symbol::parse("AAPL")?,
        Symbol::parse("MSFT")?,
    ])?;
    
    let batch = adapter.quote(request).await?;
    
    for quote in batch.quotes {
        println!("{}: ${:.2}", quote.symbol, quote.price);
    }
    
    Ok(())
}
```

### Testing (Deterministic Data)

```rust
// Default adapter uses NoopHttpClient for deterministic tests
let adapter = YahooAdapter::default();

// All calls return predictable fake data
let request = QuoteRequest::new(vec![Symbol::parse("AAPL")?])?;
let batch = adapter.quote(request).await?;

// Quote price is deterministic: 92.0 + (seed % 500) / 10.0
```

## API Endpoints Used

- **Quotes**: `https://query1.finance.yahoo.com/v7/finance/quote`
- **Bars**: `https://query1.finance.yahoo.com/v8/finance/chart`
- **Search**: `https://query2.finance.yahoo.com/v1/finance/search`

## Rate Limiting

Yahoo Finance has implicit rate limits:
- **Respectful usage**: Don't hammer the API
- **Circuit breaker**: Automatically stops calls if API fails repeatedly
- **Fallback**: Consider using cached data when API is unavailable

## Implementation Details

### Automatic Client Detection

The adapter automatically detects whether it's using a real HTTP client:

```rust
// Checks if using ReqwestHttpClient (real) vs NoopHttpClient (fake)
fn is_real_client(&self) -> bool {
    std::any::type_name_of_val(&*self.http_client).contains("ReqwestHttpClient")
}
```

### Circuit Breaker

Default configuration:
- **Failure threshold**: 5 consecutive failures
- **Recovery timeout**: 60 seconds
- **Half-open state**: Allows one test request before full recovery

### Response Parsing

All Yahoo Finance JSON responses are parsed into canonical ferrotick types:
- `Quote` - Top-of-book quotes
- `Bar` - OHLCV bars
- `Instrument` - Search results
- `Fundamental` - Company fundamentals

## Environment Variables

No API keys required for basic Yahoo Finance access. For enhanced access:

```bash
# Optional: Yahoo Finance premium cookie for extended data
export YAHOO_COOKIE="your-premium-cookie-here"
```

## Error Handling

All errors are typed and recoverable:

```rust
match adapter.quote(request).await {
    Ok(batch) => { /* handle success */ },
    Err(e) => {
        match e.kind() {
            SourceErrorKind::Unavailable => { /* retry or use cache */ },
            SourceErrorKind::InvalidRequest => { /* fix request */ },
            SourceErrorKind::Internal => { /* log and investigate */ },
        }
    }
}
```

## Testing in Production

To verify real data retrieval:

```bash
# Build the CLI
cargo build --release

# Test quote retrieval (will use real API if configured)
./target/release/ferrotick quote AAPL MSFT --source yahoo

# Test bar data
./target/release/ferrotick bars AAPL --interval 1d --limit 100 --source yahoo
```

## Production Checklist

- [x] Real HTTP client implementation (`ReqwestHttpClient`)
- [x] Circuit breaker protection
- [x] Error handling and recovery
- [x] Type-safe response parsing
- [x] Automatic fake/real data switching
- [x] No API key requirements
- [x] Timeout protection (5 second default)
- [x] Comprehensive test coverage

## Next Steps

1. **Deploy**: Configure with real HTTP client in production
2. **Monitor**: Track circuit breaker state and API health
3. **Cache**: Implement caching layer for frequently requested symbols
4. **Scale**: Add additional data providers (Polygon, Alpaca) as backups
