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
    pending_orders: Vec<Order>,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            portfolio: Portfolio::new(config.initial_capital),
            order_executor: OrderExecutor::new(config.slippage.clone()),
            config,
            event_bus: EventBus::new(),
            latest_bars: HashMap::new(),
            pending_orders: Vec::new(),
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

        // Reset state for clean run (Issue 5: Engine state reset)
        self.portfolio.reset();
        self.latest_bars.clear();
        self.pending_orders.clear();

        let mut equity_curve = Vec::with_capacity(data.len());

        for bar_event in data {
            // First, execute any pending orders from previous bars using this bar's open price
            if !self.pending_orders.is_empty() {
                let orders_to_execute = std::mem::take(&mut self.pending_orders);
                for order in orders_to_execute {
                    let bar = self
                        .latest_bars
                        .get(&order.symbol)
                        .ok_or_else(|| BacktestError::MissingBarForSymbol(order.symbol.to_string()))?;

                    if let Some(fill) = self.order_executor.execute(&order, bar, &self.config.costs)? {
                        self.event_bus.publish(BacktestEvent::Fill(fill))?;
                    }
                }
            }

            self.event_bus.publish(BacktestEvent::Bar(bar_event.clone()))?;
            self.process_event_queue(strategy)?;

            equity_curve.push(EquityPoint {
                ts: bar_event.bar.ts,
                equity: self.portfolio.equity(),
                cash: self.portfolio.cash(),
                position_value: self.portfolio.position_value(),
            });
        }

        // Execute any remaining pending orders at the end
        if !self.pending_orders.is_empty() {
            let orders_to_execute = std::mem::take(&mut self.pending_orders);
            for order in orders_to_execute {
                let bar = self
                    .latest_bars
                    .get(&order.symbol)
                    .ok_or_else(|| BacktestError::MissingBarForSymbol(order.symbol.to_string()))?;

                if let Some(fill) = self.order_executor.execute(&order, bar, &self.config.costs)? {
                    self.event_bus.publish(BacktestEvent::Fill(fill))?;
                }
            }
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
                        // Queue the order for execution on the NEXT bar (prevents look-ahead bias - Issue 1)
                        self.pending_orders.push(order);
                    }
                }
                BacktestEvent::Order(order) => {
                    // Orders should now only come from pending_orders execution
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
