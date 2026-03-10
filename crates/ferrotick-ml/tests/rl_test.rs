use ferrotick_ml::rl::{
    Action, Environment, Position, QTableAgent, QTableConfig, RandomAgent, RewardCalculator,
    RewardConfig, TradingEnvironment,
};

#[test]
fn test_environment_reset() {
    let mut env = TradingEnvironment::new(vec![100.0, 101.0, 102.0], 100_000.0, 0.001);
    let state = env.reset();

    assert_eq!(state.step, 0);
    assert_eq!(state.price, 100.0);
    assert_eq!(state.position, Position::Flat);
    assert_eq!(state.balance, 100_000.0);
    assert_eq!(state.shares_held, 0.0);
}

#[test]
fn test_environment_step() {
    let mut env = TradingEnvironment::new(vec![100.0, 101.0, 102.0], 100_000.0, 0.001);
    env.reset();

    let buy_result = env.step(Action::Buy);

    assert!(buy_result.reward.is_finite());
    assert_ne!(buy_result.reward, 0.0);
    assert!(!buy_result.done);
    assert_eq!(buy_result.next_state.step, 1);
    assert_eq!(buy_result.next_state.position, Position::Long);
    assert!(buy_result.next_state.shares_held > 0.0);
    assert!(buy_result.next_state.balance < 100_000.0);
    assert!(buy_result.info.contains_key("portfolio_value"));
    assert!(buy_result.info.contains_key("trade_size"));
    assert!(
        buy_result
            .info
            .get("trade_size")
            .expect("trade_size should be present")
            > &0.0
    );

    let sell_result = env.step(Action::Sell);
    assert!(sell_result.reward.is_finite());
    assert_ne!(sell_result.reward, 0.0);
    assert!(sell_result.done);
    assert_eq!(sell_result.next_state.step, 2);
    assert_eq!(sell_result.next_state.position, Position::Short);
    assert!(sell_result.next_state.shares_held < 0.0);
    assert!(
        sell_result
            .info
            .get("trade_size")
            .expect("trade_size should be present")
            < &0.0
    );
}

#[test]
fn test_qtable_agent_creation() {
    let agent = QTableAgent::new(QTableConfig::default());

    assert_eq!(agent.q_table_len(), 0);
    assert!((0.0..=1.0).contains(&agent.epsilon()));
}

#[test]
fn test_random_agent() {
    let mut agent = RandomAgent::new(7);
    let actions = Action::all();

    for _ in 0..32 {
        let action = agent.choose_action(&actions);
        assert!(actions.contains(&action));
    }
}

#[test]
fn test_reward_calculation() {
    let calculator = RewardCalculator::new(RewardConfig::default());
    let reward = calculator.calculate(1_000.0, 1_020.0, Action::Hold);
    assert!(reward > 0.0);

    let sharpe = calculator.sharpe_reward(&[0.01, 0.02, -0.005, 0.015]);
    assert!(sharpe.is_finite());

    let env = TradingEnvironment::new(vec![100.0, 101.0], 1_000.0, 0.001);
    let portfolio_value = RewardCalculator::calculate_portfolio_value(&env);
    assert_eq!(portfolio_value, 1_000.0);
}
