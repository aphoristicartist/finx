# ARCHITECTURE.md - AI/ML-Native Ferrotick

## Executive Summary

This document defines the technical architecture for transforming ferrotick into the first **Rust-native, AI/ML-first financial data + backtesting platform**. The design leverages existing strengths (multi-provider data, DuckDB warehouse, agent-friendly output) while adding ML-native feature engineering, backtesting, and AI-powered strategy development.

---

## 1. Current Architecture

### Existing Crates

```
ferrotick/
├── ferrotick-core/      # Domain models, adapters, routing, circuit breaker
├── ferrotick-cli/       # Command-line interface
├── ferrotick-warehouse/ # DuckDB storage, Parquet integration
└── ferrotick-agent/     # AI-agent UX (envelopes, streaming)
```

### Data Flow (Current)

```
Data Sources (Yahoo, Polygon, AlphaVantage, Alpaca)
    ↓
ferrotick-core (DataSource trait, normalized models)
    ↓
ferrotick-warehouse (DuckDB storage)
    ↓
ferrotick-cli (output: JSON, NDJSON, table)
```

### Key Extension Points

| Component | Extension Point | ML/Backtest Integration |
|-----------|----------------|------------------------|
| `DataSource` trait | New adapters | ML feature providers |
| `Bar` model | OHLCV data | Feature engineering input |
| DuckDB warehouse | SQL queries | Feature store, backtest data |
| NDJSON streaming | Agent output | Real-time ML inference |
| `Envelope` metadata | Request tracking | Experiment tracking |

---

## 2. Target Architecture

### New Crate Structure

```
ferrotick/
├── ferrotick-core/           # Existing - domain models, adapters
├── ferrotick-cli/            # Existing - CLI commands
├── ferrotick-warehouse/      # Existing - DuckDB storage
├── ferrotick-agent/          # Existing - AI agent UX
│
├── ferrotick-ml/             # NEW - ML feature engineering & models
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── features/         # Technical indicators, feature extraction
│       │   ├── mod.rs
│       │   ├── indicators.rs # RSI, MACD, Bollinger, ATR, etc.
│       │   ├── transforms.rs # Returns, log-returns, normalization
│       │   └── windows.rs    # Rolling windows, lag features
│       ├── models/           # ML model implementations
│       │   ├── mod.rs
│       │   ├── regression.rs # Price prediction models
│       │   ├── classification.rs # Buy/sell/hold signals
│       │   ├── anomaly.rs    # Market regime detection
│       │   └── forecasting.rs # Time-series forecasting
│       ├── inference/        # Real-time inference engine
│       │   ├── mod.rs
│       │   ├── onnx.rs       # ONNX runtime integration
│       │   └── candle.rs     # Candle model serving
│       └── training/         # Model training pipelines
│           ├── mod.rs
│           ├── dataset.rs    # Dataset preparation
│           ├── trainer.rs    # Training orchestration
│           └── evaluation.rs # Model evaluation metrics
│
├── ferrotick-backtest/       # NEW - Backtesting engine
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── engine/           # Backtesting engine
│       │   ├── mod.rs
│       │   ├── event_driven.rs # Event-driven backtester
│       │   ├── vectorized.rs   # Vectorized backtester
│       │   └── executor.rs     # Order execution simulation
│       ├── portfolio/        # Portfolio management
│       │   ├── mod.rs
│       │   ├── position.rs   # Position tracking
│       │   ├── order.rs      # Order types (market, limit, stop)
│       │   └── rebalance.rs  # Rebalancing logic
│       ├── metrics/          # Performance analytics
│       │   ├── mod.rs
│       │   ├── returns.rs    # Return calculations
│       │   ├── risk.rs       # Sharpe, Sortino, VaR, CVaR
│       │   └── drawdown.rs   # Drawdown analysis
│       ├── costs/            # Transaction cost modeling
│       │   ├── mod.rs
│       │   ├── slippage.rs   # Slippage models
│       │   └── fees.rs       # Commission/fee models
│       └── optimization/     # Strategy optimization
│           ├── mod.rs
│           ├── grid.rs       # Grid search
│           ├── genetic.rs    # Genetic algorithm
│           └── bayesian.rs   # Bayesian optimization
│
├── ferrotick-strategies/     # NEW - Strategy library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── library/          # Pre-built strategies
│       │   ├── mod.rs
│       │   ├── momentum.rs   # Momentum strategies
│       │   ├── mean_reversion.rs # Mean reversion
│       │   ├── trend_following.rs # Trend following
│       │   └── pairs.rs      # Pairs trading
│       ├── signals/          # Signal generation
│       │   ├── mod.rs
│       │   ├── indicator.rs  # Indicator-based signals
│       │   ├── ml.rs         # ML-based signals
│       │   └── composite.rs  # Composite signals
│       ├── execution/        # Order execution logic
│       │   ├── mod.rs
│       │   ├── sizing.rs     # Position sizing
│       │   └── timing.rs     # Execution timing
│       └── spec/             # Strategy specification (YAML/JSON)
│           ├── mod.rs
│           ├── parser.rs     # Parse strategy definitions
│           └── validator.rs  # Validate strategy configs
│
└── ferrotick-ai/             # NEW - AI-powered features
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── llm/              # LLM integration
        │   ├── mod.rs
        │   ├── openai.rs     # OpenAI API
        │   ├── anthropic.rs  # Anthropic API
        │   └── local.rs      # Local LLM (via Ollama/LM Studio)
        ├── strategy_gen/     # Natural language strategy generation
        │   ├── mod.rs
        │   ├── parser.rs     # Parse natural language
        │   └── compiler.rs   # Compile to strategy spec
        ├── reporting/        # Natural language reporting
        │   ├── mod.rs
        │   ├── summary.rs    # Generate summaries
        │   └── explanation.rs # Explain decisions
        └── discovery/        # Automated strategy discovery
            ├── mod.rs
            ├── genetic.rs    # Genetic programming for strategies
            └── reinforcement.rs # RL-based strategy learning
```

---

## 3. Component Design

### 3.1 ferrotick-ml: Feature Engineering

#### Technical Indicators (using `ta` crate)

```rust
// src/features/indicators.rs
use ta::{indicators::*, Next};

/// Feature engineering pipeline for OHLCV data.
pub struct FeatureEngineer {
    rsi: RelativeStrengthIndex,
    macd: MovingAverageConvergenceDivergence,
    bb: BollingerBands,
    atr: AverageTrueRange,
}

impl FeatureEngineer {
    pub fn new() -> Self {
        Self {
            rsi: RelativeStrengthIndex::new(14).unwrap(),
            macd: MovingAverageConvergenceDivergence::new(12, 26, 9).unwrap(),
            bb: BollingerBands::new(20, 2.0).unwrap(),
            atr: AverageTrueRange::new(14).unwrap(),
        }
    }
    
    /// Extract features from a bar.
    pub fn extract(&mut self, bar: &Bar) -> Features {
        let close = bar.close;
        let high = bar.high;
        let low = bar.low;
        
        Features {
            rsi: self.rsi.next(close),
            macd_line: self.macd.next(close).macd,
            macd_signal: self.macd.next(close).signal,
            bb_upper: self.bb.next(close).upper,
            bb_lower: self.bb.next(close).lower,
            atr: self.atr.next(DataItem {
                high, low, close,
                open: bar.open,
                volume: bar.volume.unwrap_or(0) as f64,
            }),
            returns: 0.0, // computed from prior close
            log_returns: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Features {
    pub rsi: f64,
    pub macd_line: f64,
    pub macd_signal: f64,
    pub bb_upper: f64,
    pub bb_lower: f64,
    pub atr: f64,
    pub returns: f64,
    pub log_returns: f64,
}
```

#### Feature Store (DuckDB-backed)

```rust
// src/features/store.rs
pub struct FeatureStore {
    warehouse: Warehouse,
}

impl FeatureStore {
    /// Store computed features for a symbol.
    pub async fn store_features(
        &self,
        symbol: &Symbol,
        interval: Interval,
        features: &[FeatureRow],
    ) -> Result<(), MlError> {
        // Insert into DuckDB features table
        // Partitioned by symbol, interval, date
    }
    
    /// Retrieve features for backtesting or inference.
    pub async fn get_features(
        &self,
        symbol: &Symbol,
        interval: Interval,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<FeatureRow>, MlError> {
        // Query from DuckDB with efficient columnar access
    }
}
```

### 3.2 ferrotick-ml: Model Integration

#### Candle Integration (Deep Learning)

```rust
// src/models/forecasting.rs
use candle_core::{Tensor, Device};
use candle_nn::{LSTM, RNN};

/// LSTM-based price forecasting model.
pub struct LstmForecaster {
    lstm: LSTM,
    device: Device,
}

impl LstmForecaster {
    pub fn new(input_size: usize, hidden_size: usize) -> Result<Self, MlError> {
        let device = Device::Cpu;
        let lstm = LSTM::new(input_size, hidden_size, &device)?;
        Ok(Self { lstm, device })
    }
    
    /// Forecast next N steps.
    pub fn forecast(&self, features: &Tensor, steps: usize) -> Result<Tensor, MlError> {
        // Run LSTM forward pass
        // Return predicted prices
    }
    
    /// Load model from ONNX or Candle format.
    pub fn load(path: &Path) -> Result<Self, MlError> {
        // Deserialize model weights
    }
}
```

#### Linfa Integration (Classical ML)

```rust
// src/models/classification.rs
use linfa::prelude::*;
use linfa_svm::Svm;
use linfa_trees::DecisionTree;

/// Signal classifier (Buy/Sell/Hold).
pub struct SignalClassifier {
    model: Svm<f64, bool>, // Or DecisionTree
}

impl SignalClassifier {
    /// Train classifier on labeled features.
    pub fn train(
        features: &Array2<f64>,
        labels: &Array1<Signal>,
    ) -> Result<Self, MlError> {
        let dataset = Dataset::new(features.clone(), labels.map(|s| *s == Signal::Buy));
        let model = Svm::params()
            .pos_weight(1.0)
            .fit(&dataset)?;
        Ok(Self { model })
    }
    
    /// Predict signal for features.
    pub fn predict(&self, features: &Array2<f64>) -> Vec<Signal> {
        // Return predicted signals
    }
}
```

#### ONNX Runtime (Production Serving)

```rust
// src/inference/onnx.rs
use ort::{Session, GraphOptimizationLevel};

/// ONNX model inference engine.
pub struct OnnxInferenceEngine {
    session: Session,
}

impl OnnxInferenceEngine {
    pub fn new(model_path: &Path) -> Result<Self, MlError> {
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::All)?
            .commit_from_file(model_path)?;
        Ok(Self { session })
    }
    
    /// Run inference on feature tensor.
    pub fn infer(&self, features: &[f64]) -> Result<InferenceResult, MlError> {
        let input = Array::from_shape_vec((1, features.len()), features)?;
        let outputs = self.session.run(ort::inputs![input]?)?;
        // Extract predictions
    }
}
```

### 3.3 ferrotick-backtest: Event-Driven Engine

```rust
// src/engine/event_driven.rs
use tokio::sync::mpsc;

/// Event-driven backtesting engine.
pub struct BacktestEngine {
    data_feed: Box<dyn DataFeed>,
    strategy: Box<dyn Strategy>,
    portfolio: Portfolio,
    executor: OrderExecutor,
    event_bus: EventBus,
}

/// Events processed by the engine.
pub enum BacktestEvent {
    Bar(Bar),
    Signal(Signal),
    Order(Order),
    Fill(Fill),
    Timer(DateTime<Utc>),
}

impl BacktestEngine {
    /// Run backtest over historical data.
    pub async fn run(&mut self, config: BacktestConfig) -> Result<BacktestResult, BacktestError> {
        let mut events = self.data_feed.subscribe();
        
        while let Some(event) = events.recv().await {
            match event {
                BacktestEvent::Bar(bar) => {
                    // Update portfolio value
                    self.portfolio.update(&bar);
                    
                    // Generate signals
                    if let Some(signal) = self.strategy.on_bar(&bar) {
                        self.event_bus.publish(BacktestEvent::Signal(signal));
                    }
                }
                BacktestEvent::Signal(signal) => {
                    // Create orders from signals
                    if let Some(order) = self.strategy.create_order(&signal, &self.portfolio) {
                        self.event_bus.publish(BacktestEvent::Order(order));
                    }
                }
                BacktestEvent::Order(order) => {
                    // Execute order with slippage/fees
                    if let Some(fill) = self.executor.execute(&order, &self.data_feed.current_bar()) {
                        self.event_bus.publish(BacktestEvent::Fill(fill));
                    }
                }
                BacktestEvent::Fill(fill) => {
                    // Update portfolio
                    self.portfolio.apply_fill(&fill);
                }
                _ => {}
            }
        }
        
        self.generate_report()
    }
}
```

### 3.4 ferrotick-backtest: Performance Metrics

```rust
// src/metrics/risk.rs

/// Comprehensive performance metrics calculator.
pub struct PerformanceMetrics {
    returns: Vec<f64>,
    benchmark_returns: Vec<f64>,
    risk_free_rate: f64,
}

impl PerformanceMetrics {
    /// Calculate Sharpe ratio.
    pub fn sharpe_ratio(&self) -> f64 {
        let mean = self.returns.iter().sum::<f64>() / self.returns.len() as f64;
        let std = self.std_dev(&self.returns);
        (mean - self.risk_free_rate) / std
    }
    
    /// Calculate Sortino ratio.
    pub fn sortino_ratio(&self) -> f64 {
        let mean = self.returns.iter().sum::<f64>() / self.returns.len() as f64;
        let downside_std = self.downside_deviation();
        (mean - self.risk_free_rate) / downside_std
    }
    
    /// Calculate maximum drawdown.
    pub fn max_drawdown(&self) -> DrawdownResult {
        let mut peak = 0.0;
        let mut max_dd = 0.0;
        let mut cumulative = 1.0;
        
        for r in &self.returns {
            cumulative *= 1.0 + r;
            peak = peak.max(cumulative);
            let dd = (peak - cumulative) / peak;
            max_dd = max_dd.max(dd);
        }
        
        DrawdownResult { max_drawdown: max_dd }
    }
    
    /// Calculate Value at Risk (VaR).
    pub fn var(&self, confidence: f64) -> f64 {
        let mut sorted = self.returns.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = ((1.0 - confidence) * sorted.len() as f64) as usize;
        sorted[index]
    }
    
    /// Calculate Conditional VaR (CVaR / Expected Shortfall).
    pub fn cvar(&self, confidence: f64) -> f64 {
        let var = self.var(confidence);
        self.returns.iter()
            .filter(|&&r| r < var)
            .sum::<f64>() / self.returns.iter().filter(|&&r| r < var).count() as f64
    }
}

#[derive(Debug, Serialize)]
pub struct BacktestReport {
    pub total_return: f64,
    pub annualized_return: f64,
    pub volatility: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub max_drawdown: f64,
    pub var_95: f64,
    pub cvar_95: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub trades: usize,
    pub benchmark_comparison: BenchmarkComparison,
}
```

### 3.5 ferrotick-strategies: Strategy Library

```rust
// src/library/mean_reversion.rs

/// Mean reversion strategy using RSI oversold/overbought.
pub struct RsiMeanReversion {
    rsi_period: usize,
    oversold: f64,
    overbought: f64,
    position_size: f64,
}

impl Strategy for RsiMeanReversion {
    fn on_bar(&mut self, bar: &Bar) -> Option<Signal> {
        let rsi = self.rsi.next(bar.close);
        
        if rsi < self.oversold {
            Some(Signal::Buy { 
                strength: (self.oversold - rsi) / self.oversold,
                reason: "RSI oversold".to_string(),
            })
        } else if rsi > self.overbought {
            Some(Signal::Sell {
                strength: (rsi - self.overbought) / (100.0 - self.overbought),
                reason: "RSI overbought".to_string(),
            })
        } else {
            None
        }
    }
    
    fn create_order(&self, signal: &Signal, portfolio: &Portfolio) -> Option<Order> {
        match signal {
            Signal::Buy { strength, .. } => {
                let quantity = (portfolio.cash * self.position_size * strength) / portfolio.current_price;
                Some(Order::market_buy(portfolio.symbol.clone(), quantity))
            }
            Signal::Sell { strength, .. } => {
                let quantity = portfolio.position * strength;
                Some(Order::market_sell(portfolio.symbol.clone(), quantity))
            }
        }
    }
}
```

### 3.6 ferrotick-ai: LLM Integration

```rust
// src/strategy_gen/compiler.rs

/// Natural language to strategy compiler.
pub struct StrategyCompiler {
    llm: Box<dyn LlmClient>,
}

impl StrategyCompiler {
    /// Parse natural language strategy description.
    pub async fn compile(&self, description: &str) -> Result<StrategySpec, AiError> {
        let prompt = format!(
            r#"Convert this trading strategy description into a structured strategy specification:

Description: {}

Output a JSON object with:
- name: strategy name
- type: "momentum" | "mean_reversion" | "trend_following" | "pairs" | "ml"
- entry_rules: list of entry conditions (indicator, operator, value)
- exit_rules: list of exit conditions
- position_sizing: {{ "method": "fixed" | "percent" | "kelly", "value": number }}
- risk_management: {{ "stop_loss": number, "take_profit": number }}

JSON:"#,
            description
        );
        
        let response = self.llm.complete(&prompt).await?;
        let spec: StrategySpec = serde_json::from_str(&response)?;
        spec.validate()?;
        Ok(spec)
    }
}

/// Example usage:
/// 
/// ```
/// let compiler = StrategyCompiler::new(OpenAiClient::new("gpt-4"));
/// let spec = compiler.compile(
///     "Mean reversion strategy using RSI oversold conditions with 2% position sizing"
/// ).await?;
/// 
/// // Generated spec:
/// // {
/// //   "name": "rsi_mean_reversion",
/// //   "type": "mean_reversion",
/// //   "entry_rules": [{"indicator": "rsi", "operator": "<", "value": 30}],
/// //   "exit_rules": [{"indicator": "rsi", "operator": ">", "value": 70}],
/// //   "position_sizing": {"method": "percent", "value": 0.02},
/// //   "risk_management": {"stop_loss": 0.05, "take_profit": 0.10}
/// // }
/// ```
```

---

## 4. Data Flow (Target)

```
┌─────────────────────────────────────────────────────────────────┐
│                     Data Sources                                 │
│  Yahoo │ Polygon │ AlphaVantage │ Alpaca │ Custom (WebSocket)   │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                   ferrotick-core                                 │
│  DataSource trait │ Adapters │ Circuit Breaker │ Router         │
│  Domain: Quote, Bar, Fundamental, Instrument                    │
└──────────────────────────┬──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                 ferrotick-warehouse                              │
│  DuckDB Storage │ Parquet Files │ Feature Store                 │
│  Tables: bars_1d, bars_1m, quotes, fundamentals, features       │
└───────────┬─────────────────────────────────────┬───────────────┘
            │                                     │
            ▼                                     ▼
┌───────────────────────┐           ┌─────────────────────────────┐
│   ferrotick-ml        │           │   ferrotick-backtest        │
│  Feature Engineering  │           │   Event-Driven Engine       │
│  Technical Indicators │◄──────────┤   Vectorized Engine         │
│  ML Models (Candle)   │           │   Portfolio Management      │
│  ONNX Inference       │           │   Performance Metrics       │
│  Training Pipeline    │           │   Strategy Optimization     │
└───────────┬───────────┘           └───────────┬─────────────────┘
            │                                   │
            ▼                                   ▼
┌───────────────────────┐           ┌─────────────────────────────┐
│ ferrotick-strategies  │           │     ferrotick-ai            │
│  Strategy Library     │           │   LLM Strategy Compiler     │
│  Signal Generation    │           │   Natural Language Reports  │
│  Execution Logic      │           │   Strategy Discovery (GA)   │
└───────────┬───────────┘           └───────────┬─────────────────┘
            │                                   │
            └───────────────────┬───────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                     ferrotick-cli                                │
│  Commands: quote, bars, ml, backtest, optimize, analyze         │
│  Output: JSON, NDJSON, Table, Charts (ASCII/terminal)           │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                   ferrotick-agent                                │
│  JSON Envelopes │ NDJSON Streaming │ Schema Validation          │
│  AI-Agent Integration │ MCP Server                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. CLI Commands (Target)

```bash
# Feature Engineering
ferrotick ml features AAPL --indicators rsi,macd,bollinger,atr --output features.parquet
ferrotick ml features AAPL --start 2023-01-01 --end 2024-01-01 --store

# Model Training
ferrotick ml train --data features.parquet --model lstm --target returns --output model.onnx
ferrotick ml train --data features.parquet --model svm --target signal --output model.bin
ferrotick ml evaluate --model model.onnx --test-data test.parquet

# Backtesting
ferrotick backtest --strategy rsi_mean_reversion.yaml --symbols AAPL,MSFT --start 2020-01-01 --capital 100000
ferrotick backtest --strategy ml_predictor --model model.onnx --symbols SPY --start 2022-01-01

# Strategy Optimization
ferrotick optimize --strategy momentum.yaml --method genetic --generations 100 --population 50
ferrotick optimize --strategy mean_reversion.yaml --method bayesian --trials 200
ferrotick validate --strategy optimized.yaml --method walk-forward --windows 12

# Analysis & Reporting
ferrotick analyze backtest-results.json --metrics sharpe,sortino,max-dd,win-rate
ferrotick report backtest-results.json --format markdown --output report.md

# AI-Powered Features
ferrotick strategy create --prompt "Mean reversion using RSI with 2% position sizing"
ferrotick strategy discover --symbols SPY,QQQ --method genetic --generations 500
ferrotick explain backtest-results.json --query "Why did the strategy underperform in Q3?"

# Warehouse Operations (Enhanced)
ferrotick warehouse query "SELECT * FROM features WHERE symbol='AAPL' AND rsi < 30"
ferrotick warehouse export --table features --format parquet --output features.parquet
```

---

## 6. Technology Stack

### Core Dependencies

| Category | Crate | Purpose |
|----------|-------|---------|
| **ML Framework** | `candle-core`, `candle-nn` | Deep learning (LSTM, Transformers) |
| **ML Framework** | `burn` | Alternative deep learning framework |
| **Classical ML** | `linfa` | SVM, Decision Trees, Clustering |
| **Technical Analysis** | `ta` | RSI, MACD, Bollinger Bands, etc. |
| **Numerical Computing** | `ndarray` | N-dimensional arrays |
| **DataFrames** | `polars` | Fast DataFrame operations |
| **Linear Algebra** | `nalgebra` | Matrix operations |
| **ONNX Runtime** | `ort` | Model serving in production |
| **Async Runtime** | `tokio` | Async event processing |
| **Serialization** | `serde`, `serde_json` | JSON/serialization |
| **Time Series** | `chrono`, `time` | Date/time handling |

### Optional Dependencies

| Category | Crate | Purpose |
|----------|-------|---------|
| **Python Interop** | `pyo3` | Python bindings for complex ML |
| **Visualization** | `plotters` | Plotting (optional) |
| **LLM Clients** | `async-openai` | OpenAI API |
| **HTTP** | `reqwest` | API calls |

---

## 7. Security Considerations

### ML Model Security

1. **Input Validation**: All feature vectors validated before inference
2. **Model Sandboxing**: ONNX models run in isolated context
3. **Resource Limits**: Inference timeout and memory limits
4. **No Arbitrary Code**: Models are data-only (no executable code)

### Backtesting Security

1. **No External Calls**: Backtests run in sandboxed environment
2. **Deterministic Execution**: Same inputs → same outputs
3. **Audit Logging**: All trades logged with timestamps
4. **Look-Ahead Prevention**: Point-in-time data enforcement

### LLM Integration Security

1. **Prompt Injection Prevention**: Sanitize user inputs
2. **Rate Limiting**: Prevent API abuse
3. **Output Validation**: Validate generated strategy specs
4. **No Code Execution**: LLM generates specs, not executable code

---

## 8. Performance Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Feature extraction | < 1ms per 1000 bars | Real-time ML inference |
| Event-driven backtest | 100K+ events/sec | Fast strategy iteration |
| Vectorized backtest | 1M+ bars/sec | Large-scale optimization |
| Model inference (ONNX) | < 10ms | Real-time trading |
| DuckDB query | < 50ms for 1M rows | Interactive analysis |

---

## 9. Extension Points

### Adding New ML Models

1. Implement `Model` trait in `ferrotick-ml/src/models/`
2. Register model type in `ModelRegistry`
3. Add CLI command in `ferrotick-cli`

### Adding New Strategies

1. Implement `Strategy` trait in `ferrotick-strategies/src/library/`
2. Register in `StrategyRegistry`
3. Add YAML schema in `ferrotick-strategies/src/spec/`

### Adding New Data Sources

1. Implement `DataSource` trait in `ferrotick-core/src/adapters/`
2. Register in `SourceRouter`
3. Add CLI `--source` option

---

## 10. Comparison with Existing Solutions

| Feature | ferrotick | yfinance | Backtrader | QuantLib | Barter.rs |
|---------|-----------|----------|------------|----------|-----------|
| Language | Rust | Python | Python | C++ | Rust |
| Data Sources | Multi-provider | Yahoo only | Multi | None | Multi (crypto) |
| ML Native | ✅ (planned) | ❌ | ❌ | ❌ | ❌ |
| Backtesting | ✅ (planned) | ❌ | ✅ | ✅ | ✅ |
| Real-time | ✅ | ❌ | ❌ | ❌ | ✅ |
| Performance | ⚡⚡⚡ | ⚡ | ⚡⚡ | ⚡⚡⚡ | ⚡⚡⚡ |
| AI Integration | ✅ (planned) | ❌ | ❌ | ❌ | ❌ |
| Feature Store | ✅ (planned) | ❌ | ❌ | ❌ | ❌ |

---

## Next Steps

1. Review and approve this architecture
2. Create `ROADMAP.md` with phased implementation plan
3. Create `IMPLEMENTATION.md` with detailed task breakdown
4. Start Phase 1: Feature engineering module
