pub mod bb_squeeze;
pub mod ma_crossover;
pub mod macd_trend;
pub mod rsi_reversion;

use serde::Serialize;

pub use bb_squeeze::BollingerBandSqueezeStrategy;
pub use ma_crossover::MovingAverageCrossoverStrategy;
pub use macd_trend::MacdTrendStrategy;
pub use rsi_reversion::RsiMeanReversionStrategy;

#[derive(Debug, Clone, Serialize)]
pub struct StrategyDescriptor {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn built_in_strategies() -> Vec<StrategyDescriptor> {
    vec![
        StrategyDescriptor {
            name: "ma_crossover",
            description: "Simple moving average crossover (fast/slow)",
        },
        StrategyDescriptor {
            name: "rsi_mean_reversion",
            description: "RSI oversold/overbought mean reversion",
        },
        StrategyDescriptor {
            name: "macd_trend",
            description: "MACD signal-line trend crossover",
        },
        StrategyDescriptor {
            name: "bb_squeeze",
            description: "Bollinger squeeze breakout",
        },
    ]
}
