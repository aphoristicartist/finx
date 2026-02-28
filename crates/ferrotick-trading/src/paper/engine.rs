use crate::error::TradingError;
use crate::paper::{PaperAccount, Position};
use ferrotick_core::Bar;
use ferrotick_strategies::{Signal, SignalAction, Strategy};
use std::collections::HashMap;

pub struct PaperTradingEngine {
    account: PaperAccount,
    strategy: Box<dyn Strategy>,
    positions: HashMap<String, Position>,
}

impl PaperTradingEngine {
    pub fn new(initial_capital: f64, strategy: Box<dyn Strategy>) -> Self {
        Self {
            account: PaperAccount {
                cash: initial_capital,
                initial_capital,
                portfolio_value: initial_capital,
            },
            strategy,
            positions: HashMap::new(),
        }
    }

    pub async fn on_bar(&mut self, bar: &Bar) -> Result<(), TradingError> {
        // ferrotick_core::Bar does not include a symbol field, so we update tracked
        // positions with the latest observed close.
        for position in self.positions.values_mut() {
            position.current_price = bar.close;
        }

        // Get signal from strategy
        if let Some(signal) = self.strategy.on_bar(bar) {
            self.execute_signal(&signal, bar).await?;
        }

        // Update portfolio value
        self.update_portfolio_value();

        Ok(())
    }

    async fn execute_signal(&mut self, signal: &Signal, bar: &Bar) -> Result<(), TradingError> {
        match signal.action {
            SignalAction::Buy => {
                if self.account.cash > 0.0 {
                    let quantity = self.account.cash / bar.close;
                    self.positions.insert(
                        signal.symbol.clone(),
                        Position {
                            symbol: signal.symbol.clone(),
                            quantity,
                            avg_price: bar.close,
                            current_price: bar.close,
                        },
                    );
                    self.account.cash = 0.0;
                }
            }
            SignalAction::Sell => {
                if let Some(position) = self.positions.remove(&signal.symbol) {
                    self.account.cash += position.quantity * bar.close;
                }
            }
            SignalAction::Hold => {}
        }

        Ok(())
    }

    fn update_portfolio_value(&mut self) {
        let positions_value: f64 = self
            .positions
            .values()
            .map(|p| p.quantity * p.current_price)
            .sum();

        self.account.portfolio_value = self.account.cash + positions_value;
    }

    pub fn get_account(&self) -> &PaperAccount {
        &self.account
    }

    pub fn get_positions(&self) -> &HashMap<String, Position> {
        &self.positions
    }
}
