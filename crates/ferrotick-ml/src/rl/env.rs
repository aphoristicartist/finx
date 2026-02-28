use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub trait Environment {
    type State: Clone;
    type Action: Clone;

    fn reset(&mut self) -> Self::State;
    fn step(&mut self, action: Self::Action) -> StepResult<Self::State>;
    fn action_space(&self) -> Vec<Self::Action>;
    fn is_terminal(&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct StepResult<State> {
    pub next_state: State,
    pub reward: f64,
    pub done: bool,
    pub info: HashMap<String, f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    Hold,
    Buy,
    Sell,
}

impl Action {
    pub const fn all() -> [Self; 3] {
        [Self::Hold, Self::Buy, Self::Sell]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Position {
    Flat,
    Long,
    Short,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradingState {
    pub price: f64,
    pub position: Position,
    pub balance: f64,
    pub shares_held: f64,
    pub step: usize,
}

#[derive(Clone, Debug)]
pub struct TradingEnvironment {
    prices: Vec<f64>,
    current_step: usize,
    position: Position,
    initial_balance: f64,
    balance: f64,
    shares_held: f64,
    transaction_cost: f64,
}

impl TradingEnvironment {
    pub fn new(prices: Vec<f64>, initial_balance: f64, transaction_cost: f64) -> Self {
        let prices = if prices.is_empty() { vec![1.0] } else { prices };
        Self {
            prices,
            current_step: 0,
            position: Position::Flat,
            initial_balance,
            balance: initial_balance,
            shares_held: 0.0,
            transaction_cost: transaction_cost.max(0.0),
        }
    }

    pub fn current_price(&self) -> f64 {
        self.prices[self.current_step]
    }

    pub fn initial_balance(&self) -> f64 {
        self.initial_balance
    }

    pub fn balance(&self) -> f64 {
        self.balance
    }

    pub fn shares_held(&self) -> f64 {
        self.shares_held
    }

    pub fn portfolio_value(&self) -> f64 {
        self.balance + self.shares_held * self.current_price()
    }

    fn state(&self) -> TradingState {
        TradingState {
            price: self.current_price(),
            position: self.position,
            balance: self.balance,
            shares_held: self.shares_held,
            step: self.current_step,
        }
    }

    fn apply_action(&mut self, action: Action, price: f64) -> (f64, f64) {
        let target_shares = match action {
            Action::Hold => self.shares_held,
            Action::Buy => 1.0,
            Action::Sell => -1.0,
        };

        let trade_size = target_shares - self.shares_held;
        if trade_size.abs() <= f64::EPSILON {
            return (0.0, 0.0);
        }

        let trade_notional = trade_size.abs() * price.abs();
        let transaction_fee = trade_notional * self.transaction_cost;

        // Positive trade_size means buying (spend cash), negative means selling (receive cash).
        self.balance -= trade_size * price;
        self.balance -= transaction_fee;
        self.shares_held = target_shares;
        self.position = if self.shares_held > 0.0 {
            Position::Long
        } else if self.shares_held < 0.0 {
            Position::Short
        } else {
            Position::Flat
        };

        (trade_size, transaction_fee)
    }
}

impl Environment for TradingEnvironment {
    type State = TradingState;
    type Action = Action;

    fn reset(&mut self) -> Self::State {
        self.current_step = 0;
        self.position = Position::Flat;
        self.balance = self.initial_balance;
        self.shares_held = 0.0;
        self.state()
    }

    fn step(&mut self, action: Self::Action) -> StepResult<Self::State> {
        if self.is_terminal() {
            return StepResult {
                next_state: self.state(),
                reward: 0.0,
                done: true,
                info: HashMap::new(),
            };
        }

        let execution_price = self.current_price();
        let previous_value = self.portfolio_value();
        let (trade_size, transaction_fee) = self.apply_action(action, execution_price);

        self.current_step += 1;
        let current_value = self.portfolio_value();
        let reward = current_value - previous_value;
        let done = self.is_terminal();

        let mut info = HashMap::new();
        info.insert(String::from("portfolio_value"), current_value);
        info.insert(String::from("balance"), self.balance);
        info.insert(String::from("shares_held"), self.shares_held);
        info.insert(String::from("price"), self.current_price());
        info.insert(String::from("trade_size"), trade_size);
        info.insert(String::from("transaction_fee"), transaction_fee);

        StepResult {
            next_state: self.state(),
            reward,
            done,
            info,
        }
    }

    fn action_space(&self) -> Vec<Self::Action> {
        Action::all().to_vec()
    }

    fn is_terminal(&self) -> bool {
        self.current_step >= self.prices.len() - 1
    }
}
