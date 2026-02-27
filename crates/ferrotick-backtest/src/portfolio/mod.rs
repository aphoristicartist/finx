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
