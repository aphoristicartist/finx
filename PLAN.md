# Phase 8 Plan: Backtesting Engine

This plan defines all files required for Phase 8 (`ferrotick-backtest`) with complete code for each file.  
Scope is limited to writing these files only (no implementation in other crates).

## Step-by-Step Implementation

### 1. Create `crates/ferrotick-backtest/Cargo.toml`
- **Exact file path:** `crates/ferrotick-backtest/Cargo.toml`
- **Where to place it:** Create new crate directory at `crates/ferrotick-backtest/`

```toml
[package]
name = "ferrotick-backtest"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Event-driven backtesting engine for ferrotick"

[dependencies]
ferrotick-core = { path = "../ferrotick-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
uuid.workspace = true
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile.workspace = true
```

### 2. Create `crates/ferrotick-backtest/src/lib.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/lib.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/`

```rust
pub mod costs;
pub mod engine;
pub mod error;
pub mod metrics;
pub mod portfolio;

pub use costs::{FeeModel, SlippageModel, TransactionCosts};
pub use engine::{
    BacktestConfig, BacktestEngine, BacktestEvent, BacktestReport, BarEvent, SignalAction,
    SignalEvent, Strategy,
};
pub use error::BacktestError;
pub use metrics::{EquityPoint, MetricsReport, PerformanceMetrics};
pub use portfolio::{
    CashLedger, Fill, Order, OrderSide, OrderStatus, OrderType, Portfolio, Position,
};

/// Result type for backtesting operations.
pub type BacktestResult<T> = Result<T, BacktestError>;
```

### 3. Create `crates/ferrotick-backtest/src/error.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/error.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/`

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BacktestError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("invalid order: {0}")]
    InvalidOrder(String),

    #[error("no market data provided")]
    NoMarketData,

    #[error("no bar available for symbol '{0}'")]
    MissingBarForSymbol(String),

    #[error("limit order is missing limit_price")]
    MissingLimitPrice,

    #[error("stop order is missing stop_price")]
    MissingStopPrice,

    #[error("insufficient cash: required={required:.4}, available={available:.4}")]
    InsufficientCash { required: f64, available: f64 },

    #[error(
        "insufficient position for symbol '{symbol}': requested={requested:.4}, available={available:.4}"
    )]
    InsufficientPosition {
        symbol: String,
        requested: f64,
        available: f64,
    },

    #[error("event bus is closed")]
    EventBusClosed,

    #[error(transparent)]
    Validation(#[from] ferrotick_core::ValidationError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```

### 4. Create `crates/ferrotick-backtest/src/engine/mod.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/engine/mod.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/engine/`

```rust
pub mod event_driven;
pub mod executor;

pub use event_driven::{
    BacktestConfig, BacktestEngine, BacktestEvent, BacktestReport, BarEvent, SignalAction,
    SignalEvent, Strategy,
};
pub use executor::OrderExecutor;
```

### 5. Create `crates/ferrotick-backtest/src/engine/event_driven.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/engine/event_driven.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/engine/`

```rust
use std::collections::HashMap;

use ferrotick_core::{Bar, Symbol, UtcDateTime};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::costs::{SlippageModel, TransactionCosts};
use crate::engine::executor::OrderExecutor;
use crate::metrics::{EquityPoint, MetricsReport};
use crate::portfolio::{Fill, Order, Portfolio};
use crate::{BacktestError, BacktestResult};

/// Events processed by the backtesting engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BacktestEvent {
    Bar(BarEvent),
    Signal(SignalEvent),
    Order(Order),
    Fill(Fill),
    Timer(UtcDateTime),
}

/// Market bar event keyed by symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BarEvent {
    pub symbol: Symbol,
    pub bar: Bar,
}

impl BarEvent {
    pub fn new(symbol: Symbol, bar: Bar) -> Self {
        Self { symbol, bar }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}

/// Strategy output event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalEvent {
    pub symbol: Symbol,
    pub ts: UtcDateTime,
    pub action: SignalAction,
    pub strength: f64,
    pub reason: String,
}

/// Backtest run configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_capital: f64,
    pub start_date: Option<UtcDateTime>,
    pub end_date: Option<UtcDateTime>,
    pub risk_free_rate: f64,
    pub trading_days_per_year: f64,
    pub costs: TransactionCosts,
    pub slippage: SlippageModel,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: 100_000.0,
            start_date: None,
            end_date: None,
            risk_free_rate: 0.02,
            trading_days_per_year: 252.0,
            costs: TransactionCosts::default(),
            slippage: SlippageModel::default(),
        }
    }
}

/// Final report returned by a backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestReport {
    pub initial_capital: f64,
    pub final_equity: f64,
    pub total_return: f64,
    pub annualized_return: f64,
    pub volatility: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub var_95: f64,
    pub cvar_95: f64,
    pub trades: usize,
    pub win_rate: f64,
    pub equity_curve: Vec<EquityPoint>,
}

/// Strategy trait for event-driven backtests.
pub trait Strategy {
    /// Called for each incoming bar.
    fn on_bar(&mut self, bar: &BarEvent, portfolio: &Portfolio) -> Option<SignalEvent>;

    /// Converts a signal into an order (or no-op).
    fn create_order(
        &self,
        signal: &SignalEvent,
        portfolio: &Portfolio,
        config: &BacktestConfig,
    ) -> Option<Order>;
}

/// Event-driven backtesting engine.
pub struct BacktestEngine {
    config: BacktestConfig,
    portfolio: Portfolio,
    order_executor: OrderExecutor,
    event_bus: EventBus,
    latest_bars: HashMap<Symbol, Bar>,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            portfolio: Portfolio::new(config.initial_capital),
            order_executor: OrderExecutor::new(config.slippage.clone()),
            config,
            event_bus: EventBus::new(),
            latest_bars: HashMap::new(),
        }
    }

    pub fn config(&self) -> &BacktestConfig {
        &self.config
    }

    pub fn portfolio(&self) -> &Portfolio {
        &self.portfolio
    }

    pub async fn run<S: Strategy + Send>(
        &mut self,
        strategy: &mut S,
        data: &[BarEvent],
    ) -> BacktestResult<BacktestReport> {
        self.validate_config()?;

        if data.is_empty() {
            return Err(BacktestError::NoMarketData);
        }

        let mut equity_curve = Vec::with_capacity(data.len());

        for bar_event in data {
            self.event_bus.publish(BacktestEvent::Bar(bar_event.clone()))?;
            self.process_event_queue(strategy)?;

            equity_curve.push(EquityPoint {
                ts: bar_event.bar.ts,
                equity: self.portfolio.equity(),
                cash: self.portfolio.cash(),
                position_value: self.portfolio.position_value(),
            });
        }

        self.generate_report(equity_curve)
    }

    fn process_event_queue<S: Strategy>(&mut self, strategy: &mut S) -> BacktestResult<()> {
        while let Some(event) = self.event_bus.try_next()? {
            match event {
                BacktestEvent::Bar(bar_event) => {
                    self.latest_bars
                        .insert(bar_event.symbol.clone(), bar_event.bar.clone());
                    self.portfolio
                        .update_price(&bar_event.symbol, bar_event.bar.close);

                    if let Some(signal) = strategy.on_bar(&bar_event, &self.portfolio) {
                        self.event_bus.publish(BacktestEvent::Signal(signal))?;
                    }
                }
                BacktestEvent::Signal(signal) => {
                    if let Some(order) =
                        strategy.create_order(&signal, &self.portfolio, &self.config)
                    {
                        self.event_bus.publish(BacktestEvent::Order(order))?;
                    }
                }
                BacktestEvent::Order(order) => {
                    let bar = self
                        .latest_bars
                        .get(&order.symbol)
                        .ok_or_else(|| BacktestError::MissingBarForSymbol(order.symbol.to_string()))?;

                    if let Some(fill) = self.order_executor.execute(&order, bar, &self.config.costs)? {
                        self.event_bus.publish(BacktestEvent::Fill(fill))?;
                    }
                }
                BacktestEvent::Fill(fill) => {
                    self.portfolio.apply_fill(&fill)?;
                }
                BacktestEvent::Timer(_) => {}
            }
        }

        Ok(())
    }

    fn generate_report(&self, equity_curve: Vec<EquityPoint>) -> BacktestResult<BacktestReport> {
        let metrics = MetricsReport::from_equity_curve(
            &equity_curve,
            self.config.risk_free_rate,
            self.config.trading_days_per_year,
        );

        Ok(BacktestReport {
            initial_capital: self.config.initial_capital,
            final_equity: equity_curve
                .last()
                .map(|point| point.equity)
                .unwrap_or(self.config.initial_capital),
            total_return: metrics.total_return,
            annualized_return: metrics.annualized_return,
            volatility: metrics.volatility,
            sharpe_ratio: metrics.sharpe_ratio,
            sortino_ratio: metrics.sortino_ratio,
            max_drawdown: metrics.max_drawdown,
            var_95: metrics.var_95,
            cvar_95: metrics.cvar_95,
            trades: self.portfolio.trade_count(),
            win_rate: self.portfolio.win_rate(),
            equity_curve,
        })
    }

    fn validate_config(&self) -> BacktestResult<()> {
        if !self.config.initial_capital.is_finite() || self.config.initial_capital <= 0.0 {
            return Err(BacktestError::InvalidConfig(String::from(
                "initial_capital must be > 0",
            )));
        }

        if !self.config.risk_free_rate.is_finite() {
            return Err(BacktestError::InvalidConfig(String::from(
                "risk_free_rate must be finite",
            )));
        }

        if !self.config.trading_days_per_year.is_finite() || self.config.trading_days_per_year <= 0.0
        {
            return Err(BacktestError::InvalidConfig(String::from(
                "trading_days_per_year must be > 0",
            )));
        }

        Ok(())
    }
}

struct EventBus {
    tx: mpsc::UnboundedSender<BacktestEvent>,
    rx: mpsc::UnboundedReceiver<BacktestEvent>,
}

impl EventBus {
    fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { tx, rx }
    }

    fn publish(&self, event: BacktestEvent) -> BacktestResult<()> {
        self.tx
            .send(event)
            .map_err(|_| BacktestError::EventBusClosed)
    }

    fn try_next(&mut self) -> BacktestResult<Option<BacktestEvent>> {
        match self.rx.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => Err(BacktestError::EventBusClosed),
        }
    }
}
```

### 6. Create `crates/ferrotick-backtest/src/engine/executor.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/engine/executor.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/engine/`

```rust
use ferrotick_core::Bar;

use crate::costs::{SlippageModel, TransactionCosts};
use crate::portfolio::{Fill, Order, OrderType};
use crate::{BacktestError, BacktestResult};

/// Simulates order execution against bar data.
#[derive(Debug, Clone)]
pub struct OrderExecutor {
    slippage: SlippageModel,
}

impl OrderExecutor {
    pub fn new(slippage: SlippageModel) -> Self {
        Self { slippage }
    }

    pub fn slippage_model(&self) -> &SlippageModel {
        &self.slippage
    }

    /// Executes an order on a given bar.
    ///
    /// Returns `Ok(None)` if the order is not triggered (limit/stop conditions not met).
    pub fn execute(
        &self,
        order: &Order,
        bar: &Bar,
        costs: &TransactionCosts,
    ) -> BacktestResult<Option<Fill>> {
        if !order.quantity.is_finite() || order.quantity <= 0.0 {
            return Err(BacktestError::InvalidOrder(String::from(
                "quantity must be finite and > 0",
            )));
        }

        if !self.is_triggered(order, bar)? {
            return Ok(None);
        }

        let reference_price = self.reference_price(order, bar)?;
        let execution_price = self
            .slippage
            .execution_price(order.side, bar, order.quantity);
        let fees = costs.commission(order.quantity, execution_price);
        let slippage_value = (execution_price - reference_price).abs() * order.quantity;

        Ok(Some(Fill::new(
            order.id,
            order.symbol.clone(),
            order.side,
            order.quantity,
            execution_price,
            fees,
            slippage_value,
            bar.ts,
        )))
    }

    fn is_triggered(&self, order: &Order, bar: &Bar) -> BacktestResult<bool> {
        match order.order_type {
            OrderType::Market => Ok(true),
            OrderType::Limit => {
                let limit = order.limit_price.ok_or(BacktestError::MissingLimitPrice)?;
                Ok(match order.side {
                    crate::portfolio::OrderSide::Buy => bar.low <= limit,
                    crate::portfolio::OrderSide::Sell => bar.high >= limit,
                })
            }
            OrderType::Stop => {
                let stop = order.stop_price.ok_or(BacktestError::MissingStopPrice)?;
                Ok(match order.side {
                    crate::portfolio::OrderSide::Buy => bar.high >= stop,
                    crate::portfolio::OrderSide::Sell => bar.low <= stop,
                })
            }
        }
    }

    fn reference_price(&self, order: &Order, bar: &Bar) -> BacktestResult<f64> {
        match order.order_type {
            OrderType::Market => Ok(bar.close),
            OrderType::Limit => order.limit_price.ok_or(BacktestError::MissingLimitPrice),
            OrderType::Stop => order.stop_price.ok_or(BacktestError::MissingStopPrice),
        }
    }
}
```

### 7. Create `crates/ferrotick-backtest/src/portfolio/mod.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/portfolio/mod.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/portfolio/`

```rust
pub mod cash;
pub mod order;
pub mod position;

use std::collections::HashMap;

use ferrotick_core::Symbol;
use serde::{Deserialize, Serialize};

use crate::{BacktestError, BacktestResult};

pub use cash::CashLedger;
pub use order::{Fill, Order, OrderSide, OrderStatus, OrderType};
pub use position::Position;

const POSITION_EPSILON: f64 = 1e-12;

/// Portfolio state used by the backtest engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    cash: CashLedger,
    positions: HashMap<Symbol, Position>,
    last_prices: HashMap<Symbol, f64>,
    trade_count: usize,
    closed_trades: usize,
    winning_trades: usize,
    realized_pnl: f64,
}

impl Portfolio {
    pub fn new(initial_capital: f64) -> Self {
        Self {
            cash: CashLedger::new(initial_capital),
            positions: HashMap::new(),
            last_prices: HashMap::new(),
            trade_count: 0,
            closed_trades: 0,
            winning_trades: 0,
            realized_pnl: 0.0,
        }
    }

    pub fn update_price(&mut self, symbol: &Symbol, price: f64) {
        if !price.is_finite() || price <= 0.0 {
            return;
        }

        self.last_prices.insert(symbol.clone(), price);
        if let Some(position) = self.positions.get_mut(symbol) {
            position.update_price(price);
        }
    }

    pub fn apply_fill(&mut self, fill: &Fill) -> BacktestResult<()> {
        if fill.side == OrderSide::Sell {
            let available = self.position(&fill.symbol);
            if fill.quantity > available + POSITION_EPSILON {
                return Err(BacktestError::InsufficientPosition {
                    symbol: fill.symbol.to_string(),
                    requested: fill.quantity,
                    available,
                });
            }
        }

        self.cash.apply_fill(fill)?;

        let symbol = fill.symbol.clone();
        let (realized_delta, became_flat) = {
            let position = self
                .positions
                .entry(symbol.clone())
                .or_insert_with(|| Position::new(symbol.clone()));
            let realized_before = position.realized_pnl();
            position.apply_fill(fill)?;
            (position.realized_pnl() - realized_before, position.is_flat())
        };

        if became_flat {
            self.positions.remove(&symbol);
        }

        self.last_prices.insert(symbol, fill.price);
        self.trade_count += 1;
        self.realized_pnl += realized_delta;

        if fill.side == OrderSide::Sell {
            self.closed_trades += 1;
            if realized_delta > 0.0 {
                self.winning_trades += 1;
            }
        }

        Ok(())
    }

    pub fn cash(&self) -> f64 {
        self.cash.balance()
    }

    pub fn position_value(&self) -> f64 {
        self.positions.values().map(Position::market_value).sum()
    }

    pub fn equity(&self) -> f64 {
        self.cash() + self.position_value()
    }

    pub fn current_price(&self, symbol: &Symbol) -> f64 {
        self.last_prices
            .get(symbol)
            .copied()
            .or_else(|| self.positions.get(symbol).map(Position::last_price))
            .unwrap_or(0.0)
    }

    pub fn position(&self, symbol: &Symbol) -> f64 {
        self.positions
            .get(symbol)
            .map(Position::quantity)
            .unwrap_or(0.0)
    }

    pub fn trade_count(&self) -> usize {
        self.trade_count
    }

    pub fn win_rate(&self) -> f64 {
        if self.closed_trades == 0 {
            0.0
        } else {
            self.winning_trades as f64 / self.closed_trades as f64
        }
    }

    pub fn realized_pnl(&self) -> f64 {
        self.realized_pnl
    }

    pub fn total_fees(&self) -> f64 {
        self.cash.total_fees()
    }

    pub fn total_slippage(&self) -> f64 {
        self.cash.total_slippage()
    }

    pub fn positions(&self) -> &HashMap<Symbol, Position> {
        &self.positions
    }
}
```

### 8. Create `crates/ferrotick-backtest/src/portfolio/position.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/portfolio/position.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/portfolio/`

```rust
use ferrotick_core::Symbol;
use serde::{Deserialize, Serialize};

use crate::portfolio::{Fill, OrderSide};
use crate::{BacktestError, BacktestResult};

const POSITION_EPSILON: f64 = 1e-12;

/// Position state for a single symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: Symbol,
    quantity: f64,
    average_price: f64,
    last_price: f64,
    realized_pnl: f64,
}

impl Position {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            quantity: 0.0,
            average_price: 0.0,
            last_price: 0.0,
            realized_pnl: 0.0,
        }
    }

    pub fn quantity(&self) -> f64 {
        self.quantity
    }

    pub fn average_price(&self) -> f64 {
        self.average_price
    }

    pub fn last_price(&self) -> f64 {
        self.last_price
    }

    pub fn realized_pnl(&self) -> f64 {
        self.realized_pnl
    }

    pub fn unrealized_pnl(&self) -> f64 {
        if self.quantity.abs() <= POSITION_EPSILON {
            0.0
        } else {
            (self.last_price - self.average_price) * self.quantity
        }
    }

    pub fn total_pnl(&self) -> f64 {
        self.realized_pnl + self.unrealized_pnl()
    }

    pub fn market_value(&self) -> f64 {
        self.quantity * self.last_price
    }

    pub fn is_flat(&self) -> bool {
        self.quantity.abs() <= POSITION_EPSILON
    }

    pub fn update_price(&mut self, price: f64) {
        if price.is_finite() && price > 0.0 {
            self.last_price = price;
        }
    }

    pub fn apply_fill(&mut self, fill: &Fill) -> BacktestResult<()> {
        if !fill.quantity.is_finite() || fill.quantity <= 0.0 {
            return Err(BacktestError::InvalidOrder(String::from(
                "fill quantity must be finite and > 0",
            )));
        }
        if !fill.price.is_finite() || fill.price <= 0.0 {
            return Err(BacktestError::InvalidOrder(String::from(
                "fill price must be finite and > 0",
            )));
        }

        match fill.side {
            OrderSide::Buy => {
                let new_qty = self.quantity + fill.quantity;
                let total_cost = (self.average_price * self.quantity) + (fill.price * fill.quantity);

                self.quantity = new_qty;
                self.average_price = if new_qty > POSITION_EPSILON {
                    total_cost / new_qty
                } else {
                    0.0
                };
            }
            OrderSide::Sell => {
                if fill.quantity > self.quantity + POSITION_EPSILON {
                    return Err(BacktestError::InsufficientPosition {
                        symbol: self.symbol.to_string(),
                        requested: fill.quantity,
                        available: self.quantity.max(0.0),
                    });
                }

                self.realized_pnl += (fill.price - self.average_price) * fill.quantity - fill.fees;
                self.quantity -= fill.quantity;

                if self.quantity <= POSITION_EPSILON {
                    self.quantity = 0.0;
                    self.average_price = 0.0;
                }
            }
        }

        self.last_price = fill.price;
        Ok(())
    }
}
```

### 9. Create `crates/ferrotick-backtest/src/portfolio/order.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/portfolio/order.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/portfolio/`

```rust
use ferrotick_core::{Symbol, UtcDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    New,
    Filled,
    Cancelled,
    Rejected,
}

/// Order representation used by the backtest engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: f64,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub created_at: UtcDateTime,
    pub status: OrderStatus,
}

impl Order {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: Symbol,
        side: OrderSide,
        order_type: OrderType,
        quantity: f64,
        limit_price: Option<f64>,
        stop_price: Option<f64>,
        created_at: UtcDateTime,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            symbol,
            side,
            order_type,
            quantity,
            limit_price,
            stop_price,
            created_at,
            status: OrderStatus::New,
        }
    }

    pub fn market_buy(symbol: Symbol, quantity: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Buy,
            OrderType::Market,
            quantity,
            None,
            None,
            UtcDateTime::now(),
        )
    }

    pub fn market_sell(symbol: Symbol, quantity: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Sell,
            OrderType::Market,
            quantity,
            None,
            None,
            UtcDateTime::now(),
        )
    }

    pub fn limit_buy(symbol: Symbol, quantity: f64, limit_price: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Buy,
            OrderType::Limit,
            quantity,
            Some(limit_price),
            None,
            UtcDateTime::now(),
        )
    }

    pub fn limit_sell(symbol: Symbol, quantity: f64, limit_price: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Sell,
            OrderType::Limit,
            quantity,
            Some(limit_price),
            None,
            UtcDateTime::now(),
        )
    }

    pub fn stop_buy(symbol: Symbol, quantity: f64, stop_price: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Buy,
            OrderType::Stop,
            quantity,
            None,
            Some(stop_price),
            UtcDateTime::now(),
        )
    }

    pub fn stop_sell(symbol: Symbol, quantity: f64, stop_price: f64) -> Self {
        Self::new(
            symbol,
            OrderSide::Sell,
            OrderType::Stop,
            quantity,
            None,
            Some(stop_price),
            UtcDateTime::now(),
        )
    }

    pub fn notional(&self, execution_price: f64) -> f64 {
        self.quantity * execution_price
    }
}

/// Executed fill generated by the order executor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fill {
    pub order_id: Uuid,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub quantity: f64,
    pub price: f64,
    pub gross_value: f64,
    pub fees: f64,
    pub slippage: f64,
    pub filled_at: UtcDateTime,
}

impl Fill {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        order_id: Uuid,
        symbol: Symbol,
        side: OrderSide,
        quantity: f64,
        price: f64,
        fees: f64,
        slippage: f64,
        filled_at: UtcDateTime,
    ) -> Self {
        Self {
            order_id,
            symbol,
            side,
            quantity,
            price,
            gross_value: quantity * price,
            fees,
            slippage,
            filled_at,
        }
    }

    /// Cash flow from the perspective of portfolio cash.
    pub fn net_cash_flow(&self) -> f64 {
        match self.side {
            OrderSide::Buy => -(self.gross_value + self.fees),
            OrderSide::Sell => self.gross_value - self.fees,
        }
    }
}
```

### 10. Create `crates/ferrotick-backtest/src/portfolio/cash.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/portfolio/cash.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/portfolio/`

```rust
use serde::{Deserialize, Serialize};

use crate::portfolio::{Fill, OrderSide};
use crate::{BacktestError, BacktestResult};

const CASH_EPSILON: f64 = 1e-12;

/// Cash ledger for portfolio accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashLedger {
    balance: f64,
    total_fees: f64,
    total_slippage: f64,
}

impl CashLedger {
    pub fn new(initial_balance: f64) -> Self {
        Self {
            balance: initial_balance,
            total_fees: 0.0,
            total_slippage: 0.0,
        }
    }

    pub fn balance(&self) -> f64 {
        self.balance
    }

    pub fn total_fees(&self) -> f64 {
        self.total_fees
    }

    pub fn total_slippage(&self) -> f64 {
        self.total_slippage
    }

    pub fn credit(&mut self, amount: f64) -> BacktestResult<()> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(BacktestError::InvalidOrder(String::from(
                "credit amount must be finite and >= 0",
            )));
        }

        self.balance += amount;
        Ok(())
    }

    pub fn debit(&mut self, amount: f64) -> BacktestResult<()> {
        if !amount.is_finite() || amount < 0.0 {
            return Err(BacktestError::InvalidOrder(String::from(
                "debit amount must be finite and >= 0",
            )));
        }

        if amount > self.balance + CASH_EPSILON {
            return Err(BacktestError::InsufficientCash {
                required: amount,
                available: self.balance,
            });
        }

        self.balance -= amount;
        Ok(())
    }

    pub fn apply_fill(&mut self, fill: &Fill) -> BacktestResult<()> {
        let total_fees = fill.fees.max(0.0);
        let total_slippage = fill.slippage.max(0.0);

        match fill.side {
            OrderSide::Buy => {
                self.debit(fill.gross_value + total_fees)?;
            }
            OrderSide::Sell => {
                self.credit(fill.gross_value - total_fees)?;
            }
        }

        self.total_fees += total_fees;
        self.total_slippage += total_slippage;
        Ok(())
    }
}
```

### 11. Create `crates/ferrotick-backtest/src/metrics/mod.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/metrics/mod.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/metrics/`

```rust
pub mod drawdown;
pub mod returns;
pub mod risk;

use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};

pub use drawdown::{DrawdownPoint, DrawdownSummary};
pub use risk::PerformanceMetrics;

/// Snapshot of portfolio equity at a point in time.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EquityPoint {
    pub ts: UtcDateTime,
    pub equity: f64,
    pub cash: f64,
    pub position_value: f64,
}

/// Flat metrics report for JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub total_return: f64,
    pub annualized_return: f64,
    pub volatility: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub var_95: f64,
    pub cvar_95: f64,
}

impl MetricsReport {
    pub fn from_equity_curve(
        equity_curve: &[EquityPoint],
        risk_free_rate: f64,
        trading_days_per_year: f64,
    ) -> Self {
        let metrics = PerformanceMetrics::from_equity_curve(equity_curve, trading_days_per_year);

        Self {
            total_return: metrics.total_return(),
            annualized_return: metrics.annualized_return(),
            volatility: metrics.volatility(),
            sharpe_ratio: metrics.sharpe_ratio(risk_free_rate),
            sortino_ratio: metrics.sortino_ratio(risk_free_rate),
            max_drawdown: metrics.max_drawdown(),
            var_95: metrics.var(0.95),
            cvar_95: metrics.cvar(0.95),
        }
    }
}
```

### 12. Create `crates/ferrotick-backtest/src/metrics/returns.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/metrics/returns.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/metrics/`

```rust
/// Compute simple returns from a sequence of equity values.
pub fn simple_returns(equity_values: &[f64]) -> Vec<f64> {
    if equity_values.len() < 2 {
        return Vec::new();
    }

    equity_values
        .windows(2)
        .map(|window| {
            let previous = window[0];
            let current = window[1];
            if previous.abs() <= f64::EPSILON {
                0.0
            } else {
                (current - previous) / previous
            }
        })
        .collect()
}

/// Total return over the full series.
pub fn total_return(equity_values: &[f64]) -> f64 {
    if equity_values.len() < 2 {
        return 0.0;
    }

    let first = equity_values[0];
    let last = equity_values[equity_values.len() - 1];

    if first.abs() <= f64::EPSILON {
        0.0
    } else {
        (last - first) / first
    }
}

/// Annualized return using trading-days convention.
pub fn annualized_return(total_return: f64, periods: usize, trading_days_per_year: f64) -> f64 {
    if periods == 0 || trading_days_per_year <= 0.0 {
        return 0.0;
    }

    let years = periods as f64 / trading_days_per_year;
    if years <= 0.0 {
        return 0.0;
    }

    (1.0 + total_return).powf(1.0 / years) - 1.0
}

/// Arithmetic mean.
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

/// Sample standard deviation (N-1 denominator).
pub fn sample_std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }

    let mu = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = value - mu;
            delta * delta
        })
        .sum::<f64>()
        / (values.len() as f64 - 1.0);

    variance.sqrt()
}

/// Annualized volatility from periodic returns.
pub fn annualized_volatility(returns: &[f64], trading_days_per_year: f64) -> f64 {
    if returns.is_empty() || trading_days_per_year <= 0.0 {
        return 0.0;
    }

    sample_std_dev(returns) * trading_days_per_year.sqrt()
}
```

### 13. Create `crates/ferrotick-backtest/src/metrics/risk.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/metrics/risk.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/metrics/`

```rust
use crate::metrics::{drawdown, returns, EquityPoint};

/// Performance and risk metrics derived from an equity curve.
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    returns: Vec<f64>,
    equity_curve: Vec<f64>,
    trading_days_per_year: f64,
}

impl PerformanceMetrics {
    pub fn from_equity_curve(equity_curve: &[EquityPoint], trading_days_per_year: f64) -> Self {
        let equity_values: Vec<f64> = equity_curve.iter().map(|point| point.equity).collect();
        let returns = returns::simple_returns(&equity_values);

        Self {
            returns,
            equity_curve: equity_values,
            trading_days_per_year,
        }
    }

    pub fn total_return(&self) -> f64 {
        returns::total_return(&self.equity_curve)
    }

    pub fn annualized_return(&self) -> f64 {
        returns::annualized_return(
            self.total_return(),
            self.returns.len(),
            self.trading_days_per_year,
        )
    }

    pub fn volatility(&self) -> f64 {
        returns::annualized_volatility(&self.returns, self.trading_days_per_year)
    }

    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> f64 {
        let vol = self.volatility();
        if vol <= f64::EPSILON {
            0.0
        } else {
            (self.annualized_return() - risk_free_rate) / vol
        }
    }

    pub fn sortino_ratio(&self, risk_free_rate: f64) -> f64 {
        let downside = self.downside_deviation();
        if downside <= f64::EPSILON {
            0.0
        } else {
            (self.annualized_return() - risk_free_rate) / downside
        }
    }

    fn downside_deviation(&self) -> f64 {
        if self.returns.is_empty() || self.trading_days_per_year <= 0.0 {
            return 0.0;
        }

        let downside_sq_sum: f64 = self
            .returns
            .iter()
            .map(|r| if *r < 0.0 { r * r } else { 0.0 })
            .sum();

        (downside_sq_sum / self.returns.len() as f64).sqrt() * self.trading_days_per_year.sqrt()
    }

    pub fn max_drawdown(&self) -> f64 {
        drawdown::max_drawdown_from_values(&self.equity_curve)
    }

    /// Historical VaR at confidence level (e.g., 0.95).
    pub fn var(&self, confidence: f64) -> f64 {
        if self.returns.is_empty() {
            return 0.0;
        }

        let confidence = confidence.clamp(0.0, 1.0);
        let mut sorted = self.returns.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));

        let tail_prob = 1.0 - confidence;
        let index = (tail_prob * (sorted.len() as f64 - 1.0)).floor() as usize;
        sorted[index.min(sorted.len() - 1)]
    }

    /// Historical CVaR / Expected Shortfall at confidence level.
    pub fn cvar(&self, confidence: f64) -> f64 {
        if self.returns.is_empty() {
            return 0.0;
        }

        let threshold = self.var(confidence);
        let tail: Vec<f64> = self
            .returns
            .iter()
            .copied()
            .filter(|ret| *ret <= threshold)
            .collect();

        if tail.is_empty() {
            threshold
        } else {
            tail.iter().sum::<f64>() / tail.len() as f64
        }
    }
}
```

### 14. Create `crates/ferrotick-backtest/src/metrics/drawdown.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/metrics/drawdown.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/metrics/`

```rust
use ferrotick_core::UtcDateTime;
use serde::{Deserialize, Serialize};

use crate::metrics::EquityPoint;

/// Drawdown point for each equity observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownPoint {
    pub ts: UtcDateTime,
    pub equity: f64,
    pub peak: f64,
    pub drawdown: f64,
}

/// Drawdown summary for a full equity curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawdownSummary {
    pub max_drawdown: f64,
    pub start: Option<UtcDateTime>,
    pub end: Option<UtcDateTime>,
    pub curve: Vec<DrawdownPoint>,
}

pub fn analyze_drawdown(equity_curve: &[EquityPoint]) -> DrawdownSummary {
    if equity_curve.is_empty() {
        return DrawdownSummary {
            max_drawdown: 0.0,
            start: None,
            end: None,
            curve: Vec::new(),
        };
    }

    let mut curve = Vec::with_capacity(equity_curve.len());
    let mut peak = equity_curve[0].equity;
    let mut peak_ts = equity_curve[0].ts;
    let mut max_drawdown = 0.0;
    let mut max_start = None;
    let mut max_end = None;

    for point in equity_curve {
        if point.equity > peak {
            peak = point.equity;
            peak_ts = point.ts;
        }

        let drawdown = if peak <= f64::EPSILON {
            0.0
        } else {
            ((peak - point.equity) / peak).max(0.0)
        };

        if drawdown > max_drawdown {
            max_drawdown = drawdown;
            max_start = Some(peak_ts);
            max_end = Some(point.ts);
        }

        curve.push(DrawdownPoint {
            ts: point.ts,
            equity: point.equity,
            peak,
            drawdown,
        });
    }

    DrawdownSummary {
        max_drawdown,
        start: max_start,
        end: max_end,
        curve,
    }
}

pub fn max_drawdown_from_values(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut peak = values[0];
    let mut max_drawdown = 0.0;

    for &value in values {
        if value > peak {
            peak = value;
        }

        let drawdown = if peak <= f64::EPSILON {
            0.0
        } else {
            ((peak - value) / peak).max(0.0)
        };

        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    max_drawdown
}
```

### 15. Create `crates/ferrotick-backtest/src/costs/mod.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/costs/mod.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/costs/`

```rust
pub mod fees;
pub mod slippage;

use serde::{Deserialize, Serialize};

pub use fees::FeeModel;
pub use slippage::SlippageModel;

/// Transaction cost configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionCosts {
    pub fee_model: FeeModel,
}

impl Default for TransactionCosts {
    fn default() -> Self {
        Self {
            fee_model: FeeModel::default(),
        }
    }
}

impl TransactionCosts {
    pub fn commission(&self, quantity: f64, price: f64) -> f64 {
        self.fee_model.compute(quantity, price)
    }
}
```

### 16. Create `crates/ferrotick-backtest/src/costs/slippage.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/costs/slippage.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/costs/`

```rust
use ferrotick_core::Bar;
use serde::{Deserialize, Serialize};

use crate::portfolio::OrderSide;

/// Slippage models used during simulated execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum SlippageModel {
    None,
    FixedBps { bps: f64 },
    VolumeShare {
        max_volume_share: f64,
        max_impact_bps: f64,
    },
}

impl Default for SlippageModel {
    fn default() -> Self {
        Self::None
    }
}

impl SlippageModel {
    pub fn execution_price(&self, side: OrderSide, bar: &Bar, quantity: f64) -> f64 {
        let close = bar.close;
        if close <= 0.0 || !close.is_finite() {
            return close;
        }

        let bps = self.effective_bps(bar, quantity);
        let signed_bps = match side {
            OrderSide::Buy => bps,
            OrderSide::Sell => -bps,
        };

        close * (1.0 + signed_bps / 10_000.0)
    }

    pub fn slippage_amount(&self, side: OrderSide, bar: &Bar, quantity: f64) -> f64 {
        let execution = self.execution_price(side, bar, quantity);
        (execution - bar.close).abs() * quantity.abs()
    }

    fn effective_bps(&self, bar: &Bar, quantity: f64) -> f64 {
        match self {
            SlippageModel::None => 0.0,
            SlippageModel::FixedBps { bps } => bps.max(0.0),
            SlippageModel::VolumeShare {
                max_volume_share,
                max_impact_bps,
            } => {
                let volume = bar.volume.unwrap_or(0) as f64;
                if volume <= 0.0 {
                    return max_impact_bps.max(0.0);
                }

                let max_share = max_volume_share.max(1e-9);
                let share = (quantity.abs() / volume).clamp(0.0, max_share);
                let utilization = share / max_share;
                max_impact_bps.max(0.0) * utilization
            }
        }
    }
}
```

### 17. Create `crates/ferrotick-backtest/src/costs/fees.rs`
- **Exact file path:** `crates/ferrotick-backtest/src/costs/fees.rs`
- **Where to place it:** Inside `crates/ferrotick-backtest/src/costs/`

```rust
use serde::{Deserialize, Serialize};

/// Commission/fee models used by the backtest engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum FeeModel {
    None,
    Flat { amount: f64 },
    PerShare { amount: f64 },
    Bps { bps: f64 },
}

impl Default for FeeModel {
    fn default() -> Self {
        Self::None
    }
}

impl FeeModel {
    pub fn compute(&self, quantity: f64, price: f64) -> f64 {
        let qty = quantity.abs();
        let px = price.abs();

        match self {
            FeeModel::None => 0.0,
            FeeModel::Flat { amount } => amount.max(0.0),
            FeeModel::PerShare { amount } => amount.max(0.0) * qty,
            FeeModel::Bps { bps } => (bps.max(0.0) / 10_000.0) * qty * px,
        }
    }
}
```
