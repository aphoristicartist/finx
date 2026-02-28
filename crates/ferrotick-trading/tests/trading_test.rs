use ferrotick_trading::*;

#[tokio::test]
async fn test_paper_account() {
    let account = PaperAccount {
        cash: 100_000.0,
        initial_capital: 100_000.0,
        portfolio_value: 100_000.0,
    };

    assert_eq!(account.cash, 100_000.0);
}

#[test]
fn test_alpaca_client_creation() {
    let client = AlpacaClient::new("test_key".to_string(), "test_secret".to_string(), true);

    assert!(client.base_url.contains("paper-api"));
}
