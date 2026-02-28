use rand::{rngs::StdRng, Rng, SeedableRng};

use super::{Action, Agent, TradingState};

#[derive(Clone, Debug)]
pub struct RandomAgent {
    rng: StdRng,
}

impl RandomAgent {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn choose_action(&mut self, actions: &[Action]) -> Action {
        if actions.is_empty() {
            return Action::Hold;
        }

        let idx = self.rng.gen_range(0..actions.len());
        actions[idx]
    }
}

impl Agent for RandomAgent {
    fn choose_action(&mut self, _state: &TradingState, actions: &[Action]) -> Action {
        self.choose_action(actions)
    }
}
