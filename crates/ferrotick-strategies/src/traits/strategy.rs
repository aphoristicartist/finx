use ferrotick_core::Bar;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signal {
    pub symbol: String,
    pub ts: String,
    pub action: SignalAction,
    pub strength: f64,
    pub reason: String,
    pub strategy_name: String,
    #[serde(default)]
    pub source_strategy_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderExecutionContext {
    pub portfolio_value: f64,
    pub price: f64,
}

impl OrderExecutionContext {
    pub fn new(portfolio_value: f64, price: f64) -> Self {
        Self {
            portfolio_value,
            price,
        }
    }
}

impl Default for OrderExecutionContext {
    fn default() -> Self {
        Self {
            // NaN defaults make sizing wrappers fall back to strategy-native quantity
            // unless the caller provides explicit execution context.
            portfolio_value: f64::NAN,
            price: f64::NAN,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub symbol: String,
    pub side: OrderSide,
    pub quantity: f64,
    pub reason: String,
}

impl Order {
    pub fn market(
        symbol: impl Into<String>,
        side: OrderSide,
        quantity: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            quantity,
            reason: reason.into(),
        }
    }
}

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn on_bar(&mut self, bar: &Bar) -> Option<Signal>;
    fn on_signal(&mut self, signal: &Signal) -> Option<Order>;
    fn on_signal_with_context(
        &mut self,
        signal: &Signal,
        _ctx: &OrderExecutionContext,
    ) -> Option<Order> {
        self.on_signal(signal)
    }
    fn reset(&mut self);
}

#[derive(Debug, Clone)]
pub struct Portfolio {
    pub cash: f64,
    pub position: f64,
}

impl Portfolio {
    pub fn new(cash: f64) -> Self {
        Self {
            cash,
            position: 0.0,
        }
    }

    pub fn equity(&self) -> f64 {
        self.cash + self.position
    }

    pub fn reset(&mut self) {
        self.position = 0.0;
    }
}

impl Default for Portfolio {
    fn default() -> Self {
        Self::new(100_000.0)
    }
}
