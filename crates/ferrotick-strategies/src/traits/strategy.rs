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

pub trait Strategy {
    fn name(&self) -> &str;
    fn on_bar(&mut self, bar: &Bar) -> Option<Signal>;
    fn on_signal(&mut self, signal: &Signal) -> Option<Order>;
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
