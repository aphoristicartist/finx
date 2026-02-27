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
                // Issue 4 fix: Include buy-side fees in the average price calculation
                let new_qty = self.quantity + fill.quantity;
                // Total cost includes both the purchase price and fees for buy orders
                let total_cost = (self.average_price * self.quantity) + (fill.price * fill.quantity) + fill.fees;

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

                // Issue 4 fix: Include both buy and sell fees in realized PnL
                // For sell orders, the fee is already subtracted from proceeds
                // The buy fees were already accounted for in the average_price calculation
                let sell_proceeds = fill.price * fill.quantity;
                let total_fees_on_sell = fill.fees;
                
                self.realized_pnl += (sell_proceeds - total_fees_on_sell) - (self.average_price * fill.quantity);
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
