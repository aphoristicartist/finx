use std::collections::HashMap;

use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::{Action, Agent, Position, TradingState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriceChangeBucket {
    Up,
    Flat,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PositionBucket {
    Flat,
    Long,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BalanceBucket {
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateKey {
    pub price_change: PriceChangeBucket,
    pub position: PositionBucket,
    pub balance: BalanceBucket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionKey {
    Hold,
    Buy,
    Sell,
}

impl From<Action> for ActionKey {
    fn from(value: Action) -> Self {
        match value {
            Action::Hold => Self::Hold,
            Action::Buy => Self::Buy,
            Action::Sell => Self::Sell,
        }
    }
}

impl From<Position> for PositionBucket {
    fn from(value: Position) -> Self {
        match value {
            Position::Flat => Self::Flat,
            Position::Long => Self::Long,
            Position::Short => Self::Short,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QTableAgent {
    q_table: HashMap<(StateKey, ActionKey), f64>,
    learning_rate: f64,
    discount_factor: f64,
    epsilon: f64,
    epsilon_decay: f64,
    epsilon_min: f64,
    rng: StdRng,
    initial_balance: Option<f64>,
    last_price: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct QTableConfig {
    pub learning_rate: f64,   // default 0.1
    pub discount_factor: f64, // default 0.99
    pub epsilon: f64,         // default 1.0
    pub epsilon_decay: f64,   // default 0.995
    pub epsilon_min: f64,     // default 0.01
}

impl Default for QTableConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.1,
            discount_factor: 0.99,
            epsilon: 1.0,
            epsilon_decay: 0.995,
            epsilon_min: 0.01,
        }
    }
}

impl QTableAgent {
    pub fn new(config: QTableConfig) -> Self {
        let learning_rate = config.learning_rate.clamp(0.0, 1.0);
        let discount_factor = config.discount_factor.clamp(0.0, 1.0);
        let epsilon_min = config.epsilon_min.clamp(0.0, 1.0);
        let epsilon = config.epsilon.clamp(epsilon_min, 1.0);
        let epsilon_decay = config.epsilon_decay.clamp(0.0, 1.0);

        Self {
            q_table: HashMap::new(),
            learning_rate,
            discount_factor,
            epsilon,
            epsilon_decay,
            epsilon_min,
            rng: StdRng::seed_from_u64(42),
            initial_balance: None,
            last_price: None,
        }
    }

    pub fn epsilon(&self) -> f64 {
        self.epsilon
    }

    pub fn q_table_len(&self) -> usize {
        self.q_table.len()
    }

    pub fn choose_action(&mut self, state: &TradingState) -> Action {
        let initial_balance = *self.initial_balance.get_or_insert(state.balance);
        let state_key = self.state_key(state, self.last_price, initial_balance);
        let actions = Action::all();

        let selected_action = if self.rng.gen::<f64>() < self.epsilon {
            let idx = self.rng.gen_range(0..actions.len());
            actions[idx]
        } else {
            let mut best_action = Action::Hold;
            let mut best_value = f64::NEG_INFINITY;

            for action in actions {
                let q_value = self.q_value(state_key, ActionKey::from(action));
                if q_value > best_value {
                    best_value = q_value;
                    best_action = action;
                }
            }

            best_action
        };

        self.last_price = Some(state.price);
        selected_action
    }

    pub fn update(
        &mut self,
        state: &TradingState,
        action: Action,
        reward: f64,
        next_state: &TradingState,
    ) {
        let initial_balance = *self.initial_balance.get_or_insert(state.balance);
        let state_key = self.state_key(state, self.last_price, initial_balance);
        let next_state_key = self.state_key(next_state, Some(state.price), initial_balance);
        let action_key = ActionKey::from(action);

        let current_q = self.q_value(state_key, action_key);
        let max_next_q = Action::all()
            .into_iter()
            .map(|candidate_action| self.q_value(next_state_key, ActionKey::from(candidate_action)))
            .fold(f64::NEG_INFINITY, f64::max);

        let target_q = reward + self.discount_factor * max_next_q;
        let updated_q = current_q + self.learning_rate * (target_q - current_q);
        self.q_table.insert((state_key, action_key), updated_q);

        self.last_price = Some(next_state.price);
    }

    pub fn decay_epsilon(&mut self) {
        self.epsilon = (self.epsilon * self.epsilon_decay).max(self.epsilon_min);
    }

    fn q_value(&self, state_key: StateKey, action_key: ActionKey) -> f64 {
        *self.q_table.get(&(state_key, action_key)).unwrap_or(&0.0)
    }

    fn state_key(
        &self,
        state: &TradingState,
        reference_price: Option<f64>,
        initial_balance: f64,
    ) -> StateKey {
        let price_change = match reference_price {
            Some(price) if price.abs() > f64::EPSILON => {
                let pct_change = (state.price - price) / price;
                if pct_change > 0.01 {
                    PriceChangeBucket::Up
                } else if pct_change < -0.01 {
                    PriceChangeBucket::Down
                } else {
                    PriceChangeBucket::Flat
                }
            }
            _ => PriceChangeBucket::Flat,
        };

        let position = PositionBucket::from(state.position);
        let balance = if state.balance >= initial_balance {
            BalanceBucket::Above
        } else {
            BalanceBucket::Below
        };

        StateKey {
            price_change,
            position,
            balance,
        }
    }
}

impl Agent for QTableAgent {
    fn choose_action(&mut self, state: &TradingState, _actions: &[Action]) -> Action {
        self.choose_action(state)
    }
}
