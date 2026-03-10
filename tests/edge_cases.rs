use std::mem::size_of;
use std::sync::{Arc, Mutex};
use std::thread;

use ferrotick_backtest::{
    BacktestConfig, BacktestEngine, BacktestError, BarEvent, Fill, Order, OrderSide, Portfolio,
    SignalAction, SignalEvent, Strategy as BacktestStrategy,
};
use ferrotick_core::{Bar, Symbol, UtcDateTime};
use ferrotick_strategies::{MovingAverageCrossoverStrategy, Strategy as StrategyTrait};
use uuid::Uuid;

fn make_bar(close: f64, step: i64) -> Bar {
    let ts = UtcDateTime::from_unix_timestamp(step * 86_400).expect("valid ts");
    Bar::new(
        ts,
        close.max(0.0),
        (close + 1.0).max(0.0),
        (close - 1.0).max(0.0),
        close.max(0.0),
        Some(1_000),
        None,
    )
    .expect("valid bar")
}

fn make_fill(symbol: &str, quantity: f64, price: f64) -> Fill {
    Fill {
        order_id: Uuid::new_v4(),
        symbol: Symbol::parse(symbol).expect("valid symbol"),
        side: OrderSide::Buy,
        quantity,
        price,
        gross_value: quantity * price,
        fees: 0.0,
        slippage: 0.0,
        filled_at: UtcDateTime::from_unix_timestamp(0).expect("valid timestamp"),
    }
}

struct HoldStrategy;

impl BacktestStrategy for HoldStrategy {
    fn on_bar(&mut self, bar: &BarEvent, _portfolio: &Portfolio) -> Option<SignalEvent> {
        Some(SignalEvent {
            symbol: bar.symbol.clone(),
            ts: bar.bar.ts,
            action: SignalAction::Hold,
            strength: 0.0,
            reason: String::from("hold"),
        })
    }

    fn create_order(
        &self,
        _signal: &SignalEvent,
        _portfolio: &Portfolio,
        _config: &BacktestConfig,
    ) -> Option<Order> {
        None
    }
}

#[tokio::test]
async fn test_empty_bars_handling() {
    let mut engine = BacktestEngine::new(BacktestConfig::default());
    let mut strategy = HoldStrategy;

    let result = engine.run(&mut strategy, &[]).await;
    assert!(
        matches!(result, Err(BacktestError::NoMarketData)),
        "empty market data should return NoMarketData error"
    );
}

#[test]
fn test_single_bar_handling() {
    let mut strategy = MovingAverageCrossoverStrategy::new("AAPL", 5, 10, 1.0).unwrap();
    let single_bar = make_bar(100.0, 0);

    let signal = strategy.on_bar(&single_bar);
    assert!(signal.is_none(), "single bar should not generate a signal");
}

#[test]
fn test_extreme_prices() {
    let ts = UtcDateTime::from_unix_timestamp(0).expect("valid ts");

    let max_price_bar = Bar::new(ts, f64::MAX, f64::MAX, f64::MAX, f64::MAX, Some(1), None);
    assert!(max_price_bar.is_ok(), "f64::MAX prices should be handled");

    let min_price_bar = Bar::new(ts, f64::MIN, f64::MIN, f64::MIN, f64::MIN, Some(1), None);
    assert!(min_price_bar.is_err(), "f64::MIN prices should be rejected");

    let zero_price_bar = Bar::new(ts, 0.0, 0.0, 0.0, 0.0, Some(1), None);
    assert!(zero_price_bar.is_ok(), "zero prices should be handled");
}

#[test]
fn test_concurrent_access() {
    let initial_cash = 1_000_000.0;
    let workers = 16_usize;
    let portfolio = Arc::new(Mutex::new(Portfolio::new(initial_cash)));

    let mut handles = Vec::with_capacity(workers);
    for idx in 0..workers {
        let portfolio = Arc::clone(&portfolio);
        handles.push(thread::spawn(move || {
            let fill = make_fill(&format!("SYM{}", idx), 1.0, 100.0 + idx as f64);
            let mut guard = portfolio.lock().expect("mutex lock");
            guard.apply_fill(&fill).expect("concurrent write succeeds");
        }));
    }

    for handle in handles {
        handle.join().expect("thread should complete");
    }

    let guard = portfolio.lock().expect("mutex lock");
    let expected_spend: f64 = (0..workers).map(|idx| 100.0 + idx as f64).sum();
    let expected_cash = initial_cash - expected_spend;

    assert_eq!(guard.trade_count(), workers);
    assert!(
        (guard.cash() - expected_cash).abs() < 1e-9,
        "cash mismatch after concurrent writes"
    );
}

#[test]
fn test_memory_usage_large_dataset() {
    let bar_count = 1_000_000_usize;
    let mut bars = Vec::with_capacity(bar_count);

    for i in 0..bar_count {
        bars.push(make_bar(100.0 + (i as f64 % 10.0), i as i64));
    }

    let estimated_bytes = bars.len() * size_of::<Bar>();
    assert_eq!(bars.len(), bar_count);
    assert!(
        estimated_bytes < 1_000_000_000,
        "estimated memory usage {} bytes should stay under 1GB",
        estimated_bytes
    );
}
