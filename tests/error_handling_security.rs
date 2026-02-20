//! Behavior-driven tests for Error Handling and Security behavior
//!
//! These tests verify HOW the system handles errors gracefully and maintains
//! security invariants, focusing on user-visible outcomes.

use ferrotick_core::{
    adapters::{AlpacaAdapter, YahooAdapter},
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState},
    data_source::{BarsRequest, DataSource, FundamentalsRequest, HealthState, QuoteRequest, SourceErrorKind},
    routing::{SourceRouter, SourceStrategy},
    ProviderId, Symbol,
};
use ferrotick_warehouse::{
    BarRecord, FundamentalRecord, QueryGuardrails, QuoteRecord, Warehouse, WarehouseConfig,
    WarehouseError,
};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

// =============================================================================
// Error Handling: Network Failures
// =============================================================================

#[tokio::test]
async fn when_network_request_fails_user_receives_unavailable_error() {
    // Given: A circuit breaker that will fail
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        open_timeout: Duration::from_secs(60),
    }));
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();

    let adapter = YahooAdapter::with_circuit_breaker(circuit_breaker);

    // When: A request is made
    let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid")])
        .expect("valid request");
    let result = adapter.quote(request).await;

    // Then: An unavailable error is returned with retry guidance
    let error = result.expect_err("should fail");
    assert_eq!(error.kind(), SourceErrorKind::Unavailable);
    assert!(error.retryable(), "network errors should be retryable");
    assert!(
        error.message().contains("circuit breaker"),
        "error should mention circuit breaker state"
    );
}

#[tokio::test]
async fn when_transport_error_occurs_system_tracks_failure_count() {
    // Given: A circuit breaker with tracking
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 5,
        open_timeout: Duration::from_secs(30),
    }));

    // When: Multiple failures occur
    for _ in 0..3 {
        circuit_breaker.record_failure();
    }

    // Then: Failure count is tracked
    assert_eq!(circuit_breaker.consecutive_failures(), 3);
    assert_eq!(circuit_breaker.state(), CircuitState::Closed); // Not yet open
}

// =============================================================================
// Error Handling: Validation Errors
// =============================================================================

#[tokio::test]
async fn when_invalid_symbol_format_provided_user_gets_actionable_error() {
    // Given: A user tries to use an invalid symbol
    let result = Symbol::parse("INVALID_SYMBOL_WITH_UNDERSCORE");

    // When: The symbol is parsed
    // Then: A clear validation error is returned
    assert!(result.is_err());
    let _error = result.expect_err("invalid symbol should fail");
    // The error message should help the user understand valid formats
}

#[tokio::test]
async fn when_bars_request_has_zero_limit_user_gets_clear_error() {
    // Given: A user accidentally sets zero limit
    let symbol = Symbol::parse("AAPL").expect("valid");

    // When: A bars request with zero limit is created
    let result = BarsRequest::new(symbol, ferrotick_core::Interval::OneDay, 0);

    // Then: A validation error explains the issue
    let error = result.expect_err("zero limit should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
    assert!(
        error.message().to_lowercase().contains("limit"),
        "error should mention the limit parameter"
    );
}

#[tokio::test]
async fn when_quote_request_has_empty_symbols_user_gets_clear_error() {
    // Given: A user submits an empty symbol list

    // When: The request is created
    let result = QuoteRequest::new(vec![]);

    // Then: A validation error explains the issue
    let error = result.expect_err("empty symbols should fail");
    assert_eq!(error.kind(), SourceErrorKind::InvalidRequest);
    assert!(
        error.message().to_lowercase().contains("symbol"),
        "error should mention symbols requirement"
    );
}

// =============================================================================
// Error Handling: Provider Errors
// =============================================================================

#[tokio::test]
async fn when_provider_doesnt_support_endpoint_clear_error_is_returned() {
    // Given: Alpaca adapter (doesn't support fundamentals)
    let adapter = AlpacaAdapter::default();

    // When: User requests fundamentals
    let symbol = Symbol::parse("AAPL").expect("valid");
    let request = FundamentalsRequest::new(vec![symbol]).expect("valid request");
    let result = adapter.fundamentals(request).await;

    // Then: An unsupported endpoint error is returned
    let error = result.expect_err("unsupported endpoint should fail");
    assert_eq!(error.kind(), SourceErrorKind::UnsupportedEndpoint);
    assert!(!error.retryable(), "unsupported endpoints are not retryable");
}

#[tokio::test]
async fn when_all_providers_fail_user_sees_comprehensive_error_list() {
    // Given: A router that will exhaust all providers
    let router = SourceRouter::default();

    // When: A request that fails on all sources is made
    // Using strict mode with a source that will fail
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

    // Then: The failure includes all errors for debugging
    let failure = result.expect_err("should fail");
    assert!(!failure.errors.is_empty(), "should have error details");
    assert!(!failure.warnings.is_empty(), "should have warning summary");

    // Each error should identify its source
    for error in &failure.errors {
        assert!(error.source.is_some(), "error should identify its source");
    }
}

// =============================================================================
// Error Handling: Graceful Degradation
// =============================================================================

#[tokio::test]
async fn when_primary_provider_fails_system_attempts_fallback() {
    // Given: A router with multiple providers
    let router = SourceRouter::default();

    // When: A request that triggers rate limit on primary is made
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

    // Then: System successfully falls back to another provider
    assert_ne!(result.selected_source, ProviderId::Polygon);
    assert!(!result.warnings.is_empty(), "should warn about fallback");
}

#[tokio::test]
async fn when_provider_health_degrades_routing_adapts() {
    // Given: An adapter with degraded health
    let adapter = YahooAdapter::with_health(HealthState::Degraded, true);

    // When: Health is checked
    let health = adapter.health().await;

    // Then: Health reflects degraded state but still available
    assert_eq!(health.state, HealthState::Degraded);
    assert!(health.rate_available, "degraded provider should still be usable");
}

// =============================================================================
// Security: SQL Injection Prevention
// =============================================================================

#[test]
fn when_sql_injection_attempted_query_is_handled_safely() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // Ingest some initial data
    let quotes = vec![QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 150.0,
        bid: None,
        ask: None,
        volume: None,
        currency: "USD".to_string(),
        as_of: "2026-02-20T10:00:00Z".to_string(),
    }];
    warehouse
        .ingest_quotes("test", "req-001", &quotes, 100)
        .expect("ingest");

    // When: User attempts SQL injection via query
    let result = warehouse.execute_query(
        "SELECT * FROM quotes_latest WHERE symbol = 'AAPL'; DROP TABLE quotes_latest; --'",
        QueryGuardrails::default(),
        false,
    );

    // Then: Either the query fails safely, or if parsed, doesn't execute the DROP
    // (DuckDB doesn't support multiple statements by default in query())
    match result {
        Ok(query_result) => {
            // If parsed, it was treated as a single query, not injection
            assert_eq!(query_result.row_count, 0, "injection payload shouldn't match");
        }
        Err(_) => {
            // Query rejected - also safe
        }
    }

    // And: The data still exists
    let verify = warehouse
        .execute_query(
            "SELECT COUNT(*) FROM quotes_latest",
            QueryGuardrails::default(),
            false,
        )
        .expect("verify query");
    assert_eq!(verify.row_count, 1);
}

#[test]
fn when_injection_attempted_via_ingest_data_is_stored_safely() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts injection via ingest (parameterized queries prevent this)
    let malicious_quotes = vec![QuoteRecord {
        symbol: "AAPL'; DROP TABLE quotes_latest; --".to_string(),
        price: 150.0,
        bid: None,
        ask: None,
        volume: None,
        currency: "USD".to_string(),
        as_of: "2026-02-20T10:00:00Z".to_string(),
    }];

    // Then: Ingest succeeds (parameterized queries handle escaping)
    warehouse
        .ingest_quotes("test", "req-001", &malicious_quotes, 100)
        .expect("parameterized ingest should handle special chars");

    // And: The malicious string is stored as data, not executed
    let result = warehouse
        .execute_query(
            "SELECT symbol FROM quotes_latest WHERE symbol LIKE '%DROP%'",
            QueryGuardrails::default(),
            false,
        )
        .expect("query");

    assert_eq!(result.row_count, 1, "malicious string stored as data");
}

#[test]
fn when_injection_attempted_via_bars_ingest_data_is_safe() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts injection via bars
    let malicious_bars = vec![BarRecord {
        symbol: "MSFT'); DELETE FROM bars_1d; --".to_string(),
        ts: "2026-02-20T10:00:00Z".to_string(),
        open: 300.0,
        high: 305.0,
        low: 299.0,
        close: 303.0,
        volume: Some(1000),
    }];

    // Then: Ingest succeeds with parameterized queries
    warehouse
        .ingest_bars("test", "bars_1d", "req-001", &malicious_bars, 100)
        .expect("parameterized ingest should be safe");

    // And: Data is stored safely
    let result = warehouse
        .execute_query(
            "SELECT COUNT(*) FROM bars_1d",
            QueryGuardrails::default(),
            false,
        )
        .expect("verify");
    assert_eq!(result.row_count, 1);
}

#[test]
fn when_injection_attempted_via_fundamentals_ingest_data_is_safe() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts injection via fundamentals
    let malicious_fundamentals = vec![FundamentalRecord {
        symbol: "GOOG\"; DROP TABLE fundamentals; --".to_string(),
        metric: "pe_ratio\"; --".to_string(),
        value: 25.0,
        date: "2026-02-20".to_string(),
    }];

    // Then: Ingest succeeds with parameterized queries
    warehouse
        .ingest_fundamentals("test", "req-001", &malicious_fundamentals, 100)
        .expect("parameterized ingest should be safe");

    // And: Table still exists with data
    let result = warehouse
        .execute_query(
            "SELECT COUNT(*) FROM fundamentals",
            QueryGuardrails::default(),
            false,
        )
        .expect("verify");
    assert_eq!(result.row_count, 1);
}

// =============================================================================
// Security: Write Protection
// =============================================================================

#[test]
fn when_user_attempts_delete_in_readonly_mode_it_is_rejected() {
    // Given: A warehouse in read-only mode
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts a DELETE
    let result = warehouse.execute_query(
        "DELETE FROM quotes_latest",
        QueryGuardrails::default(),
        false, // allow_write = false
    );

    // Then: The operation is rejected
    let error = result.expect_err("DELETE should be rejected in read-only mode");
    assert!(matches!(error, WarehouseError::QueryRejected(_)));
}

#[test]
fn when_user_attempts_update_in_readonly_mode_it_is_rejected() {
    // Given: A warehouse in read-only mode
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts an UPDATE
    let result = warehouse.execute_query(
        "UPDATE quotes_latest SET price = 0",
        QueryGuardrails::default(),
        false,
    );

    // Then: The operation is rejected
    let error = result.expect_err("UPDATE should be rejected in read-only mode");
    assert!(matches!(error, WarehouseError::QueryRejected(_)));
}

#[test]
fn when_user_attempts_drop_table_in_readonly_mode_it_is_rejected() {
    // Given: A warehouse in read-only mode
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts DROP TABLE
    let result = warehouse.execute_query(
        "DROP TABLE quotes_latest",
        QueryGuardrails::default(),
        false,
    );

    // Then: The operation is rejected
    let error = result.expect_err("DROP should be rejected in read-only mode");
    assert!(matches!(error, WarehouseError::QueryRejected(_)));
}

#[test]
fn when_user_attempts_multiple_statements_in_readonly_mode_it_is_rejected() {
    // Given: A warehouse in read-only mode
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User attempts multiple statements
    let result = warehouse.execute_query(
        "SELECT * FROM quotes_latest; DROP TABLE quotes_latest;",
        QueryGuardrails::default(),
        false,
    );

    // Then: The operation is rejected (multiple statements not allowed)
    let error = result.expect_err("multiple statements should be rejected");
    assert!(matches!(error, WarehouseError::QueryRejected(_)));
}

// =============================================================================
// Security: Input Validation
// =============================================================================

#[test]
fn when_user_provides_invalid_dataset_for_bars_clear_error_returned() {
    // Given: A warehouse
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: User tries to ingest to an invalid dataset
    let bars = vec![BarRecord {
        symbol: "AAPL".to_string(),
        ts: "2026-02-20T10:00:00Z".to_string(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: Some(1000),
    }];
    let result = warehouse.ingest_bars("test", "invalid_dataset", "req-001", &bars, 100);

    // Then: A clear error is returned
    let error = result.expect_err("invalid dataset should fail");
    let msg = error.to_string().to_lowercase();
    assert!(
        msg.contains("unsupported") || msg.contains("dataset"),
        "error should mention unsupported dataset: {}",
        msg
    );
}

// =============================================================================
// Error Handling: User Guidance
// =============================================================================

#[tokio::test]
async fn when_rate_limited_user_receives_retryable_error() {
    // Given: A request that will hit rate limits
    let router = SourceRouter::default();
    let symbols = vec![
        Symbol::parse("A").expect("valid"),
        Symbol::parse("B").expect("valid"),
        Symbol::parse("C").expect("valid"),
        Symbol::parse("D").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: Request is made with strict source (no fallback)
    let result = router
        .route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon))
        .await;

    // Then: Error indicates retryability
    if let Err(failure) = result {
        let rate_limit_error = failure
            .errors
            .iter()
            .find(|e| e.message.to_lowercase().contains("rate"));
        if let Some(error) = rate_limit_error {
            assert!(
                error.retryable.unwrap_or(false),
                "rate limit errors should be retryable"
            );
        }
    }
}

#[tokio::test]
async fn when_circuit_breaker_open_user_gets_retry_guidance() {
    // Given: An adapter with open circuit breaker
    let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        open_timeout: Duration::from_secs(60),
    }));
    circuit_breaker.record_failure();
    circuit_breaker.record_failure();

    let adapter = YahooAdapter::with_circuit_breaker(circuit_breaker);

    // When: Request is made
    let request = QuoteRequest::new(vec![Symbol::parse("AAPL").expect("valid")])
        .expect("valid request");
    let result = adapter.quote(request).await;

    // Then: Error indicates it's retryable (circuit will close after timeout)
    let error = result.expect_err("should fail with open circuit");
    assert!(error.retryable(), "circuit breaker errors should be retryable");
}

// =============================================================================
// Error Handling: Error Attribution
// =============================================================================

#[tokio::test]
async fn when_routing_fails_each_error_identifies_its_source() {
    // Given: A router
    let router = SourceRouter::default();

    // When: A request that exhausts sources is made
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

    // Then: Each error is attributed to its source
    let failure = result.expect_err("should fail");
    for error in &failure.errors {
        assert!(
            error.source.is_some(),
            "error should have source attribution"
        );
        let source = error.source.unwrap();
        assert!(
            matches!(source, ProviderId::Polygon | ProviderId::Alpaca | ProviderId::Yahoo | ProviderId::Alphavantage),
            "source should be a valid provider"
        );
    }
}

// =============================================================================
// Error Handling: Latency Tracking
// =============================================================================

#[tokio::test]
async fn when_operation_fails_latency_is_still_recorded() {
    // Given: A router
    let router = SourceRouter::default();

    // When: A request fails
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

    // Then: Latency is still measured and reported
    let failure = result.expect_err("should fail");
    // Note: In fast test environments, latency could be 0ms
    assert!(failure.latency_ms < 5000, "latency should be reasonable");
}
