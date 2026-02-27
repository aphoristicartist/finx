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
