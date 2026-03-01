use crate::models::api::StrategyInfo;
use axum::Json;

pub async fn list_strategies() -> Json<Vec<StrategyInfo>> {
    Json(vec![
        StrategyInfo {
            name: "ma_crossover".to_string(),
            description: "Moving Average Crossover".to_string(),
            parameters: vec!["short_period".to_string(), "long_period".to_string()],
        },
        StrategyInfo {
            name: "rsi_reversion".to_string(),
            description: "RSI Mean Reversion".to_string(),
            parameters: vec![
                "period".to_string(),
                "oversold".to_string(),
                "overbought".to_string(),
            ],
        },
    ])
}
