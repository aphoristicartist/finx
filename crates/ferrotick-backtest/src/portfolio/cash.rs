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
