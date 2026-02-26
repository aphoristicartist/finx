use ferrotick_core::{
    data_source::{BarsRequest, Endpoint, HealthState, QuoteRequest, SearchRequest},
    routing::{SourceRouter, SourceStrategy},
    ProviderId, Symbol,
};
use std::sync::Arc;

mod test_helpers;
use test_helpers::{mock_alpaca, mock_alphavantage, mock_polygon, mock_yahoo, mock_router};

#[tokio::test]
async fn test_polygon_adapter_exists() {
    let adapter = Arc::new(mock_polygon());
    assert_eq!(adapter.id(), ProviderId::Polygon);
    assert!(!adapter.capabilities().is_empty());
}

#[tokio::test]
async fn test_yahoo_adapter_exists() {
    let adapter = Arc::new(mock_yahoo());
    assert_eq!(adapter.id(), ProviderId::Yahoo);
    assert!(!adapter.capabilities().is_empty());
}

#[tokio::test]
async fn test_alpaca_adapter_exists() {
    let adapter = Arc::new(mock_alpaca());
    assert_eq!(adapter.id(), ProviderId::Alpaca);
    assert!(!adapter.capabilities().is_empty());
}

#[tokio::test]
async fn test_alphavantage_adapter_exists() {
    let adapter = Arc::new(mock_alphavantage());
    assert_eq!(adapter.id(), ProviderId::AlphaVantage);
    assert!(!adapter.capabilities().is_empty());
}

#[tokio::test]
async fn test_router_initializes_with_all_adapters() {
    let router = mock_router();

    assert_eq!(router.adapters.len(), 4);
    assert!(router.adapters.contains_key(&ProviderId::Polygon));
    assert!(router.adapters.contains_key(&ProviderId::Alpaca));
    assert!(router.adapters.contains_key(&ProviderId::AlphaVantage));
    assert!(router.adapters.contains_key(&ProviderId::Yahoo));
}

#[tokio::test]
async fn test_quote_request_creation() {
    let symbols = vec![
        Symbol::parse("AAPL").expect("valid symbol"),
        Symbol::parse("MSFT").expect("valid symbol"),
    ];

    let request = QuoteRequest::new(symbols).expect("valid request");
    assert_eq!(request.symbols.len(), 2);
}

#[tokio::test]
async fn test_bars_request_creation() {
    let symbol = Symbol::parse("AAPL").expect("valid symbol");
    let request = BarsRequest::new(vec![symbol])
        .with_limit(30)
        .expect("valid request");

    assert_eq!(request.bars.len(), 1);
    assert_eq!(request.bars[0].limit, 30);
}

#[tokio::test]
async fn test_search_request_creation() {
    let request = SearchRequest::new("apple")
        .with_limit(10)
        .expect("valid request");

    assert_eq!(request.query, "apple");
    assert_eq!(request.limit, 10);
}

#[tokio::test]
async fn test_quote_request_with_invalid_symbol() {
    let symbols = vec![Symbol::parse("INVALID_SYMBOL_123").unwrap()];
    let result = QuoteRequest::new(symbols);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_router_has_auto_strategy() {
    let router = mock_router();
    let strategy = SourceStrategy::Auto;

    let chain = router.source_chain_for_strategy(Endpoint::Quote, &strategy).await;
    assert!(!chain.is_empty());
}

#[tokio::test]
async fn test_router_has_strict_strategy() {
    let router = mock_router();
    let strategy = SourceStrategy::Strict(ProviderId::Polygon);

    let chain = router.source_chain_for_strategy(Endpoint::Quote, &strategy).await;
    assert_eq!(chain, vec![ProviderId::Polygon]);
}

#[tokio::test]
async fn test_adapter_capabilities() {
    let polygon = mock_polygon();
    let capabilities = polygon.capabilities();
    assert!(capabilities.contains(&Endpoint::Quote));
    assert!(capabilities.contains(&Endpoint::Bars));
}

#[tokio::test]
async fn test_adapter_health_state() {
    let yahoo = mock_yahoo();
    let health = yahoo.health().await;

    assert_eq!(health.state, HealthState::Healthy);
}

#[tokio::test]
async fn test_alpaca_adapter_has_required_endpoints() {
    let alpaca = mock_alpaca();
    let capabilities = alpaca.capabilities();

    assert!(capabilities.contains(&Endpoint::Quote));
    assert!(capabilities.contains(&Endpoint::Bars));
}

#[tokio::test]
async fn test_alphavantage_adapter_has_fundamentals() {
    let alphavantage = mock_alphavantage();
    let capabilities = alphavantage.capabilities();

    assert!(capabilities.contains(&Endpoint::Fundamentals));
    assert!(capabilities.contains(&Endpoint::Search));
}
