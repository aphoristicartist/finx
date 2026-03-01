use ferrotick_core::assets::*;

#[test]
fn test_option_pricing() {
    let option = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 150.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 160.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let price = option.price();
    assert!(price > 0.0);

    let greeks = option.greeks();
    assert!(greeks.delta > 0.0);
}

#[test]
fn test_futures_contract() {
    let contract = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0,
    );

    let pnl = contract.calculate_pnl(4000.0, 4100.0, 1.0);
    assert!(pnl > 0.0);
}

#[test]
fn test_forex_pair() {
    let pair = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.0850);

    let converted = pair.convert(1000.0, true);
    assert!((converted - 1085.0).abs() < 0.01);
}

#[test]
fn test_crypto_pair() {
    let crypto = CryptoPair::new(
        "BTC".to_string(),
        "USD".to_string(),
        "Binance".to_string(),
        45000.0,
    );

    let symbol = crypto.symbol();
    assert_eq!(symbol, "BTC/USD");

    let value = crypto.calculate_value(0.5);
    assert!((value - 22500.0).abs() < 0.01);
}
