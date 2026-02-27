# IMPLEMENTATION.md - Detailed Implementation Plan

This document provides a detailed implementation plan for the AI/ML-native ferrotick features, including code structures, API designs, and implementation notes.

---

## Phase 7: Feature Engineering Module

### Crate Structure

```
ferrotick-ml/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── error.rs
    ├── features/
    │   ├── mod.rs
    │   ├── indicators.rs      # Technical indicators using `ta` crate
    │   ├── transforms.rs      # Returns, log-returns, normalization
    │   ├── windows.rs         # Rolling windows, lag features
    │   └── store.rs           # Feature store (DuckDB)
    ├── models/
    │   ├── mod.rs
    │   └── traits.rs          # Model traits
    └── training/
        ├── mod.rs
        └── dataset.rs         # Dataset preparation
```

### Cargo.toml

```toml
[package]
name = "ferrotick-ml"
version = "0.1.0"
edition = "2021"

[dependencies]
ferrotick-core = { path = "../ferrotick-core" }
ferrotick-warehouse = { path = "../ferrotick-warehouse" }

# Technical Analysis
ta = "0.5"

# Numerical Computing
ndarray = { version = "0.15", features = ["rayon"] }
polars = { version = "0.41", features = ["lazy", "parquet", "dtype-datetime"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async
tokio = { version = "1", features = ["full"] }

# Error Handling
thiserror = "2.0"

[dev-dependencies]
tempfile = "3.17"
```

### Core Types

```rust
// src/lib.rs
pub mod error;
pub mod features;
pub mod models;
pub mod training;

pub use error::MlError;
pub use features::{FeatureEngineer, Features, FeatureStore};
pub use models::{Model, ModelRegistry};
pub use training::{Dataset, DatasetBuilder};

/// Result type for ML operations.
pub type MlResult<T> = Result<T, MlError>;
```

```rust
// src/features/indicators.rs
use ta::indicators::*;
use ta::{Next, Reset};
use serde::{Deserialize, Serialize};

/// Feature engineering pipeline for OHLCV data.
pub struct FeatureEngineer {
    config: FeatureConfig,
    indicators: Box<dyn IndicatorSet>,
}

/// Configuration for feature engineering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// RSI period (default: 14)
    pub rsi_period: usize,
    /// MACD fast period (default: 12)
    pub macd_fast: usize,
    /// MACD slow period (default: 26)
    pub macd_slow: usize,
    /// MACD signal period (default: 9)
    pub macd_signal: usize,
    /// Bollinger period (default: 20)
    pub bb_period: usize,
    /// Bollinger std dev (default: 2.0)
    pub bb_std: f64,
    /// ATR period (default: 14)
    pub atr_period: usize,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            rsi_period: 14,
            macd_fast: 12,
            macd_slow: 26,
            macd_signal: 9,
            bb_period: 20,
            bb_std: 2.0,
            atr_period: 14,
        }
    }
}

/// Computed features for a single bar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Features {
    pub ts: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<u64>,
    
    // Technical indicators
    pub rsi: f64,
    pub macd_line: f64,
    pub macd_signal: f64,
    pub macd_histogram: f64,
    pub bb_upper: f64,
    pub bb_middle: f64,
    pub bb_lower: f64,
    pub bb_percent: f64,  // %B indicator
    pub bb_width: f64,    // Bandwidth
    pub atr: f64,
    pub atr_percent: f64, // ATR as % of price
    
    // Derived features
    pub returns: f64,
    pub log_returns: f64,
    pub volatility_20d: f64,
}

impl FeatureEngineer {
    pub fn new(config: FeatureConfig) -> Self {
        Self {
            config,
            indicators: Box::new(StandardIndicators::new(&config)),
        }
    }
    
    /// Extract features from a bar series.
    pub fn extract(&mut self, bars: &[ferrotick_core::Bar]) -> Vec<Features> {
        let mut results = Vec::with_capacity(bars.len());
        let mut prev_close = None;
        
        for bar in bars {
            let indicator_values = self.indicators.compute(bar);
            
            let returns = prev_close
                .map(|pc| (bar.close - pc) / pc)
                .unwrap_or(0.0);
            
            let log_returns = prev_close
                .map(|pc| (bar.close / pc).ln())
                .unwrap_or(0.0);
            
            results.push(Features {
                ts: bar.ts.to_string(),
                open: bar.open,
                high: bar.high,
                low: bar.low,
                close: bar.close,
                volume: bar.volume,
                rsi: indicator_values.rsi,
                macd_line: indicator_values.macd_line,
                macd_signal: indicator_values.macd_signal,
                macd_histogram: indicator_values.macd_histogram,
                bb_upper: indicator_values.bb_upper,
                bb_middle: indicator_values.bb_middle,
                bb_lower: indicator_values.bb_lower,
                bb_percent: (bar.close - indicator_values.bb_lower) 
                    / (indicator_values.bb_upper - indicator_values.bb_lower),
                bb_width: (indicator_values.bb_upper - indicator_values.bb_lower) 
                    / indicator_values.bb_middle,
                atr: indicator_values.atr,
                atr_percent: indicator_values.atr / bar.close,
                returns,
                log_returns,
                volatility_20d: 0.0, // Computed in post-processing
            });
            
            prev_close = Some(bar.close);
        }
        
        // Post-process for rolling calculations
        self.add_rolling_features(&mut results);
        results
    }
    
    fn add_rolling_features(&self, features: &mut [Features]) {
        // Add 20-day rolling volatility
        for i in 20..features.len() {
            let returns: Vec<f64> = features[i-20..i].iter().map(|f| f.returns).collect();
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() 
                / returns.len() as f64;
            features[i].volatility_20d = variance.sqrt() * (252.0_f64).sqrt(); // Annualized
        }
    }
}
```

### Feature Store

```rust
// src/features/store.rs
use ferrotick_warehouse::{Warehouse, WarehouseError};
use crate::{Features, MlError, MlResult};

/// DuckDB-backed feature store for ML features.
pub struct FeatureStore {
    warehouse: Warehouse,
}

impl FeatureStore {
    pub fn new(warehouse: Warehouse) -> Self {
        Self { warehouse }
    }
    
    /// Store computed features.
    pub async fn store(
        &self,
        symbol: &str,
        interval: &str,
        features: &[Features],
    ) -> MlResult<()> {
        // Create features table if not exists
        self.warehouse.execute_query(
            r#"
            CREATE TABLE IF NOT EXISTS features (
                symbol VARCHAR NOT NULL,
                interval VARCHAR NOT NULL,
                ts TIMESTAMP NOT NULL,
                open DOUBLE,
                high DOUBLE,
                low DOUBLE,
                close DOUBLE,
                volume BIGINT,
                rsi DOUBLE,
                macd_line DOUBLE,
                macd_signal DOUBLE,
                macd_histogram DOUBLE,
                bb_upper DOUBLE,
                bb_middle DOUBLE,
                bb_lower DOUBLE,
                bb_percent DOUBLE,
                bb_width DOUBLE,
                atr DOUBLE,
                atr_percent DOUBLE,
                returns DOUBLE,
                log_returns DOUBLE,
                volatility_20d DOUBLE,
                PRIMARY KEY (symbol, interval, ts)
            )
            "#,
            Default::default(),
            true, // allow_write
        ).map_err(|e| MlError::StoreError(e.to_string()))?;
        
        // Insert features
        for feature in features {
            // Use parameterized query for security
            // ... insertion logic
        }
        
        Ok(())
    }
    
    /// Retrieve features for training/inference.
    pub async fn get(
        &self,
        symbol: &str,
        interval: &str,
        start: &str,
        end: &str,
    ) -> MlResult<Vec<Features>> {
        let query = format!(
            r#"
            SELECT * FROM features 
            WHERE symbol = '{}' 
              AND interval = '{}'
              AND ts >= '{}'
              AND ts <= '{}'
            ORDER BY ts ASC
            "#,
            symbol, interval, start, end
        );
        
        let result = self.warehouse.execute_query(
            &query,
            Default::default(),
            false,
        ).map_err(|e| MlError::StoreError(e.to_string()))?;
        
        // Convert rows to Features
        Ok(self.rows_to_features(&result.rows))
    }
    
    fn rows_to_features(&self, rows: &[Vec<serde_json::Value>]) -> Vec<Features> {
        rows.iter().map(|row| Features {
            ts: row[2].as_str().unwrap_or_default().to_string(),
            open: row[3].as_f64().unwrap_or_default(),
            // ... map all fields
            ..Default::default()
        }).collect()
    }
}
```

---

## Phase 8: Backtesting Engine

### Crate Structure

```
ferrotick-backtest/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── error.rs
    ├── engine/
    │   ├── mod.rs
    │   ├── event_driven.rs    # Event-driven backtest loop
    │   ├── vectorized.rs      # Vectorized backtest (future)
    │   └── executor.rs        # Order execution
    ├── portfolio/
    │   ├── mod.rs
    │   ├── position.rs        # Position tracking
    │   ├── order.rs           # Order types
    │   └── cash.rs            # Cash management
    ├── metrics/
    │   ├── mod.rs
    │   ├── returns.rs         # Return calculations
    │   ├── risk.rs            # Sharpe, Sortino, VaR
    │   └── drawdown.rs        # Drawdown analysis
    └── costs/
        ├── mod.rs
        ├── slippage.rs        # Slippage models
        └── fees.rs            # Commission models
```

### Core Types

```rust
// src/lib.rs
pub mod engine;
pub mod portfolio;
pub mod metrics;
pub mod costs;
pub mod error;

pub use engine::{BacktestEngine, BacktestConfig, BacktestResult};
pub use portfolio::{Portfolio, Position, Order, OrderType};
pub use metrics::{PerformanceMetrics, MetricsReport};
pub use costs::{TransactionCosts, SlippageModel};

/// Result type for backtest operations.
pub type BacktestResult<T> = Result<T, BacktestError>;
```

```rust
// src/engine/event_driven.rs
use tokio::sync::mpsc;
use crate::{Portfolio, Order, BacktestConfig, BacktestResult, BacktestError};

/// Events processed by the backtest engine.
#[derive(Debug, Clone)]
pub enum BacktestEvent {
    /// New bar received
    Bar(BarEvent),
    /// Signal generated by strategy
    Signal(SignalEvent),
    /// Order created
    Order(OrderEvent),
    /// Order filled
    Fill(FillEvent),
    /// Timer event (for scheduled tasks)
    Timer(TimerEvent),
}

#[derive(Debug, Clone)]
pub struct BarEvent {
    pub symbol: String,
    pub ts: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

#[derive(Debug, Clone)]
pub struct SignalEvent {
    pub symbol: String,
    pub ts: String,
    pub action: SignalAction,
    pub strength: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}

/// Event-driven backtesting engine.
pub struct BacktestEngine {
    config: BacktestConfig,
    portfolio: Portfolio,
    order_executor: OrderExecutor,
    event_bus: EventBus,
}

/// Configuration for backtest run.
#[derive(Debug, Clone)]
pub struct BacktestConfig {
    /// Initial capital
    pub initial_capital: f64,
    /// Start date (ISO 8601)
    pub start_date: String,
    /// End date (ISO 8601)
    pub end_date: String,
    /// Transaction costs
    pub costs: TransactionCosts,
    /// Slippage model
    pub slippage: SlippageModel,
}

impl BacktestEngine {
    pub fn new(config: BacktestConfig) -> Self {
        Self {
            portfolio: Portfolio::new(config.initial_capital),
            order_executor: OrderExecutor::new(config.slippage.clone()),
            config,
            event_bus: EventBus::new(),
        }
    }
    
    /// Run backtest with strategy.
    pub async fn run<S: Strategy + Send>(
        &mut self,
        strategy: &mut S,
        data: Vec<BarEvent>,
    ) -> BacktestResult<BacktestReport> {
        let mut equity_curve = Vec::new();
        
        for bar in data {
            // Update portfolio value
            self.portfolio.update_price(&bar.symbol, bar.close);
            
            // Generate signals
            if let Some(signal) = strategy.on_bar(&bar) {
                // Create order from signal
                if let Some(order) = strategy.create_order(
                    &signal,
                    &self.portfolio,
                    &self.config,
                ) {
                    // Execute order
                    if let Some(fill) = self.order_executor.execute(
                        &order,
                        &bar,
                        &self.config.costs,
                    ) {
                        // Apply fill to portfolio
                        self.portfolio.apply_fill(&fill);
                    }
                }
            }
            
            // Record equity
            equity_curve.push(EquityPoint {
                ts: bar.ts.clone(),
                equity: self.portfolio.equity(),
                cash: self.portfolio.cash(),
                position_value: self.portfolio.position_value(),
            });
        }
        
        // Generate performance report
        self.generate_report(equity_curve)
    }
    
    fn generate_report(&self, equity_curve: Vec<EquityPoint>) -> BacktestResult<BacktestReport> {
        let metrics = PerformanceMetrics::from_equity_curve(&equity_curve);
        
        Ok(BacktestReport {
            initial_capital: self.config.initial_capital,
            final_equity: equity_curve.last().map(|e| e.equity).unwrap_or_default(),
            total_return: metrics.total_return(),
            annualized_return: metrics.annualized_return(),
            volatility: metrics.volatility(),
            sharpe_ratio: metrics.sharpe_ratio(0.02), // 2% risk-free rate
            sortino_ratio: metrics.sortino_ratio(0.02),
            max_drawdown: metrics.max_drawdown(),
            var_95: metrics.var(0.95),
            cvar_95: metrics.cvar(0.95),
            trades: self.portfolio.trade_count(),
            win_rate: self.portfolio.win_rate(),
            equity_curve,
        })
    }
}

/// Strategy trait for implementing trading strategies.
pub trait Strategy {
    /// Process bar and optionally generate signal.
    fn on_bar(&mut self, bar: &BarEvent) -> Option<SignalEvent>;
    
    /// Create order from signal.
    fn create_order(
        &self,
        signal: &SignalEvent,
        portfolio: &Portfolio,
        config: &BacktestConfig,
    ) -> Option<OrderEvent>;
}
```

### Performance Metrics

```rust
// src/metrics/risk.rs
use crate::BacktestResult;

/// Performance metrics calculator.
pub struct PerformanceMetrics {
    returns: Vec<f64>,
    equity_curve: Vec<f64>,
}

impl PerformanceMetrics {
    pub fn from_equity_curve(equity: &[EquityPoint]) -> Self {
        let equity_values: Vec<f64> = equity.iter().map(|e| e.equity).collect();
        let returns: Vec<f64> = equity_values
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();
        
        Self {
            returns,
            equity_curve: equity_values,
        }
    }
    
    /// Total return over the period.
    pub fn total_return(&self) -> f64 {
        let first = self.equity_curve.first().unwrap_or(&1.0);
        let last = self.equity_curve.last().unwrap_or(&1.0);
        (last - first) / first
    }
    
    /// Annualized return.
    pub fn annualized_return(&self) -> f64 {
        let total = self.total_return();
        let years = self.returns.len() as f64 / 252.0; // Trading days
        (1.0 + total).powf(1.0 / years) - 1.0
    }
    
    /// Annualized volatility.
    pub fn volatility(&self) -> f64 {
        let mean = self.returns.iter().sum::<f64>() / self.returns.len() as f64;
        let variance = self.returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / (self.returns.len() - 1) as f64;
        variance.sqrt() * (252.0_f64).sqrt() // Annualized
    }
    
    /// Sharpe ratio.
    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> f64 {
        let excess_return = self.annualized_return() - risk_free_rate;
        excess_return / self.volatility()
    }
    
    /// Sortino ratio (downside deviation only).
    pub fn sortino_ratio(&self, risk_free_rate: f64) -> f64 {
        let excess_return = self.annualized_return() - risk_free_rate;
        let downside = self.downside_deviation();
        excess_return / downside
    }
    
    fn downside_deviation(&self) -> f64 {
        let mean = self.returns.iter().sum::<f64>() / self.returns.len() as f64;
        let downside_sq: f64 = self.returns
            .iter()
            .filter(|&&r| r < 0.0)
            .map(|&r| (r - mean).powi(2))
            .sum();
        (downside_sq / self.returns.len() as f64).sqrt() * (252.0_f64).sqrt()
    }
    
    /// Maximum drawdown.
    pub fn max_drawdown(&self) -> f64 {
        let mut peak = 0.0;
        let mut max_dd = 0.0;
        
        for &equity in &self.equity_curve {
            peak = peak.max(equity);
            let dd = (peak - equity) / peak;
            max_dd = max_dd.max(dd);
        }
        
        max_dd
    }
    
    /// Value at Risk (parametric).
    pub fn var(&self, confidence: f64) -> f64 {
        let mut sorted = self.returns.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = ((1.0 - confidence) * sorted.len() as f64) as usize;
        sorted[index.min(sorted.len() - 1)]
    }
    
    /// Conditional VaR (Expected Shortfall).
    pub fn cvar(&self, confidence: f64) -> f64 {
        let var = self.var(confidence);
        let tail_returns: Vec<f64> = self.returns
            .iter()
            .filter(|&&r| r <= var)
            .cloned()
            .collect();
        
        if tail_returns.is_empty() {
            return var;
        }
        
        tail_returns.iter().sum::<f64>() / tail_returns.len() as f64
    }
}
```

---

## Phase 9: Strategy Library

### Strategy Trait

```rust
// ferrotick-strategies/src/lib.rs
pub mod library;
pub mod signals;
pub mod spec;

pub use library::*;
pub use signals::{Signal, SignalGenerator};
pub use spec::{StrategySpec, StrategyParser};

/// Core strategy trait.
pub trait Strategy: Send + Sync {
    /// Strategy name.
    fn name(&self) -> &str;
    
    /// Process bar and generate signal.
    fn on_bar(&mut self, bar: &BarEvent) -> Option<Signal>;
    
    /// Create order from signal.
    fn create_order(
        &self,
        signal: &Signal,
        portfolio: &Portfolio,
        config: &BacktestConfig,
    ) -> Option<Order>;
    
    /// Reset strategy state.
    fn reset(&mut self);
}

/// Signal generated by strategy.
#[derive(Debug, Clone)]
pub struct Signal {
    pub symbol: String,
    pub ts: String,
    pub action: SignalAction,
    pub strength: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SignalAction {
    Buy,
    Sell,
    Hold,
}
```

### Strategy Implementation

```rust
// ferrotick-strategies/src/library/mean_reversion.rs
use crate::{Strategy, Signal, SignalAction};
use ta::indicators::RelativeStrengthIndex;
use ta::Next;

/// RSI-based mean reversion strategy.
pub struct RsiMeanReversion {
    rsi: RelativeStrengthIndex,
    oversold: f64,
    overbought: f64,
    position_size: f64,
}

impl RsiMeanReversion {
    pub fn new(period: usize, oversold: f64, overbought: f64, position_size: f64) -> Self {
        Self {
            rsi: RelativeStrengthIndex::new(period).unwrap(),
            oversold,
            overbought,
            position_size,
        }
    }
}

impl Strategy for RsiMeanReversion {
    fn name(&self) -> &str {
        "rsi_mean_reversion"
    }
    
    fn on_bar(&mut self, bar: &BarEvent) -> Option<Signal> {
        let rsi = self.rsi.next(bar.close);
        
        if rsi < self.oversold {
            Some(Signal {
                symbol: bar.symbol.clone(),
                ts: bar.ts.clone(),
                action: SignalAction::Buy,
                strength: (self.oversold - rsi) / self.oversold,
                reason: format!("RSI oversold ({:.2})", rsi),
            })
        } else if rsi > self.overbought {
            Some(Signal {
                symbol: bar.symbol.clone(),
                ts: bar.ts.clone(),
                action: SignalAction::Sell,
                strength: (rsi - self.overbought) / (100.0 - self.overbought),
                reason: format!("RSI overbought ({:.2})", rsi),
            })
        } else {
            None
        }
    }
    
    fn create_order(
        &self,
        signal: &Signal,
        portfolio: &Portfolio,
        config: &BacktestConfig,
    ) -> Option<Order> {
        match signal.action {
            SignalAction::Buy => {
                let quantity = (portfolio.cash() * self.position_size * signal.strength) 
                    / portfolio.current_price(&signal.symbol);
                Some(Order::market_buy(signal.symbol.clone(), quantity))
            }
            SignalAction::Sell => {
                let quantity = portfolio.position(&signal.symbol) * signal.strength;
                Some(Order::market_sell(signal.symbol.clone(), quantity))
            }
            SignalAction::Hold => None,
        }
    }
    
    fn reset(&mut self) {
        self.rsi.reset();
    }
}
```

### Strategy Specification

```rust
// ferrotick-strategies/src/spec/parser.rs
use serde::{Deserialize, Serialize};
use crate::{Strategy, MlError};

/// Strategy specification (YAML/JSON format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySpec {
    pub name: String,
    #[serde(rename = "type")]
    pub strategy_type: StrategyType,
    pub timeframe: String,
    #[serde(default)]
    pub entry_rules: Vec<Rule>,
    #[serde(default)]
    pub exit_rules: Vec<Rule>,
    pub position_sizing: PositionSizing,
    #[serde(default)]
    pub risk_management: RiskManagement,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyType {
    Momentum,
    MeanReversion,
    TrendFollowing,
    Pairs,
    Ml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: Option<String>,
    pub indicator: String,
    pub period: Option<usize>,
    pub operator: String,
    pub value: serde_json::Value,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSizing {
    pub method: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskManagement {
    #[serde(default)]
    pub stop_loss: Option<f64>,
    #[serde(default)]
    pub take_profit: Option<f64>,
    #[serde(default)]
    pub trailing_stop: Option<f64>,
}

/// Parse strategy specification from YAML.
pub struct StrategyParser;

impl StrategyParser {
    pub fn from_yaml(yaml: &str) -> Result<StrategySpec, MlError> {
        serde_yaml::from_str(yaml)
            .map_err(|e| MlError::ParseError(e.to_string()))
    }
    
    pub fn from_json(json: &str) -> Result<StrategySpec, MlError> {
        serde_json::from_str(json)
            .map_err(|e| MlError::ParseError(e.to_string()))
    }
    
    /// Compile spec to executable strategy.
    pub fn compile(spec: &StrategySpec) -> Result<Box<dyn Strategy>, MlError> {
        match spec.strategy_type {
            StrategyType::MeanReversion => {
                // Extract RSI parameters from rules
                let rsi_period = spec.entry_rules
                    .iter()
                    .find(|r| r.indicator == "rsi")
                    .and_then(|r| r.period)
                    .unwrap_or(14);
                
                let oversold = spec.entry_rules
                    .iter()
                    .find(|r| r.operator == "<")
                    .and_then(|r| r.value.as_f64())
                    .unwrap_or(30.0);
                
                let overbought = spec.entry_rules
                    .iter()
                    .find(|r| r.operator == ">")
                    .and_then(|r| r.value.as_f64())
                    .unwrap_or(70.0);
                
                let position_size = spec.position_sizing.value;
                
                Ok(Box::new(RsiMeanReversion::new(
                    rsi_period,
                    oversold,
                    overbought,
                    position_size,
                )))
            }
            StrategyType::Momentum => {
                // Compile momentum strategy
                todo!("Implement momentum strategy compilation")
            }
            _ => Err(MlError::UnsupportedStrategyType(format!("{:?}", spec.strategy_type))),
        }
    }
}
```

---

## Phase 10: ML Model Integration

### Model Trait

```rust
// ferrotick-ml/src/models/traits.rs
use ndarray::{Array1, Array2};
use crate::{MlError, MlResult};

/// ML model trait.
pub trait Model: Send + Sync {
    /// Model name.
    fn name(&self) -> &str;
    
    /// Predict from features.
    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>>;
    
    /// Predict single sample.
    fn predict_one(&self, features: &[f64]) -> MlResult<f64>;
    
    /// Save model to file.
    fn save(&self, path: &std::path::Path) -> MlResult<()>;
    
    /// Load model from file.
    fn load(path: &std::path::Path) -> MlResult<Self> where Self: Sized;
}

/// Model type enumeration.
#[derive(Debug, Clone)]
pub enum ModelType {
    /// LSTM neural network
    Lstm { hidden_size: usize, num_layers: usize },
    /// SVM classifier
    Svm { kernel: String },
    /// Random forest
    RandomForest { trees: usize },
    /// Gradient boosting
    GradientBoosting { trees: usize, learning_rate: f64 },
}

/// Model registry for dynamic model creation.
pub struct ModelRegistry {
    models: std::collections::HashMap<String, Box<dyn Model>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: std::collections::HashMap::new(),
        }
    }
    
    pub fn register(&mut self, name: String, model: Box<dyn Model>) {
        self.models.insert(name, model);
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Model> {
        self.models.get(name).map(|b| b.as_ref())
    }
}
```

### Candle LSTM Implementation

```rust
// ferrotick-ml/src/models/forecasting.rs
use candle_core::{Tensor, Device, DType};
use candle_nn::{LSTM, RNN, VarBuilder};
use crate::{Model, MlError, MlResult};

/// LSTM-based price forecaster.
pub struct LstmForecaster {
    lstm: LSTM,
    device: Device,
    hidden_size: usize,
}

impl LstmForecaster {
    pub fn new(
        input_size: usize,
        hidden_size: usize,
        num_layers: usize,
    ) -> MlResult<Self> {
        let device = Device::Cpu;
        
        // Create LSTM config
        let config = candle_nn::rnn::LSTMConfig {
            layer_idx: 0,
            bidirectional: false,
        };
        
        // Initialize with random weights (for training)
        let lstm = LSTM::new(input_size, hidden_size, num_layers, &device)
            .map_err(|e| MlError::ModelError(e.to_string()))?;
        
        Ok(Self {
            lstm,
            device,
            hidden_size,
        })
    }
    
    /// Forecast next N steps from sequence.
    pub fn forecast(&self, input: &Tensor, steps: usize) -> MlResult<Tensor> {
        let output = self.lstm
            .seq(input)
            .map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        // Extract last hidden state and generate forecasts
        // ... forecasting logic
        
        Ok(output)
    }
}

impl Model for LstmForecaster {
    fn name(&self) -> &str {
        "lstm_forecaster"
    }
    
    fn predict(&self, features: &ndarray::Array2<f64>) -> MlResult<ndarray::Array1<f64>> {
        // Convert ndarray to Candle tensor
        let shape = features.shape();
        let data: Vec<f32> = features.iter().map(|&x| x as f32).collect();
        
        let tensor = Tensor::from_slice(&data, (shape[0], shape[1]), &self.device)
            .map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        let output = self.forecast(&tensor, 1)?;
        
        // Convert back to ndarray
        let data: Vec<f32> = output
            .flatten_all()
            .map_err(|e| MlError::InferenceError(e.to_string()))?
            .to_vec1()
            .map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        Ok(ndarray::Array1::from_vec(data.iter().map(|&x| x as f64).collect()))
    }
    
    fn predict_one(&self, features: &[f64]) -> MlResult<f64> {
        let arr = ndarray::Array2::from_shape_vec((1, features.len()), features.to_vec())?;
        let predictions = self.predict(&arr)?;
        Ok(predictions[0])
    }
    
    fn save(&self, path: &std::path::Path) -> MlResult<()> {
        // Save model weights using safetensors
        todo!("Implement model serialization")
    }
    
    fn load(path: &std::path::Path) -> MlResult<Self> {
        // Load model weights
        todo!("Implement model deserialization")
    }
}
```

### ONNX Inference

```rust
// ferrotick-ml/src/inference/onnx.rs
use ort::{Session, GraphOptimizationLevel, Value};
use ndarray::{Array1, Array2};
use crate::{Model, MlError, MlResult};

/// ONNX Runtime inference engine.
pub struct OnnxEngine {
    session: Session,
    input_name: String,
    output_name: String,
}

impl OnnxEngine {
    pub fn new(model_path: &std::path::Path) -> MlResult<Self> {
        let session = Session::builder()
            .map_err(|e| MlError::LoadError(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::All)
            .map_err(|e| MlError::LoadError(e.to_string()))?
            .commit_from_file(model_path)
            .map_err(|e| MlError::LoadError(e.to_string()))?;
        
        // Get input/output names from model
        let input_name = session
            .inputs
            .first()
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "input".to_string());
        
        let output_name = session
            .outputs
            .first()
            .map(|o| o.name.clone())
            .unwrap_or_else(|| "output".to_string());
        
        Ok(Self {
            session,
            input_name,
            output_name,
        })
    }
}

impl Model for OnnxEngine {
    fn name(&self) -> &str {
        "onnx_model"
    }
    
    fn predict(&self, features: &Array2<f64>) -> MlResult<Array1<f64>> {
        let (rows, cols) = features.dim();
        
        // Convert to f32 array
        let data: Vec<f32> = features.iter().map(|&x| x as f32).collect();
        
        // Create ONNX input tensor
        let input = ndarray::Array3::from_shape_vec((rows, 1, cols), data)
            .map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        let input_value = Value::from_array(
            self.session.allocator(),
            &input.into_dyn(),
        ).map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        // Run inference
        let outputs = self.session
            .run(vec![input_value])
            .map_err(|e| MlError::InferenceError(e.to_string()))?;
        
        // Extract output
        let output = outputs
            .get(&self.output_name)
            .ok_or_else(|| MlError::InferenceError("Output not found".to_string()))?;
        
        let output_array: ndarray::ArrayD<f32> = output
            .try_extract_tensor()
            .map_err(|e| MlError::InferenceError(e.to_string()))?
            .view()
            .to_owned();
        
        // Convert to f64
        let result: Array1<f64> = output_array
            .into_dimensionality::<ndarray::Ix1>()
            .map_err(|e| MlError::InferenceError(e.to_string()))?
            .mapv(|x| x as f64);
        
        Ok(result)
    }
    
    fn predict_one(&self, features: &[f64]) -> MlResult<f64> {
        let arr = Array2::from_shape_vec((1, features.len()), features.to_vec())?;
        let predictions = self.predict(&arr)?;
        Ok(predictions[0])
    }
    
    fn save(&self, _path: &std::path::Path) -> MlResult<()> {
        // ONNX models are already saved
        Ok(())
    }
    
    fn load(path: &std::path::Path) -> MlResult<Self> {
        Self::new(path)
    }
}
```

---

## Quick Start Implementation

### Week 1: Setup and Feature Engineering

```bash
# Day 1-2: Create ferrotick-ml crate
cargo new --lib crates/ferrotick-ml

# Day 3-4: Implement feature engineering
# - Add ta crate dependency
# - Implement RSI, MACD, Bollinger Bands
# - Test with sample data

# Day 5: CLI integration
# - Add `ml features` command to ferrotick-cli
# - Test end-to-end feature extraction
```

### Week 2-3: Backtesting Engine

```bash
# Day 1-3: Core engine
# - Event system
# - Portfolio tracking
# - Order execution

# Day 4-5: Performance metrics
# - Returns calculation
# - Sharpe, Sortino, drawdown

# Week 3: CLI and testing
# - `backtest` command
# - Example strategies
# - Unit and integration tests
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature_extraction() {
        let bars = create_test_bars(100);
        let mut engineer = FeatureEngineer::default();
        let features = engineer.extract(&bars);
        
        assert_eq!(features.len(), 100);
        assert!(features[50].rsi > 0.0 && features[50].rsi < 100.0);
    }
    
    #[test]
    fn test_backtest_engine() {
        let config = BacktestConfig::default();
        let mut engine = BacktestEngine::new(config);
        let strategy = RsiMeanReversion::new(14, 30.0, 70.0, 0.1);
        let data = create_test_bars(252);
        
        let report = tokio_test::block_on(async {
            engine.run(&mut strategy.clone(), data).await
        }).unwrap();
        
        assert!(report.final_equity > 0.0);
        assert!(report.sharpe_ratio.is_finite());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_backtest() {
    // 1. Fetch data
    let bars = fetch_bars("AAPL", "2023-01-01", "2023-12-31").await.unwrap();
    
    // 2. Engineer features
    let mut engineer = FeatureEngineer::default();
    let features = engineer.extract(&bars);
    
    // 3. Run backtest
    let config = BacktestConfig {
        initial_capital: 100000.0,
        ..Default::default()
    };
    let mut engine = BacktestEngine::new(config);
    let strategy = RsiMeanReversion::new(14, 30.0, 70.0, 0.1);
    
    let report = engine.run(&mut strategy.clone(), bars).await.unwrap();
    
    // 4. Validate results
    assert!(report.total_return.is_finite());
    assert!(report.max_drawdown >= 0.0);
    assert!(report.max_drawdown <= 1.0);
}
```

---

## Dependencies Summary

### Phase 7 (Feature Engineering)
```toml
ta = "0.5"
ndarray = "0.15"
polars = { version = "0.41", features = ["lazy", "parquet"] }
```

### Phase 8 (Backtesting)
```toml
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
```

### Phase 10 (ML Integration)
```toml
candle-core = "0.4"
candle-nn = "0.4"
linfa = "0.7"
linfa-svm = "0.7"
ort = "2.0"
```

### Phase 12 (AI Features)
```toml
async-openai = "0.20"
serde_yaml = "0.9"
```
