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

        // Validate limit_price and stop_price for price constraints (Issue 2)
        self.validate_order_prices(order)?;

        if !self.is_triggered(order, bar)? {
            return Ok(None);
        }

        let reference_price = self.reference_price(order, bar)?;
        
        // Get the base execution price from slippage (uses close for market orders)
        let base_execution_price = self
            .slippage
            .execution_price(order.side, bar, order.quantity);

        // Apply price constraints based on order type (Issue 2 fix)
        let execution_price = self.apply_price_constraints(order, base_execution_price, bar)?;

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

    /// Validates that limit_price and stop_price are valid (positive, finite)
    fn validate_order_prices(&self, order: &Order) -> BacktestResult<()> {
        if let Some(limit_price) = order.limit_price {
            if !limit_price.is_finite() || limit_price <= 0.0 {
                return Err(BacktestError::InvalidOrder(format!(
                    "limit_price must be positive and finite, got: {}",
                    limit_price
                )));
            }
        }

        if let Some(stop_price) = order.stop_price {
            if !stop_price.is_finite() || stop_price <= 0.0 {
                return Err(BacktestError::InvalidOrder(format!(
                    "stop_price must be positive and finite, got: {}",
                    stop_price
                )));
            }
        }

        Ok(())
    }

    /// Applies price constraints for limit and stop orders
    /// Issue 2 fix: Ensures fill prices respect order constraints
    fn apply_price_constraints(
        &self,
        order: &Order,
        base_execution_price: f64,
        _bar: &Bar,
    ) -> BacktestResult<f64> {
        match order.order_type {
            OrderType::Market => Ok(base_execution_price),
            OrderType::Limit => {
                let limit = order.limit_price.ok_or(BacktestError::MissingLimitPrice)?;
                
                match order.side {
                    crate::portfolio::OrderSide::Buy => {
                        // Buy limit: fill price must be <= limit_price
                        // Use min(limit, execution_price) to ensure we don't pay more than the limit
                        Ok(f64::min(limit, base_execution_price))
                    }
                    crate::portfolio::OrderSide::Sell => {
                        // Sell limit: fill price must be >= limit_price
                        // Use max(limit, execution_price) to ensure we get at least the limit price
                        Ok(f64::max(limit, base_execution_price))
                    }
                }
            }
            OrderType::Stop => {
                let stop = order.stop_price.ok_or(BacktestError::MissingStopPrice)?;
                
                match order.side {
                    crate::portfolio::OrderSide::Buy => {
                        // Buy stop: triggers when price goes above stop
                        // Fill at stop price or worse (higher), but not better
                        Ok(f64::max(stop, base_execution_price))
                    }
                    crate::portfolio::OrderSide::Sell => {
                        // Sell stop: triggers when price goes below stop
                        // Fill at stop price or worse (lower), but not better
                        Ok(f64::min(stop, base_execution_price))
                    }
                }
            }
        }
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
