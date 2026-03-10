//! Behavioral tests for portfolio management - Phase 8 Backtesting
//!
//! These tests verify portfolio state changes correctly reflect trading operations.

use ferrotick_backtest::{Fill, OrderSide, Portfolio};
use ferrotick_core::{Symbol, UtcDateTime};
use uuid::Uuid;

fn create_fill(symbol: &str, side: OrderSide, quantity: f64, price: f64) -> Fill {
    Fill {
        order_id: Uuid::new_v4(),
        symbol: Symbol::parse(symbol).expect("valid symbol"),
        side,
        quantity,
        price,
        gross_value: quantity * price,
        fees: 0.0,
        slippage: 0.0,
        filled_at: UtcDateTime::now(),
    }
}

#[test]
fn test_portfolio_tracks_cash_correctly_on_buy() {
    let mut portfolio = Portfolio::new(100_000.0);

    // Buy 100 shares at $50 = $5,000
    let fill = create_fill("AAPL", OrderSide::Buy, 100.0, 50.0);
    portfolio.apply_fill(&fill).expect("fill should succeed");

    // BEHAVIOR: Cash should decrease by exact purchase amount
    assert_eq!(
        portfolio.cash(),
        95_000.0,
        "Cash should be 95,000 after buying 100 shares @ $50"
    );
}

#[test]
fn test_portfolio_tracks_cash_correctly_on_sell() {
    let mut portfolio = Portfolio::new(100_000.0);

    // Buy first
    let buy_fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&buy_fill).expect("buy should succeed");

    // Sell at higher price
    let sell_fill = create_fill("AAPL", OrderSide::Sell, 100.0, 110.0);
    portfolio
        .apply_fill(&sell_fill)
        .expect("sell should succeed");

    // BEHAVIOR: Cash should increase from sale (100 - 10k + 11k = 101k)
    assert_eq!(
        portfolio.cash(),
        101_000.0,
        "Cash should be 101,000 after buying 100 @ $100 and selling @ $110"
    );
}

#[test]
fn test_portfolio_tracks_positions() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");

    // Buy 100 shares
    let fill = create_fill("AAPL", OrderSide::Buy, 100.0, 50.0);
    portfolio.apply_fill(&fill).expect("fill should succeed");

    // BEHAVIOR: Position should be tracked
    let position = portfolio.position(&aapl);
    assert_eq!(
        position, 100.0,
        "Should have 100 shares of AAPL, but got {}",
        position
    );
}

#[test]
fn test_portfolio_reduces_position_on_partial_sell() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");

    // Buy 100 shares
    let buy_fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&buy_fill).expect("buy should succeed");

    // Sell 30 shares
    let sell_fill = create_fill("AAPL", OrderSide::Sell, 30.0, 110.0);
    portfolio
        .apply_fill(&sell_fill)
        .expect("sell should succeed");

    // BEHAVIOR: Position should be reduced to 70
    let position = portfolio.position(&aapl);
    assert_eq!(
        position, 70.0,
        "Should have 70 shares remaining after selling 30, but got {}",
        position
    );
}

#[test]
fn test_portfolio_equity_calculation() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");

    // Buy 100 shares at $100
    let fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&fill).expect("fill should succeed");

    // Update price to $110
    portfolio.update_price(&aapl, 110.0);

    // BEHAVIOR: Equity should reflect unrealized gains
    // Cash: 90,000 + Position value: 11,000 = 101,000
    let equity = portfolio.equity();
    assert_eq!(
        equity, 101_000.0,
        "Equity should be 101,000 (90k cash + 11k position), but got {}",
        equity
    );
}

#[test]
fn test_portfolio_multiple_positions() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");
    let msft = Symbol::parse("MSFT").expect("valid symbol");

    // Buy AAPL
    let aapl_fill = create_fill("AAPL", OrderSide::Buy, 50.0, 150.0);
    portfolio
        .apply_fill(&aapl_fill)
        .expect("AAPL fill should succeed");

    // Buy MSFT
    let msft_fill = create_fill("MSFT", OrderSide::Buy, 30.0, 300.0);
    portfolio
        .apply_fill(&msft_fill)
        .expect("MSFT fill should succeed");

    // BEHAVIOR: Should track both positions independently
    assert_eq!(
        portfolio.position(&aapl),
        50.0,
        "Should have 50 shares of AAPL"
    );
    assert_eq!(
        portfolio.position(&msft),
        30.0,
        "Should have 30 shares of MSFT"
    );

    // Cash should be 100k - (50*150) - (30*300) = 100k - 7.5k - 9k = 83.5k
    assert_eq!(
        portfolio.cash(),
        83_500.0,
        "Cash should be 83,500 after both purchases"
    );
}

#[test]
fn test_portfolio_rejects_oversized_sell() {
    let mut portfolio = Portfolio::new(100_000.0);

    // Buy 100 shares
    let buy_fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&buy_fill).expect("buy should succeed");

    // Try to sell 150 shares (more than we have)
    let sell_fill = create_fill("AAPL", OrderSide::Sell, 150.0, 110.0);
    let result = portfolio.apply_fill(&sell_fill);

    // BEHAVIOR: Should reject the sell
    assert!(
        result.is_err(),
        "Should reject sell of 150 shares when only 100 owned"
    );
}

#[test]
fn test_portfolio_realized_pnl_tracking() {
    let mut portfolio = Portfolio::new(100_000.0);

    // Buy 100 shares at $100
    let buy_fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&buy_fill).expect("buy should succeed");

    // Sell 100 shares at $120 - should realize $2,000 profit
    let sell_fill = create_fill("AAPL", OrderSide::Sell, 100.0, 120.0);
    portfolio
        .apply_fill(&sell_fill)
        .expect("sell should succeed");

    // BEHAVIOR: Realized P&L should be tracked
    let realized_pnl = portfolio.realized_pnl();
    assert!(
        realized_pnl > 0.0,
        "Realized P&L should be positive after profitable trade, got {}",
        realized_pnl
    );
}

#[test]
fn test_portfolio_trade_count() {
    let mut portfolio = Portfolio::new(100_000.0);

    assert_eq!(portfolio.trade_count(), 0, "Should start with 0 trades");

    // Execute 3 trades
    let fill1 = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&fill1).expect("fill1 should succeed");

    let fill2 = create_fill("MSFT", OrderSide::Buy, 50.0, 200.0);
    portfolio.apply_fill(&fill2).expect("fill2 should succeed");

    let fill3 = create_fill("AAPL", OrderSide::Sell, 100.0, 110.0);
    portfolio.apply_fill(&fill3).expect("fill3 should succeed");

    // BEHAVIOR: Trade count should increment for each fill
    assert_eq!(
        portfolio.trade_count(),
        3,
        "Should have 3 trades after 3 fills"
    );
}

#[test]
fn test_portfolio_position_clears_on_full_sell() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");

    // Buy 100 shares
    let buy_fill = create_fill("AAPL", OrderSide::Buy, 100.0, 100.0);
    portfolio.apply_fill(&buy_fill).expect("buy should succeed");

    assert_eq!(
        portfolio.position(&aapl),
        100.0,
        "Should have position after buy"
    );

    // Sell all 100 shares
    let sell_fill = create_fill("AAPL", OrderSide::Sell, 100.0, 110.0);
    portfolio
        .apply_fill(&sell_fill)
        .expect("sell should succeed");

    // BEHAVIOR: Position should be cleared
    assert_eq!(
        portfolio.position(&aapl),
        0.0,
        "Position should be 0 after full sell"
    );
}

#[test]
fn test_portfolio_buy_increases_position_additively() {
    let mut portfolio = Portfolio::new(100_000.0);
    let aapl = Symbol::parse("AAPL").expect("valid symbol");

    // Buy 100 shares
    let fill1 = create_fill("AAPL", OrderSide::Buy, 100.0, 50.0);
    portfolio.apply_fill(&fill1).expect("fill1 should succeed");

    // Buy 50 more shares
    let fill2 = create_fill("AAPL", OrderSide::Buy, 50.0, 60.0);
    portfolio.apply_fill(&fill2).expect("fill2 should succeed");

    // BEHAVIOR: Position should be additive (100 + 50 = 150)
    assert_eq!(
        portfolio.position(&aapl),
        150.0,
        "Position should be 150 after two buys"
    );

    // Cash should be 100k - (100*50) - (50*60) = 100k - 5k - 3k = 92k
    assert_eq!(
        portfolio.cash(),
        92_000.0,
        "Cash should be 92,000 after both buys"
    );
}
