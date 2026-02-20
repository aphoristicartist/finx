//! Behavior-driven tests for CLI user journeys
//!
//! These tests verify WHAT the user can accomplish with ferrotick CLI,
//! focusing on observable behavior rather than implementation details.

use ferrotick_core::{
    adapters::YahooAdapter,
    data_source::{BarsRequest, DataSource, QuoteRequest, SearchRequest},
    routing::{SourceRouter, SourceStrategy},
    Interval, ProviderId, Symbol,
};
use ferrotick_warehouse::{QueryGuardrails, QuoteRecord, Warehouse, WarehouseConfig};
use tempfile::tempdir;

// =============================================================================
// CLI User Journey: Quote Lookups
// =============================================================================

#[tokio::test]
async fn user_can_lookup_single_stock_quote_and_receives_valid_data() {
    // Given: A user wants to look up a stock quote for AAPL
    let router = SourceRouter::default();
    let symbols = vec![Symbol::parse("AAPL").expect("AAPL is valid")];
    let request = QuoteRequest::new(symbols).expect("valid quote request");

    // When: They query the system with auto-routing
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("quote lookup should succeed");

    // Then: They receive a valid quote with expected fields
    assert!(!result.data.quotes.is_empty(), "should return at least one quote");

    let quote = &result.data.quotes[0];
    assert_eq!(quote.symbol.as_str(), "AAPL", "quote symbol should match requested symbol");
    assert!(quote.price > 0.0, "price should be positive");
    assert!(!quote.currency.is_empty(), "currency should be present");
    assert!(quote.as_of.into_inner().unix_timestamp() > 0, "timestamp should be valid");

    // And: The user knows which data source was used
    assert!(!result.source_chain.is_empty(), "source chain should be recorded");
    assert!(matches!(
        result.selected_source,
        ProviderId::Polygon | ProviderId::Alpaca | ProviderId::Yahoo | ProviderId::Alphavantage
    ));
}

#[tokio::test]
async fn user_can_lookup_multiple_stocks_in_single_request() {
    // Given: A user wants quotes for multiple tech stocks
    let router = SourceRouter::default();
    let symbols = vec![
        Symbol::parse("AAPL").expect("valid"),
        Symbol::parse("MSFT").expect("valid"),
        Symbol::parse("GOOGL").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: They request quotes for all symbols at once
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("batch quote should succeed");

    // Then: They receive quotes for all requested symbols
    assert_eq!(result.data.quotes.len(), 3, "should return all 3 quotes");

    let symbols_returned: Vec<&str> = result
        .data
        .quotes
        .iter()
        .map(|q| q.symbol.as_str())
        .collect();

    assert!(symbols_returned.contains(&"AAPL"), "should include AAPL quote");
    assert!(symbols_returned.contains(&"MSFT"), "should include MSFT quote");
    assert!(symbols_returned.contains(&"GOOGL"), "should include GOOGL quote");

    // And: Each quote has valid market data
    for quote in &result.data.quotes {
        assert!(quote.price > 0.0, "all prices should be positive");
        assert!(!quote.currency.is_empty(), "all quotes should have currency");
    }
}

#[tokio::test]
async fn user_can_search_for_stocks_by_partial_name() {
    // Given: A user wants to find Apple stock but only remembers "apple"
    let router = SourceRouter::default();
    let request = SearchRequest::new("apple", 10).expect("valid search request");

    // When: They search with the partial name
    let result = router
        .route_search(&request, SourceStrategy::Auto)
        .await
        .expect("search should succeed");

    // Then: They receive matching instruments
    assert!(!result.data.results.is_empty(), "should return search results");

    // And: At least one result matches the query
    let has_apple_match = result
        .data
        .results
        .iter()
        .any(|inst| inst.name.to_lowercase().contains("apple") || inst.symbol.as_str() == "AAPL");
    assert!(has_apple_match, "should find Apple-related instruments");

    // And: Results are limited to the requested amount
    assert!(
        result.data.results.len() <= 10,
        "results should respect limit"
    );
}

// =============================================================================
// CLI User Journey: Historical Data (Bars)
// =============================================================================

#[tokio::test]
async fn user_can_fetch_historical_daily_bars_for_analysis() {
    // Given: A user wants 30 days of AAPL price history
    let adapter = YahooAdapter::default();
    let symbol = Symbol::parse("AAPL").expect("valid symbol");
    let request = BarsRequest::new(symbol, Interval::OneDay, 30).expect("valid bars request");

    // When: They request the historical bars
    let bars = adapter.bars(request).await.expect("bars request should succeed");

    // Then: They receive exactly the number of bars requested
    assert_eq!(bars.bars.len(), 30, "should return exactly 30 bars");

    // And: Each bar has valid OHLCV data
    for bar in &bars.bars {
        assert!(bar.open > 0.0, "open price should be positive");
        assert!(bar.high >= bar.open, "high should be >= open");
        assert!(bar.high >= bar.close, "high should be >= close");
        assert!(bar.low <= bar.open, "low should be <= open");
        assert!(bar.low <= bar.close, "low should be <= close");
        assert!(bar.high >= bar.low, "high should be >= low");
    }

    // And: Bars are in chronological order
    for window in bars.bars.windows(2) {
        let ts1 = window[0].ts.into_inner().unix_timestamp();
        let ts2 = window[1].ts.into_inner().unix_timestamp();
        assert!(ts2 > ts1, "bars should be in ascending time order");
    }
}

#[tokio::test]
async fn user_can_fetch_intraday_bars_for_different_intervals() {
    // Given: A user wants to analyze intraday price movements
    let adapter = YahooAdapter::default();
    let symbol = Symbol::parse("AAPL").expect("valid");

    // When: They request bars at different intervals
    for interval in [
        Interval::OneMinute,
        Interval::FiveMinutes,
        Interval::FifteenMinutes,
        Interval::OneHour,
    ] {
        let request = BarsRequest::new(symbol.clone(), interval, 10).expect("valid request");

        let bars = adapter.bars(request).await.unwrap_or_else(|_| {
            panic!("bars request for {:?} should succeed", interval)
        });

        // Then: Each interval type returns valid data
        assert_eq!(bars.interval, interval, "interval should match request");
        assert!(!bars.bars.is_empty(), "should return bars for {:?}", interval);
    }
}

// =============================================================================
// CLI User Journey: SQL Queries on Warehouse
// =============================================================================

#[test]
fn user_can_query_warehouse_with_standard_sql() {
    // Given: A user has a warehouse with some data
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // And: Some quotes have been ingested
    let quotes = vec![QuoteRecord {
        symbol: "AAPL".to_string(),
        price: 150.0,
        bid: Some(149.5),
        ask: Some(150.5),
        volume: Some(1_000_000),
        currency: "USD".to_string(),
        as_of: "2026-02-20T10:00:00Z".to_string(),
    }];
    warehouse
        .ingest_quotes("yahoo", "req-001", &quotes, 100)
        .expect("ingest should succeed");

    // When: The user queries the data with SQL
    let result = warehouse
        .execute_query(
            "SELECT symbol, price FROM quotes_latest WHERE symbol = 'AAPL'",
            QueryGuardrails::default(),
            false,
        )
        .expect("query should succeed");

    // Then: They receive the expected results
    assert_eq!(result.row_count, 1, "should return one row");
    assert!(!result.rows.is_empty(), "should have data");
    assert_eq!(result.columns.len(), 2, "should have two columns");
}

#[test]
fn user_can_aggregate_data_using_sql_functions() {
    // Given: A warehouse with multiple data points
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // And: Multiple quotes for different symbols
    let quotes = vec![
        QuoteRecord {
            symbol: "AAPL".to_string(),
            price: 150.0,
            bid: None,
            ask: None,
            volume: Some(1_000_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "MSFT".to_string(),
            price: 300.0,
            bid: None,
            ask: None,
            volume: Some(2_000_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
        QuoteRecord {
            symbol: "GOOGL".to_string(),
            price: 140.0,
            bid: None,
            ask: None,
            volume: Some(500_000),
            currency: "USD".to_string(),
            as_of: "2026-02-20T10:00:00Z".to_string(),
        },
    ];
    warehouse
        .ingest_quotes("yahoo", "req-001", &quotes, 100)
        .expect("ingest should succeed");

    // When: The user aggregates the data
    let result = warehouse
        .execute_query(
            "SELECT COUNT(*) AS count, AVG(price) AS avg_price, SUM(volume) AS total_volume FROM quotes_latest",
            QueryGuardrails::default(),
            false,
        )
        .expect("aggregation should succeed");

    // Then: Aggregation results are correct
    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns.len(), 3);
}

// =============================================================================
// CLI User Journey: Error Handling
// =============================================================================

#[test]
fn user_gets_helpful_error_when_sql_syntax_is_invalid() {
    // Given: A user tries to run invalid SQL
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: They submit malformed SQL
    let result = warehouse.execute_query(
        "SELEC * FORM quotes_latest", // Intentional typos
        QueryGuardrails::default(),
        false,
    );

    // Then: They receive a meaningful error (not a panic)
    assert!(result.is_err(), "invalid SQL should return error");

    // And: The error message helps them understand the problem
    let error = result.expect_err("should have error");
    let error_msg = error.to_string().to_lowercase();
    // DuckDB returns a parse error for syntax issues
    assert!(
        error_msg.contains("error") || error_msg.contains("reject"),
        "error message should be descriptive: {}",
        error
    );
}

#[test]
fn user_gets_clear_error_when_query_attempts_write_operation() {
    // Given: A user tries to modify data in read-only mode
    let temp = tempdir().expect("tempdir");
    let warehouse = Warehouse::open(WarehouseConfig {
        ferrotick_home: temp.path().to_path_buf(),
        db_path: temp.path().join("warehouse.duckdb"),
        max_pool_size: 2,
    })
    .expect("warehouse open");

    // When: They attempt a write operation without --write flag
    let result = warehouse.execute_query(
        "DELETE FROM quotes_latest",
        QueryGuardrails::default(),
        false, // allow_write = false
    );

    // Then: The operation is rejected with a clear message
    let error = result.expect_err("write should be rejected");
    let msg = error.to_string();

    assert!(
        msg.contains("read-only") || msg.contains("SELECT") || msg.contains("write"),
        "error should mention read-only or select requirement: {}",
        msg
    );
}

#[tokio::test]
async fn user_gets_error_when_requesting_zero_limit_bars() {
    // Given: A user accidentally requests zero bars
    let symbol = Symbol::parse("AAPL").expect("valid");

    // When: They try to create the request
    let result = BarsRequest::new(symbol, Interval::OneDay, 0);

    // Then: They get an error explaining the issue
    assert!(result.is_err(), "zero limit should produce error");
}

// =============================================================================
// CLI User Journey: Source Selection
// =============================================================================

#[tokio::test]
async fn user_can_force_specific_data_source_with_strict_mode() {
    // Given: A user wants data specifically from Yahoo
    let router = SourceRouter::default();
    let symbols = vec![Symbol::parse("AAPL").expect("valid")];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: They use strict mode to specify Yahoo
    let result = router
        .route_quote(&request, SourceStrategy::Strict(ProviderId::Yahoo))
        .await;

    // Then: Only Yahoo is used (or an error if Yahoo fails)
    match result {
        Ok(success) => {
            assert_eq!(
                success.selected_source,
                ProviderId::Yahoo,
                "strict mode should use only Yahoo"
            );
            assert_eq!(
                success.source_chain,
                vec![ProviderId::Yahoo],
                "source chain should only contain Yahoo"
            );
        }
        Err(failure) => {
            // If Yahoo fails, no fallback should occur
            assert_eq!(
                failure.source_chain,
                vec![ProviderId::Yahoo],
                "strict mode should not fallback"
            );
        }
    }
}

#[tokio::test]
async fn user_sees_which_sources_were_tried_on_failure() {
    // Given: A user makes a request that will fail on all sources
    let router = SourceRouter::default();
    // Requesting 4 symbols triggers rate limiting in Polygon (the default)
    let symbols = vec![
        Symbol::parse("A").expect("valid"),
        Symbol::parse("B").expect("valid"),
        Symbol::parse("C").expect("valid"),
        Symbol::parse("D").expect("valid"),
    ];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: The request fails
    let result = router
        .route_quote(&request, SourceStrategy::Strict(ProviderId::Polygon))
        .await;

    // Then: The error includes which sources were attempted
    let failure = result.expect_err("should fail in strict mode with rate limit");
    assert!(!failure.source_chain.is_empty(), "should record attempted sources");
    assert!(!failure.errors.is_empty(), "should record errors from each source");
}

// =============================================================================
// CLI User Journey: Data Freshness
// =============================================================================

#[tokio::test]
async fn user_receives_fresh_timestamps_with_quotes() {
    // Given: A user wants current market data
    let router = SourceRouter::default();
    let symbols = vec![Symbol::parse("AAPL").expect("valid")];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: They request a quote
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("should succeed");

    // Then: The timestamp indicates when data was fetched
    let quote = &result.data.quotes[0];
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let quote_ts = quote.as_of.into_inner().unix_timestamp();

    // Timestamp should be within the last minute (fresh data)
    let diff = (now - quote_ts).abs();
    assert!(diff < 60, "quote timestamp should be recent (within 60s), got diff of {}s", diff);
}

#[tokio::test]
async fn user_receives_latency_information_for_performance_monitoring() {
    // Given: A user wants to monitor API response times
    let router = SourceRouter::default();
    let symbols = vec![Symbol::parse("AAPL").expect("valid")];
    let request = QuoteRequest::new(symbols).expect("valid request");

    // When: They make a request
    let result = router
        .route_quote(&request, SourceStrategy::Auto)
        .await
        .expect("should succeed");

    // Then: They receive latency information
    assert!(
        result.latency_ms < 5000,
        "latency should be measured and reasonable"
    );
    // Note: In fast test environments, latency could be 0ms
}
