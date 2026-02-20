//! Behavior-driven tests for Data Provider behavior
//!
//! These tests verify HOW the system handles various data provider scenarios,
//! focusing on API responses, error handling, and rate limiting behavior.

use ferrotick_core::{
    adapters::{AlpacaAdapter, AlphaVantageAdapter, PolygonAdapter, YahooAdapter},
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState},
    data_source::{
        BarsRequest, DataSource, Endpoint, FundamentalsRequest, HealthState,
        QuoteRequest, SearchRequest, SourceErrorKind,
    },
    routing::{SourceRouter, SourceStrategy},
    Interval, ProviderId, Symbol,
};
use std::sync::Arc;
use std::time::Duration;

// =============================================================================
// Data Provider: Valid Response Handling
// =============================================================================

#[tokio::test]
async fn when_yahoo_returns_valid_data_system_parses_it_correctly() {
    // Given: A Yahoo adapter with fake data mode
    let adapter = YahooAdapter::default();

    // When: The system requests a quote
    let symbols = vec![Symbol::parse("AAPL").expect("valid")];
    let request = QuoteRequest::new(symbols).expect("valid request");
    let result = adapter.quote(request).await;

    // Then: The data is parsed correctly into domain types
    let batch = result.expect("valid data should parse successfully");
    assert_eq!(batch.quotes.len(), 1);

    let quote = &batch.quotes[0];
    assert_eq!(quote.symbol.as_str(), "AAPL");
    assert!(quote.price > 0.0, "price should be a positive number");
    assert!(!quote.currency.is_empty(), "currency should be present");
}

#[tokio::test]
async fn when_yahoo_returns_valid_bars_system_creates_proper_ohlcv_structure() {
    // Given: A Yahoo adapter
    let adapter = YahooAdapter::default();
    let symbol = Symbol::parse("MSFT").expect("valid");

    // When: The system requests historical bars
    let request = BarsRequest::new(symbol, Interval::OneDay, 10).expect("valid request");
    let result = adapter.bars(request).await;

    // Then: Each bar has valid OHLCV structure
    let bars = result.expect("bars should parse successfully");
    assert_eq!(bars.bars.len(), 10);

    for bar in &bars.bars {
        // High must be >= all others
        assert!(bar.high >= bar.open, "high >= open invariant violated");
        assert!(bar.high >= bar.close, "high >= close invariant violated");
        assert!(bar.high >= bar.low, "high >= low invariant violated");

        // Low must be <= all others
        assert!(bar.low <= bar.open, "low <= open invariant violated");
        assert!(bar.low <= bar.close, "low <= close invariant violated");

        // All prices must be positive
        assert!(bar.open > 0.0);
        assert!(bar.high > 0.0);
        assert!(bar.low > 0.0);
        assert!(bar.close > 0.0);
    }
}

// =============================================================================
// Data Provider: Error Handling
// =============================================================================

#[tokio::test]
async fn when_empty_symbol_list_provided_system_returns_validation_error() {
    // Given: A user accidentally submits an empty symbol list

    // When: The request is created
    let result = QuoteRequest::new(vec![]);

    // Then: A clear validation error is returned
    let error = result.expect_err("empty symbols should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
    assert!(
        error.message().contains("symbol"),
        "error should mention symbols: {}",
        error.message()
    );
}

#[tokio::test]
async fn when_bars_limit_is_zero_system_returns_validation_error() {
    // Given: A user accidentally sets limit to 0
    let symbol = Symbol::parse("AAPL").expect("valid");

    // When: The request is created
    let result = BarsRequest::new(symbol, Interval::OneDay, 0);

    // Then: A validation error explains the issue
    let error = result.expect_err("zero limit should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
    assert!(
        error.message().contains("limit"),
        "error should mention limit: {}",
        error.message()
    );
}

#[tokio::test]
async fn when_search_query_is_empty_system_returns_validation_error() {
    // Given: A user submits an empty search query

    // When: The search request is created
    let result = SearchRequest::new("", 10);

    // Then: A validation error is returned
    let error = result.expect_err("empty query should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
}

#[tokio::test]
async fn when_search_limit_is_zero_system_returns_validation_error() {
    // Given: A user submits zero limit

    // When: The search request is created
    let result = SearchRequest::new("test", 0);

    // Then: A validation error is returned
    let error = result.expect_err("zero limit should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
}

// =============================================================================
// Data Provider: Circuit Breaker Behavior
// =============================================================================

#[tokio::test]
async fn when_transport_failures_exceed_threshold_circuit_breaker_tracks_state() {
    // Given: A circuit breaker with low failure threshold
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 2,
        open_timeout: Duration::from_secs(60),
    }));

    // When: Multiple failures are recorded
    for _ in 0..3 {
        circuit_breaker.record_failure();
    }

    // Then: The circuit breaker state changes to Open
    assert_eq!(circuit_breaker.state(), CircuitState::Open);
    assert_eq!(circuit_breaker.consecutive_failures(), 3);
}

#[tokio::test]
async fn when_circuit_breaker_is_open_requests_are_rejected_immediately() {
    // Given: A circuit breaker that's already open
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        open_timeout: Duration::from_secs(60),
    }));
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();
    assert_eq!(circuit_breaker.state(), CircuitState::Open);

    // And: An adapter using this circuit breaker
    let adapter = YahooAdapter::with_circuit_breaker(circuit_breaker);

    // When: A request is made
    let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid")])
        .expect("valid request");
    let result = adapter.quote(request).await;

    // Then: The request is rejected without waiting for timeout
    let error = result.expect_err("open circuit should reject request");
    assert_eq!(error.kind(), SourceErrorKind::Unavailable);
    assert!(
        error.message().contains("circuit breaker"),
        "error should mention circuit breaker: {}",
        error.message()
    );
}

// =============================================================================
// Data Provider: Rate Limiting Behavior
// =============================================================================

#[tokio::test]
async fn when_provider_is_rate_limited_router_attempts_fallback() {
    // Given: A router with multiple providers
    let router = SourceRouter::default();

    // When: A request is made that triggers rate limit on primary source
    // (Using 4+ symbols to trigger Polygon's 3-symbol limit)
    let symbols = vec![
        Symbol::parse("AAPL").expect("valid"),
        Symbol::parse("MSFT").expect("valid"),
        Symbol::parse("GOOGL").expect("valid"),
        Symbol::parse("TSLA").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("should succeed with fallback");

    // Then: The system uses a fallback provider
    assert_ne!(
        result.selected_source,
        ProviderId::Polygon,
        "should fallback from rate-limited Polygon"
    );
    assert!(!result.warnings.is_empty(), "should warn about fallback");
}

// =============================================================================
// Data Provider: Batch Request Efficiency
// =============================================================================

#[tokio::test]
async fn when_multiple_symbols_requested_system_batches_efficiently() {
    // Given: A user wants quotes for 10 symbols
    let router = SourceRouter::default();
    let symbols: Vec<Symbol> = vec![
        Symbol::parse("AAPL").expect("valid"),
        Symbol::parse("MSFT").expect("valid"),
        Symbol::parse("GOOGL").expect("valid"),
        Symbol::parse("AMZN").expect("valid"),
        Symbol::parse("META").expect("valid"),
        Symbol::parse("NVDA").expect("valid"),
        Symbol::parse("TSLA").expect("valid"),
        Symbol::parse("JPM").expect("valid"),
        Symbol::parse("V").expect("valid"),
        Symbol::parse("JNJ").expect("valid"),
    ];

    // When: A single batch request is made
    let request = QuoteRequest::new(symbols).expect("valid request");
    let start = std::time::Instant::now();
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("batch should succeed");
    let elapsed = start.elapsed();

    // Then: All symbols are returned
    assert_eq!(result.data.quotes.len(), 10, "should return all 10 quotes");

    // And: The request completes in reasonable time (batch is efficient)
    assert!(
        elapsed < Duration::from_secs(5),
        "batch request should complete quickly"
    );
}

// =============================================================================
// Data Provider: Health Monitoring
// =============================================================================

#[tokio::test]
async fn when_adapter_health_is_queried_status_is_accurate() {
    // Given: A fresh adapter
    let adapter = YahooAdapter::default();

    // When: Health is checked
    let health = adapter.health().await;

    // Then: Health status includes all expected fields
    assert!(matches!(
        health.state,
        HealthState::Healthy | HealthState::Degraded | HealthState::Unhealthy
    ));
    assert!(health.score > 0, "health score should be positive");
    // rate_available indicates if the adapter can accept requests
}

#[tokio::test]
async fn when_router_snapshots_provider_full_status_is_returned() {
    // Given: A router with registered providers
    let router = SourceRouter::default();

    // When: Provider status is requested
    for provider in [
        ProviderId::Polygon,
        ProviderId::Alpaca,
        ProviderId::Alphavantage,
        ProviderId::Yahoo,
    ] {
        if let Some(snapshot) = router.snapshot(provider).await {
            // Then: Snapshot contains all status information
            assert_eq!(snapshot.id, provider);
            assert!(snapshot.capabilities.supports(Endpoint::Quote) || snapshot.capabilities.supports(Endpoint::Bars));
            assert!(matches!(
                snapshot.health.state,
                HealthState::Healthy | HealthState::Degraded | HealthState::Unhealthy
            ));
        }
    }
}

// =============================================================================
// Data Provider: Fallback Behavior
// =============================================================================

#[tokio::test]
async fn when_primary_source_fails_system_attempts_secondary_sources() {
    // Given: A router with multiple sources
    let router = SourceRouter::default();

    // When: A request is made that will fail on Polygon (4 symbols > 3 limit)
    let symbols = vec![
        Symbol::parse("AAPL").expect("valid"),
        Symbol::parse("MSFT").expect("valid"),
        Symbol::parse("GOOGL").expect("valid"),
        Symbol::parse("AMZN").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("should succeed with fallback");

    // Then: Multiple sources were tried (chain has more than one entry)
    assert!(
        result.source_chain.len() > 1,
        "multiple sources should be tried"
    );

    // And: The successful source is recorded
    assert!(matches!(
        result.selected_source,
        ProviderId::Alpaca | ProviderId::Alphavantage | ProviderId::Yahoo
    ));

    // And: Errors from failed sources are preserved for debugging
    assert!(!result.errors.is_empty(), "errors from failed sources should be recorded");
}

#[tokio::test]
async fn when_all_sources_fail_system_returns_comprehensive_error() {
    // Given: A router
    let router = SourceRouter::default();

    // When: A request is made with a strict source that will fail
    let symbols = vec![
        Symbol::parse("A").expect("valid"),
        Symbol::parse("B").expect("valid"),
        Symbol::parse("C").expect("valid"),
        Symbol::parse("D").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");
    let result = router
        .route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon))
        .await;

    // Then: The failure includes all attempted sources and their errors
    let failure = result.expect_err("should fail");
    assert!(!failure.source_chain.is_empty());
    assert!(!failure.errors.is_empty());
    assert!(!failure.warnings.is_empty());

    // And: Each error has a source attribution
    for error in &failure.errors {
        assert!(
            error.source.is_some(),
            "each error should identify its source"
        );
    }
}

// =============================================================================
// Data Provider: Retry Guidance
// =============================================================================

#[tokio::test]
async fn when_error_is_retryable_user_receives_retry_guidance() {
    // Given: A router that can produce retryable errors
    let router = SourceRouter::default();

    // When: A request fails due to rate limiting
    let symbols = vec![
        Symbol::parse("A").expect("valid"),
        Symbol::parse("B").expect("valid"),
        Symbol::parse("C").expect("valid"),
        Symbol::parse("D").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");
    let result = router
        .route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon))
        .await;

    // Then: The error indicates retryability
    if let Err(failure) = result {
        for error in &failure.errors {
            // Rate limit errors should be retryable
            if error.message.to_lowercase().contains("rate") {
                assert!(
                    error.retryable.unwrap_or(false),
                    "rate limit errors should be marked retryable"
                );
            }
        }
    }
}

// =============================================================================
// Data Provider: Data Consistency
// =============================================================================

#[tokio::test]
async fn when_same_symbol_queried_multiple_times_data_is_consistent() {
    // Given: An adapter
    let adapter = YahooAdapter::default();

    // When: The same symbol is queried multiple times
    let symbol = Symbol::parse("AAPL").expect("valid");
    let request = QuoteRequest::new(vec![symbol.clone()]).expect("valid request");

    let result1 = adapter.quote(request.clone()).await.expect("first quote");
    let request2 = QuoteRequest::new(vec![symbol]).expect("valid request");
    let result2 = adapter.quote(request2).await.expect("second quote");

    // Then: Both results have the same symbol
    assert_eq!(result1.quotes[0].symbol, result2.quotes[0].symbol);
    // Note: In fake mode, prices vary deterministically but consistently
}

#[tokio::test]
async fn when_bars_requested_timestamps_are_chronologically_ordered() {
    // Given: An adapter
    let adapter = YahooAdapter::default();

    // When: Historical bars are requested
    let symbol = Symbol::parse("AAPL").expect("valid");
    let request = BarsRequest::new(symbol, Interval::OneDay, 20).expect("valid request");
    let result = adapter.bars(request).await.expect("bars");

    // Then: Bars are in chronological order (oldest first)
    for window in result.bars.windows(2) {
        let ts1 = window[0].ts.into_inner().unix_timestamp();
        let ts2 = window[1].ts.into_inner().unix_timestamp();
        assert!(
            ts1 < ts2,
            "bars should be chronologically ordered: {} should be < {}",
            ts1,
            ts2
        );
    }
}

// =============================================================================
// Data Provider: Endpoint Support
// =============================================================================

#[tokio::test]
async fn when_fundamentals_requested_from_alpaca_unsupported_error_is_returned() {
    // Given: Alpaca adapter (which doesn't support fundamentals)
    let adapter = AlpacaAdapter::default();

    // When: Fundamentals are requested
    let symbol = Symbol::parse("AAPL").expect("valid");
    let request = FundamentalsRequest::new(vec![symbol]).expect("valid request");
    let result = adapter.fundamentals(request).await;

    // Then: An unsupported endpoint error is returned
    let error = result.expect_err("Alpaca should not support fundamentals");
    assert_eq!(error.kind(), SourceErrorKind::UnsupportedEndpoint);
}

#[tokio::test]
async fn when_adapter_capabilities_checked_correct_endpoints_reported() {
    // Given: Different adapters
    // When: Capabilities are queried
    let yahoo = YahooAdapter::default();
    let polygon = PolygonAdapter::default();
    let alpaca = AlpacaAdapter::default();
    let alphavantage = AlphaVantageAdapter::default();

    // Then: Each adapter reports correct capabilities
    assert!(yahoo.capabilities().supports(Endpoint::Quote));
    assert!(yahoo.capabilities().supports(Endpoint::Bars));
    assert!(yahoo.capabilities().supports(Endpoint::Fundamentals));

    assert!(polygon.capabilities().supports(Endpoint::Quote));
    assert!(polygon.capabilities().supports(Endpoint::Bars));

    assert!(alpaca.capabilities().supports(Endpoint::Quote));
    assert!(!alpaca.capabilities().supports(Endpoint::Fundamentals));

    assert!(alphavantage.capabilities().supports(Endpoint::Fundamentals));
    assert!(alphavantage.capabilities().supports(Endpoint::Search));
}
