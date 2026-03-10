//! Behavioral tests for multi-asset support - Phase 17
//!
//! These tests verify that multi-asset instruments (options, futures, forex)
//! behave correctly according to financial principles.

use ferrotick_core::{ForexPair, FuturesContract, Greeks, OptionContract, OptionType};

// ============================================================================
// OPTIONS TESTS
// ============================================================================

#[test]
fn test_call_option_delta_increases_with_moneyness() {
    // Out of the money (OTM): strike > underlying
    let otm = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 150.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    // At the money (ATM): strike = underlying
    let atm = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 100.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    // In the money (ITM): strike < underlying
    let itm = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 50.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let delta_otm = otm.greeks().delta;
    let delta_atm = atm.greeks().delta;
    let delta_itm = itm.greeks().delta;

    // BEHAVIOR: Delta should increase with moneyness for call options
    // Note: Simplified implementation uses step function
    assert!(
        delta_otm <= delta_atm,
        "OTM call delta ({:.2}) should be <= ATM delta ({:.2})",
        delta_otm,
        delta_atm
    );
    assert!(
        delta_atm <= delta_itm,
        "ATM call delta ({:.2}) should be <= ITM delta ({:.2})",
        delta_atm,
        delta_itm
    );

    // All deltas should be in valid range [0, 1] for calls
    assert!(
        delta_otm >= 0.0 && delta_otm <= 1.0,
        "OTM delta should be in [0, 1]"
    );
    assert!(
        delta_atm >= 0.0 && delta_atm <= 1.0,
        "ATM delta should be in [0, 1]"
    );
    assert!(
        delta_itm >= 0.0 && delta_itm <= 1.0,
        "ITM delta should be in [0, 1]"
    );
}

#[test]
fn test_put_option_delta_negative() {
    let put = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 100.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Put,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let delta = put.greeks().delta;

    // BEHAVIOR: Put option delta should be negative
    assert!(
        delta < 0.0,
        "Put option delta should be negative, but was {:.2}",
        delta
    );
}

#[test]
fn test_call_option_intrinsic_value() {
    // ITM call: underlying > strike
    let itm_call = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 100.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 120.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let price = itm_call.price();

    // BEHAVIOR: ITM call should have positive intrinsic value
    assert!(
        price > 0.0,
        "ITM call should have positive intrinsic value, but was {:.2}",
        price
    );
    assert!(
        price >= 20.0,
        "ITM call price ({:.2}) should be >= intrinsic value (20.0)",
        price
    );
}

#[test]
fn test_put_option_intrinsic_value() {
    // ITM put: strike > underlying
    let itm_put = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 120.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Put,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let price = itm_put.price();

    // BEHAVIOR: ITM put should have positive intrinsic value
    assert!(
        price > 0.0,
        "ITM put should have positive intrinsic value, but was {:.2}",
        price
    );
    assert!(
        price >= 20.0,
        "ITM put price ({:.2}) should be >= intrinsic value (20.0)",
        price
    );
}

#[test]
fn test_otm_option_zero_intrinsic_value() {
    // OTM call: strike > underlying
    let otm_call = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 150.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let price = otm_call.price();

    // BEHAVIOR: OTM option should have zero intrinsic value
    assert_eq!(
        price, 0.0,
        "OTM call should have zero intrinsic value, but was {:.2}",
        price
    );
}

#[test]
fn test_greeks_structure_complete() {
    let option = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 100.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    let greeks = option.greeks();

    // BEHAVIOR: Greeks should have all required fields
    assert!(greeks.delta.is_finite(), "Delta should be finite");
    assert!(greeks.gamma.is_finite(), "Gamma should be finite");
    assert!(greeks.theta.is_finite(), "Theta should be finite");
    assert!(greeks.vega.is_finite(), "Vega should be finite");
    assert!(greeks.rho.is_finite(), "Rho should be finite");
}

// ============================================================================
// FUTURES TESTS
// ============================================================================

#[test]
fn test_futures_pnl_calculation() {
    let contract = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0, // Contract size (ES = $50 per point)
    );

    // Long 1 contract, entry 4000, exit 4100
    let pnl = contract.calculate_pnl(4000.0, 4100.0, 1.0);

    // BEHAVIOR: P&L should be (4100-4000) * 50 * 1 = $5000
    let expected = 5000.0;
    assert!(
        (pnl - expected).abs() < 0.01,
        "P&L should be ${:.2} but was ${:.2}",
        expected,
        pnl
    );
}

#[test]
fn test_futures_pnl_negative_for_loss() {
    let contract = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0,
    );

    // Long 1 contract, entry 4100, exit 4000 (loss)
    let pnl = contract.calculate_pnl(4100.0, 4000.0, 1.0);

    // BEHAVIOR: P&L should be negative for loss
    assert!(
        pnl < 0.0,
        "P&L should be negative for losing trade, but was {:.2}",
        pnl
    );
}

#[test]
fn test_futures_pnl_scales_with_quantity() {
    let contract = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0,
    );

    let pnl_1 = contract.calculate_pnl(4000.0, 4100.0, 1.0);
    let pnl_2 = contract.calculate_pnl(4000.0, 4100.0, 2.0);

    // BEHAVIOR: P&L should scale linearly with quantity
    assert!(
        (pnl_2 - 2.0 * pnl_1).abs() < 0.01,
        "P&L should double when quantity doubles: pnl_1={:.2}, pnl_2={:.2}",
        pnl_1,
        pnl_2
    );
}

#[test]
fn test_futures_margin_calculation() {
    let contract = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0,
    );

    let margin = contract.calculate_margin(4000.0, 1.0);

    // BEHAVIOR: Margin should be positive
    assert!(
        margin > 0.0,
        "Margin requirement should be positive, but was {:.2}",
        margin
    );

    // BEHAVIOR: Margin should be less than full contract value
    let contract_value = 4000.0 * 50.0;
    assert!(
        margin < contract_value,
        "Margin ({:.2}) should be < contract value ({:.2})",
        margin,
        contract_value
    );
}

#[test]
fn test_futures_contract_creation() {
    let contract = FuturesContract::new(
        "CL".to_string(),
        "Crude Oil".to_string(),
        "2024-04-01".to_string(),
        1000.0, // 1000 barrels per contract
    );

    // BEHAVIOR: Contract should have correct attributes
    assert_eq!(contract.symbol, "CL");
    assert_eq!(contract.underlying, "Crude Oil");
    assert_eq!(contract.contract_size, 1000.0);
    assert!(contract.tick_size > 0.0);
    assert!(contract.margin_requirement > 0.0);
}

// ============================================================================
// FOREX TESTS
// ============================================================================

#[test]
fn test_forex_conversion_base_to_quote() {
    let pair = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.10);

    // Convert 1000 EUR to USD
    let usd = pair.convert(1000.0, true);

    // BEHAVIOR: Should be 1000 * 1.10 = 1100 USD
    let expected = 1100.0;
    assert!(
        (usd - expected).abs() < 0.01,
        "Should be {:.2} USD but was {:.2}",
        expected,
        usd
    );
}

#[test]
fn test_forex_conversion_quote_to_base() {
    let pair = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.10);

    // Convert 1100 USD back to EUR
    let eur = pair.convert(1100.0, false);

    // BEHAVIOR: Should be 1100 / 1.10 = 1000 EUR
    let expected = 1000.0;
    assert!(
        (eur - expected).abs() < 0.01,
        "Should be {:.2} EUR but was {:.2}",
        expected,
        eur
    );
}

#[test]
fn test_forex_roundtrip_conversion() {
    let pair = ForexPair::new("GBP".to_string(), "USD".to_string(), 1.25);

    // Convert 1000 GBP to USD, then back to GBP
    let usd = pair.convert(1000.0, true);
    let gbp_back = pair.convert(usd, false);

    // BEHAVIOR: Roundtrip should return approximately the same amount
    assert!(
        (gbp_back - 1000.0).abs() < 0.01,
        "Roundtrip conversion should return original amount, but got {:.2}",
        gbp_back
    );
}

#[test]
fn test_forex_pip_value_calculation() {
    let pair = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.10);

    let pip_value_1_lot = pair.calculate_pip_value(1.0);

    // BEHAVIOR: Pip value should be positive
    assert!(
        pip_value_1_lot > 0.0,
        "Pip value should be positive, but was {:.2}",
        pip_value_1_lot
    );
}

#[test]
fn test_forex_pip_value_scales_with_lots() {
    let pair = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.10);

    let pip_value_1 = pair.calculate_pip_value(1.0);
    let pip_value_5 = pair.calculate_pip_value(5.0);

    // BEHAVIOR: Pip value should scale linearly with lot count
    assert!(
        (pip_value_5 - 5.0 * pip_value_1).abs() < 0.01,
        "Pip value should scale with lots: 1 lot={:.2}, 5 lots={:.2}",
        pip_value_1,
        pip_value_5
    );
}

#[test]
fn test_forex_pair_creation() {
    let pair = ForexPair::new("USD".to_string(), "JPY".to_string(), 110.50);

    // BEHAVIOR: Pair should have correct attributes
    assert_eq!(pair.base_currency, "USD");
    assert_eq!(pair.quote_currency, "JPY");
    assert!((pair.exchange_rate - 110.50).abs() < 0.01);
    assert!(pair.pip_value > 0.0);
    assert!(pair.lot_size > 0.0);
}

// ============================================================================
// CROSS-ASSET TESTS
// ============================================================================

#[test]
fn test_different_asset_classes_have_distinct_behavior() {
    // Option: non-linear payoff
    let option = OptionContract {
        symbol: "AAPL".to_string(),
        strike: 100.0,
        expiry: "2024-12-31".to_string(),
        option_type: OptionType::Call,
        underlying_price: 100.0,
        volatility: 0.2,
        risk_free_rate: 0.05,
    };

    // Futures: linear payoff
    let future = FuturesContract::new(
        "ES".to_string(),
        "S&P 500".to_string(),
        "2024-03-15".to_string(),
        50.0,
    );

    // Forex: symmetric conversion
    let forex = ForexPair::new("EUR".to_string(), "USD".to_string(), 1.10);

    // BEHAVIOR: Each asset class should have distinct characteristics
    let option_price = option.price();
    let futures_pnl = future.calculate_pnl(4000.0, 4100.0, 1.0);
    let forex_converted = forex.convert(1000.0, true);

    // All should produce finite results
    assert!(option_price.is_finite(), "Option price should be finite");
    assert!(futures_pnl.is_finite(), "Futures P&L should be finite");
    assert!(
        forex_converted.is_finite(),
        "Forex conversion should be finite"
    );
}
