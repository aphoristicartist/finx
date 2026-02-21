# Phase 5 Implementation Plan: Alpha Vantage + Alpaca Adapters

**Timeline**: Weeks 11-12 (Current)
**Dependencies**: Phase 4 (DuckDB Warehouse) - COMPLETE ✅
**Status**: Ready to Start

---

## Overview

Add Alpha Vantage and Alpaca market data providers to complete the four-provider ecosystem (Yahoo, Polygon, Alpha Vantage, Alpaca).

---

## Alpha Vantage Adapter

### API Details

**Base URL**: `https://www.alphavantage.co/query`
**Authentication**: API key via `apikey` parameter
**Rate Limits**:
- Free tier: 5 calls/minute, 500 calls/day
- Standard tier: 75 calls/minute, 3,000 calls/day
- Premium tier: 300 calls/minute, 30,000 calls/day

**Endpoints**:
1. **Quote**: `GLOBAL_QUOTE` function
   - Parameters: `symbol`, `apikey`
   - Returns: Latest price, volume, etc.

2. **Bars**: `TIME_SERIES_INTRADAY`, `TIME_SERIES_DAILY`
   - Parameters: `symbol`, `interval`, `outputsize`, `apikey`
   - Returns: OHLCV time series

3. **Fundamentals**: `OVERVIEW`, `INCOME_STATEMENT`, `BALANCE_SHEET`, `CASH_FLOW`
   - Parameters: `symbol`, `apikey`
   - Returns: Company financials

4. **Search**: `SYMBOL_SEARCH`
   - Parameters: `keywords`, `apikey`
   - Returns: Symbol matches

### Implementation

```rust
// crates/ferrotick-core/src/adapters/alphavantage.rs

pub struct AlphaVantageAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
    rate_limiter: RateLimiter, // NEW: Throttling-aware queue
}

impl AlphaVantageAdapter {
    pub fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 70, // Lower than Polygon (90) due to rate limits
            http_client: Arc::new(NoopHttpClient),
            auth: HttpAuth::Query {
                name: String::from("apikey"),
                value: std::env::var("FERROTICK_ALPHAVANTAGE_API_KEY")
                    .unwrap_or_else(|_| String::from("demo")),
            },
            circuit_breaker: Arc::new(CircuitBreaker::default()),
            rate_limiter: RateLimiter::new(5, 60), // 5 calls per minute
        }
    }
}
```

### Throttling Strategy

Alpha Vantage has strict rate limits, so we need:

1. **Rate Limiter**: Use `governor` crate for token bucket algorithm
   ```rust
   use governor::{Quota, RateLimiter};

   let quota = Quota::per_minute(NonZeroU32::new(5).unwrap());
   let limiter = RateLimiter::direct(quota);
   ```

2. **Queue System**: Buffer requests when rate limit exceeded
   ```rust
   pub struct ThrottlingQueue {
       limiter: RateLimiter,
       pending: VecDeque<PendingRequest>,
   }
   ```

3. **Backoff Policy**: Exponential backoff on rate limit errors
   ```rust
   fn calculate_backoff(retry_count: u32) -> Duration {
       Duration::from_secs(2u64.pow(retry_count))
   }
   ```

### Error Handling

- **Rate Limit**: Return `SourceError::rate_limited` with retry-after hint
- **Quota Exceeded**: Queue request for later execution
- **Invalid API Key**: Return `SourceError::authentication_failed`
- **Network Error**: Use circuit breaker pattern

---

## Alpaca Adapter

### API Details

**Base URL**: `https://data.alpaca.markets/v2`
**Authentication**: API key + secret via headers
**Rate Limits**: No explicit limits, but respect fair use

**Endpoints**:
1. **Quote**: `/stocks/{symbol}/quotes/latest`
   - Headers: `APCA-API-KEY-ID`, `APCA-API-SECRET-KEY`
   - Returns: Latest quote

2. **Bars**: `/stocks/{symbol}/bars`
   - Parameters: `timeframe`, `start`, `end`, `limit`
   - Returns: OHLCV bars

3. **Fundamentals**: Not supported (Alpaca is market data only)
4. **Search**: Not supported (use Polygon or Yahoo for search)

### Implementation

```rust
// crates/ferrotick-core/src/adapters/alpaca.rs

pub struct AlpacaAdapter {
    health_state: HealthState,
    rate_available: bool,
    score: u16,
    http_client: Arc<dyn HttpClient>,
    auth: HttpAuth,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl AlpacaAdapter {
    pub fn default() -> Self {
        Self {
            health_state: HealthState::Healthy,
            rate_available: true,
            score: 85, // High score - reliable, fast
            http_client: Arc::new(NoopHttpClient),
            auth: HttpAuth::Header {
                name: String::from("APCA-API-KEY-ID"),
                value: std::env::var("FERROTICK_ALPACA_API_KEY")
                    .unwrap_or_else(|_| String::from("demo")),
            },
            circuit_breaker: Arc::new(CircuitBreaker::default()),
        }
    }

    fn secret_key(&self) -> String {
        std::env::var("FERROTICK_ALPACA_SECRET_KEY")
            .unwrap_or_else(|_| String::from("demo"))
    }
}
```

### Authentication

Alpaca requires two headers:
- `APCA-API-KEY-ID`: API key
- `APCA-API-SECRET-KEY`: Secret key

```rust
fn build_request(&self, endpoint: &str) -> HttpRequest {
    HttpRequest::get(endpoint)
        .with_header("APCA-API-KEY-ID", &self.api_key())
        .with_header("APCA-API-SECRET-KEY", &self.secret_key())
}
```

### Capability Matrix

```rust
impl DataSource for AlpacaAdapter {
    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::new(
            true,  // quote
            true,  // bars
            false, // fundamentals - NOT SUPPORTED
            false, // search - NOT SUPPORTED
        )
    }
}
```

---

## Routing and Scoring

### Updated Capability Matrix

| Provider    | Quote | Bars | Fundamentals | Search | Score |
|-------------|-------|------|--------------|--------|-------|
| Polygon     | ✅    | ✅   | ✅           | ✅     | 90    |
| Alpaca      | ✅    | ✅   | ❌           | ❌     | 85    |
| Alpha Vantage| ✅   | ✅   | ✅           | ✅     | 70    |
| Yahoo       | ✅    | ✅   | ✅           | ✅     | 60    |

### Routing Strategy

```rust
// crates/ferrotick-core/src/routing.rs

pub fn select_provider(
    endpoint: Endpoint,
    providers: &[ProviderScore],
) -> Option<ProviderId> {
    let capable: Vec<_> = providers
        .iter()
        .filter(|p| p.capabilities.supports(endpoint))
        .collect();

    // Sort by score (descending)
    capable.sort_by(|a, b| b.score.cmp(&a.score));

    capable.first().map(|p| p.provider_id)
}
```

### Fallback Chain

For each endpoint, providers are tried in order:

**Quote**:
1. Polygon (90)
2. Alpaca (85)
3. Alpha Vantage (70)
4. Yahoo (60)

**Bars**:
1. Polygon (90)
2. Alpaca (85)
3. Alpha Vantage (70)
4. Yahoo (60)

**Fundamentals**:
1. Polygon (90)
2. Alpha Vantage (70)
3. Yahoo (60)
4. ~~Alpaca (not supported)~~

**Search**:
1. Polygon (90)
2. Alpha Vantage (70)
3. Yahoo (60)
4. ~~Alpaca (not supported)~~

---

## Per-Provider Policies

### Configuration Structure

```rust
// crates/ferrotick-core/src/provider_policy.rs

#[derive(Debug, Clone)]
pub struct ProviderPolicy {
    pub provider_id: ProviderId,
    pub max_concurrency: usize,
    pub quota_window: Duration,
    pub quota_limit: u32,
    pub retry_backoff: BackoffPolicy,
}

#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub max_retries: u32,
}

impl ProviderPolicy {
    pub fn alphavantage() -> Self {
        Self {
            provider_id: ProviderId::AlphaVantage,
            max_concurrency: 1, // One request at a time
            quota_window: Duration::from_secs(60),
            quota_limit: 5, // 5 calls per minute
            retry_backoff: BackoffPolicy {
                initial_delay: Duration::from_secs(60),
                max_delay: Duration::from_secs(300),
                multiplier: 2.0,
                max_retries: 3,
            },
        }
    }

    pub fn alpaca() -> Self {
        Self {
            provider_id: ProviderId::Alpaca,
            max_concurrency: 10,
            quota_window: Duration::from_secs(60),
            quota_limit: 100, // Generous limits
            retry_backoff: BackoffPolicy {
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(30),
                multiplier: 2.0,
                max_retries: 3,
            },
        }
    }
}
```

---

## Contract Tests

### Shared Test Suite

All four providers must pass the same contract tests:

```rust
// tests/contract/provider_contract.rs

#[test]
fn quote_returns_valid_structure() {
    for provider in &[Yahoo, Polygon, AlphaVantage, Alpaca] {
        let result = provider.quote(QuoteRequest {
            symbols: vec!["AAPL".into()],
            ..Default::default()
        }).await?;

        assert_eq!(result.quotes.len(), 1);
        assert!(result.quotes[0].price > 0.0);
        assert_eq!(result.quotes[0].symbol.as_ref(), "AAPL");
    }
}

#[test]
fn bars_respects_limit() {
    for provider in &[Yahoo, Polygon, AlphaVantage, Alpaca] {
        let result = provider.bars(BarsRequest {
            symbol: "AAPL".into(),
            interval: Interval::Day1,
            limit: 10,
        }).await?;

        assert!(result.bars.len() <= 10);
    }
}
```

### Canonical Output Parity

All providers must return identical canonical structures:

```rust
#[test]
fn canonical_quote_parity() {
    let yahoo_quote = yahoo.quote(...).await?;
    let polygon_quote = polygon.quote(...).await?;
    let alphavantage_quote = alphavantage.quote(...).await?;
    let alpaca_quote = alpaca.quote(...).await?;

    // All should have same structure
    assert!(yahoo_quote.quotes[0].price > 0.0);
    assert!(polygon_quote.quotes[0].price > 0.0);
    assert!(alphavantage_quote.quotes[0].price > 0.0);
    assert!(alpaca_quote.quotes[0].price > 0.0);
}
```

---

## Implementation Steps

### Step 1: Alpha Vantage Adapter (Week 11)
1. ✅ Create `alphavantage.rs` adapter file
2. ✅ Implement `DataSource` trait
3. ✅ Add rate limiter with governor
4. ✅ Implement quote endpoint
5. ✅ Implement bars endpoint
6. ✅ Implement fundamentals endpoint
7. ✅ Implement search endpoint
8. ✅ Add contract tests
9. ✅ Update routing logic

### Step 2: Alpaca Adapter (Week 11)
1. ✅ Create `alpaca.rs` adapter file
2. ✅ Implement `DataSource` trait
3. ✅ Implement quote endpoint
4. ✅ Implement bars endpoint
5. ✅ Add contract tests
6. ✅ Update routing logic

### Step 3: Provider Policies (Week 12)
1. ✅ Create `provider_policy.rs`
2. ✅ Define per-provider configs
3. ✅ Implement throttling queue
4. ✅ Add retry backoff logic
5. ✅ Update router to use policies

### Step 4: Integration Testing (Week 12)
1. ✅ Add full contract test suite
2. ✅ Test all four providers
3. ✅ Test fallback chains
4. ✅ Test rate limiting
5. ✅ Test circuit breaker
6. ✅ Performance benchmarks

### Step 5: Documentation (Week 12)
1. ✅ Update README with new providers
2. ✅ Document API key setup
3. ✅ Update capability matrix
4. ✅ Add provider-specific docs

---

## Acceptance Criteria

### Must Pass:
1. ✅ All providers pass shared contract suite
2. ✅ Source router picks valid provider > 99.9% in simulation
3. ✅ Rate limiting works correctly
4. ✅ Fallback chain functions properly
5. ✅ All 4 providers return canonical structures
6. ✅ Performance: quote < 100ms p95

### Nice to Have:
- Provider health dashboard
- Real-time rate limit monitoring
- Automatic provider failover

---

## Environment Variables

```bash
# Alpha Vantage
export FERROTICK_ALPHAVANTAGE_API_KEY="your-key-here"

# Alpaca
export FERROTICK_ALPACA_API_KEY="your-key-id-here"
export FERROTICK_ALPACA_SECRET_KEY="your-secret-key-here"
```

---

## Files to Create/Modify

### New Files:
- `crates/ferrotick-core/src/adapters/alphavantage.rs`
- `crates/ferrotick-core/src/adapters/alpaca.rs`
- `crates/ferrotick-core/src/provider_policy.rs`
- `crates/ferrotick-core/src/throttling.rs`
- `tests/contract/provider_contract.rs`

### Modified Files:
- `crates/ferrotick-core/src/adapters/mod.rs` - Add new adapters
- `crates/ferrotick-core/src/routing.rs` - Update scoring/routing
- `crates/ferrotick-core/src/lib.rs` - Export new types
- `crates/ferrotick-cli/src/commands/sources.rs` - Update provider list
- `README.md` - Document new providers

---

## Estimated Effort

- **Alpha Vantage**: 2-3 days (with rate limiting)
- **Alpaca**: 1-2 days (simpler, no fundamentals)
- **Provider Policies**: 1 day
- **Testing**: 2 days
- **Documentation**: 0.5 days

**Total**: ~7-9 days (within 2-week timeline)

---

## Ready to Start? ✅

All dependencies are met (Phase 4 complete). Let's begin with Alpha Vantage adapter implementation!

**Next Step**: Create `alphavantage.rs` adapter file
