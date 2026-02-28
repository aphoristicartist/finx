pub mod env;
pub mod qtable;
pub mod random;
pub mod reward;

pub use env::{Action, Environment, Position, StepResult, TradingEnvironment, TradingState};
pub use qtable::{
    ActionKey, BalanceBucket, PositionBucket, PriceChangeBucket, QTableAgent, QTableConfig,
    StateKey,
};
pub use random::RandomAgent;
pub use reward::{RewardCalculator, RewardConfig};

pub trait Agent {
    fn choose_action(&mut self, state: &TradingState, actions: &[Action]) -> Action;
}
