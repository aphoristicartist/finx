# ROADMAP.md - Ferrotick Development Roadmap

## Overview

This roadmap outlines the phased development of ferrotick from a multi-provider financial data CLI to a comprehensive AI/ML-native financial data platform with integrated backtesting capabilities.

---

## Completed Phases (0-6)

### Phase 0: Foundation ✅
- [x] Workspace setup with 4 crates
- [x] Core domain models (Quote, Bar, Fundamental, Instrument)
- [x] DataSource trait for provider abstraction
- [x] DuckDB warehouse with secure parameterized queries

### Phase 1: Multi-Provider Support ✅
- [x] Yahoo Finance adapter
- [x] Polygon.io adapter
- [x] Alpha Vantage adapter
- [x] Alpaca adapter

### Phase 2: CLI & UX ✅
- [x] Command-line interface with clap
- [x] JSON, NDJSON, table output formats
- [x] Source routing with priority scoring
- [x] Circuit breaker for resilient calls

### Phase 3: Warehouse & Analytics ✅
- [x] DuckDB connection pooling
- [x] Parquet file integration
- [x] SQL query interface with guardrails
- [x] Cache sync functionality

### Phase 4: Agent UX ✅
- [x] JSON envelope specification
- [x] NDJSON streaming events
- [x] Schema registry
- [x] Request metadata tracking

### Phase 5: Security Hardening ✅
- [x] Parameterized SQL queries
- [x] SQL injection prevention tests
- [x] Query timeout enforcement
- [x] Row limit guardrails

### Phase 6: Documentation ✅
- [x] Inline rustdoc
- [x] README with examples
- [x] Schema documentation

---

## Upcoming Phases (7-12): AI/ML-Native Features

### Phase 7: Feature Engineering Module (2-3 weeks)

**Goal**: Build foundation for ML feature extraction from OHLCV data.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Create `ferrotick-ml` crate | P0 | 1 day | None |
| Integrate `ta` crate for technical indicators | P0 | 2 days | Crate setup |
| Implement RSI, MACD, Bollinger Bands, ATR | P0 | 3 days | ta crate |
| Feature transform functions (returns, log-returns) | P0 | 1 day | None |
| Rolling window features | P1 | 2 days | None |
| Feature normalization (z-score, min-max) | P1 | 1 day | None |
| Feature storage in DuckDB | P0 | 2 days | Warehouse |
| CLI `ml features` command | P0 | 2 days | Feature pipeline |
| Parquet export for features | P1 | 1 day | DuckDB |

#### Deliverables

- `ferrotick-ml` crate with feature engineering
- CLI command: `ferrotick ml features <symbol> --indicators rsi,macd`
- Feature storage in warehouse `features` table
- Parquet export capability

#### Key Dependencies

```toml
[dependencies]
ta = "0.5"           # Technical analysis indicators
ndarray = "0.15"     # Numerical computing
polars = { version = "0.41", features = ["lazy"] }
```

---

### Phase 8: Backtesting Engine (3-4 weeks)

**Goal**: Build high-performance event-driven backtesting engine.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Create `ferrotick-backtest` crate | P0 | 1 day | None |
| Design event system (Bar, Signal, Order, Fill) | P0 | 2 days | None |
| Implement event-driven backtest loop | P0 | 4 days | Event system |
| Portfolio tracking (positions, cash, equity) | P0 | 3 days | None |
| Order types (market, limit, stop) | P0 | 2 days | None |
| Order execution simulation | P0 | 2 days | Order types |
| Transaction cost modeling (slippage, fees) | P1 | 2 days | Execution |
| Performance metrics (Sharpe, Sortino, drawdown) | P0 | 3 days | None |
| VaR and CVaR calculations | P1 | 2 days | Returns |
| CLI `backtest` command | P0 | 3 days | Engine |
| Backtest result JSON output | P0 | 1 day | Metrics |
| Benchmark comparison (vs S&P 500) | P1 | 1 day | Metrics |

#### Deliverables

- `ferrotick-backtest` crate with event-driven engine
- CLI command: `ferrotick backtest --strategy <file> --symbols AAPL,MSFT`
- Performance metrics report (JSON)
- Transaction cost simulation

#### Key Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

---

### Phase 9: Strategy Library (2-3 weeks)

**Goal**: Provide pre-built strategies and strategy specification DSL.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Create `ferrotick-strategies` crate | P0 | 1 day | None |
| Define `Strategy` trait | P0 | 1 day | None |
| Implement Moving Average Crossover | P0 | 1 day | Strategy trait |
| Implement RSI Mean Reversion | P0 | 1 day | Strategy trait |
| Implement MACD Trend Following | P1 | 1 day | Strategy trait |
| Implement Bollinger Band Squeeze | P1 | 1 day | Strategy trait |
| Position sizing strategies | P0 | 2 days | None |
| Strategy specification YAML/JSON format | P0 | 2 days | None |
| Strategy spec parser and validator | P0 | 2 days | YAML format |
| Signal generation framework | P0 | 2 days | Strategy trait |
| Composite signals (combine multiple) | P1 | 2 days | Signal framework |
| CLI `strategy list` and `strategy validate` | P1 | 1 day | Parser |

#### Deliverables

- `ferrotick-strategies` crate with 5+ pre-built strategies
- Strategy specification format (YAML)
- Strategy validation and parsing
- CLI commands for strategy management

#### Example Strategy Spec

```yaml
# strategies/rsi_mean_reversion.yaml
name: rsi_mean_reversion
type: mean_reversion
timeframe: 1d

entry_rules:
  - indicator: rsi
    period: 14
    operator: "<"
    value: 30
    action: buy
    
exit_rules:
  - indicator: rsi
    period: 14
    operator: ">"
    value: 70
    action: sell

position_sizing:
  method: percent
  value: 0.02  # 2% of portfolio
  
risk_management:
  stop_loss: 0.05    # 5% stop loss
  take_profit: 0.10  # 10% take profit
```

---

### Phase 10: ML Model Integration (3-4 weeks)

**Goal**: Integrate ML frameworks for predictive modeling.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Add Candle dependencies | P0 | 0.5 day | None |
| Add Linfa dependencies | P0 | 0.5 day | None |
| Feature dataset preparation | P0 | 2 days | Feature engineering |
| Train/test split utilities | P0 | 1 day | Dataset prep |
| LSTM price forecasting model | P1 | 4 days | Candle |
| Transformer time-series model | P2 | 5 days | Candle |
| SVM signal classifier (Linfa) | P0 | 2 days | Linfa |
| Decision Tree classifier (Linfa) | P1 | 2 days | Linfa |
| Model serialization (ONNX) | P0 | 2 days | ONNX Runtime |
| ONNX inference engine | P0 | 2 days | ort crate |
| Model evaluation metrics (accuracy, precision, F1) | P0 | 2 days | None |
| Cross-validation utilities | P1 | 2 days | Evaluation |
| CLI `ml train` command | P0 | 2 days | Training pipeline |
| CLI `ml evaluate` command | P0 | 1 day | Evaluation |
| Feature store for ML features | P0 | 3 days | DuckDB |

#### Deliverables

- ML model training pipeline
- 3+ model types (LSTM, SVM, Decision Tree)
- ONNX model export and serving
- Feature store in DuckDB
- CLI commands: `ml train`, `ml evaluate`

#### Key Dependencies

```toml
[dependencies]
candle-core = "0.4"
candle-nn = "0.4"
linfa = "0.7"
linfa-svm = "0.7"
linfa-trees = "0.7"
ort = "2.0"  # ONNX Runtime
```

---

### Phase 11: Strategy Optimization (2-3 weeks)

**Goal**: Automated strategy parameter optimization.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Grid search optimization | P0 | 2 days | Backtest engine |
| Walk-forward optimization | P0 | 3 days | Backtest engine |
| Genetic algorithm framework | P1 | 4 days | None |
| Bayesian optimization | P1 | 3 days | None |
| Multi-objective optimization (return vs risk) | P2 | 3 days | Optimization |
| Parameter space definition | P0 | 1 day | None |
| Optimization result tracking | P0 | 2 days | DuckDB |
| Overfitting detection | P1 | 2 days | Cross-validation |
| CLI `optimize` command | P0 | 2 days | Optimization |
| CLI `validate` command | P0 | 1 day | Walk-forward |

#### Deliverables

- Grid search and walk-forward optimization
- Genetic algorithm for strategy discovery
- Bayesian hyperparameter tuning
- CLI commands: `optimize`, `validate`
- Optimization result storage

#### Key Dependencies

```toml
[dependencies]
rand = "0.8"
statrs = "0.16"
# Bayesian optimization: custom implementation or Python interop
```

---

### Phase 12: AI-Powered Features (3-4 weeks)

**Goal**: LLM integration for natural language strategy development.

#### Tasks

| Task | Priority | Effort | Dependencies |
|------|----------|--------|--------------|
| Create `ferrotick-ai` crate | P0 | 1 day | None |
| OpenAI API client | P0 | 2 days | async-openai |
| Anthropic API client | P1 | 2 days | HTTP client |
| Local LLM support (Ollama) | P1 | 2 days | HTTP client |
| Natural language → strategy spec compiler | P0 | 4 days | LLM client |
| Strategy spec → Rust code generator | P1 | 3 days | Parser |
| Natural language reporting | P0 | 2 days | LLM client |
| Backtest explanation generator | P1 | 2 days | LLM client |
| Prompt templates for finance | P0 | 2 days | None |
| Output validation and sanitization | P0 | 2 days | None |
| CLI `strategy create --prompt` command | P0 | 2 days | Compiler |
| CLI `explain` command | P1 | 1 day | Reporting |
| Rate limiting for API calls | P0 | 1 day | None |

#### Deliverables

- `ferrotick-ai` crate with LLM integration
- Natural language strategy creation
- Natural language backtest reports
- CLI commands: `strategy create --prompt`, `explain`

#### Key Dependencies

```toml
[dependencies]
async-openai = "0.20"
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
```

---

## Future Phases (13+)

### Phase 13: Vectorized Backtesting
- Vectorized engine for massive parameter sweeps
- Integration with DuckDB columnar operations
- 100x faster than event-driven for optimization

### Phase 14: Reinforcement Learning
- RL environment for trading
- DQN, PPO, A2C agents
- Custom reward functions

### Phase 15: Real-Time Trading
- Paper trading mode
- Broker integrations (Alpaca, Interactive Brokers)
- Live strategy execution

### Phase 16: Web Dashboard
- Actix-web or Axum server
- Real-time backtest visualization
- Strategy performance monitoring

### Phase 17: Multi-Asset Support
- Options pricing and Greeks
- Futures and commodities
- Forex pairs
- Crypto exchanges

---

## Timeline Summary

| Phase | Duration | Start | End | Status |
|-------|----------|-------|-----|--------|
| 0-6: Foundation | 6 weeks | Q1 2025 | Q2 2025 | ✅ Complete |
| 7: Feature Engineering | 2-3 weeks | Q1 2026 | Q1 2026 | 🔲 Planned |
| 8: Backtesting Engine | 3-4 weeks | Q1 2026 | Q2 2026 | 🔲 Planned |
| 9: Strategy Library | 2-3 weeks | Q2 2026 | Q2 2026 | 🔲 Planned |
| 10: ML Integration | 3-4 weeks | Q2 2026 | Q3 2026 | 🔲 Planned |
| 11: Optimization | 2-3 weeks | Q3 2026 | Q3 2026 | 🔲 Planned |
| 12: AI Features | 3-4 weeks | Q3 2026 | Q4 2026 | 🔲 Planned |

**Total AI/ML Initiative**: 15-21 weeks (~4-5 months)

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ML ecosystem immaturity | Medium | High | Python interop via PyO3 for complex models |
| Overfitting in backtests | High | High | Walk-forward validation, out-of-sample testing |
| LLM API costs | Medium | Medium | Local LLM support, caching, rate limiting |
| Performance regressions | Low | High | Benchmark suite, CI performance gates |
| Data quality issues | Medium | High | Validation, cleaning, point-in-time enforcement |

---

## Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Feature extraction speed | < 1ms per 1000 bars | Benchmark |
| Backtest events/sec | 100K+ | Benchmark |
| Strategy optimization time | < 1 hour for 10K combinations | Benchmark |
| ML inference latency | < 10ms | Benchmark |
| LLM strategy accuracy | 80%+ valid specs | Testing |
| User adoption | 100+ GitHub stars | Analytics |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on contributing to ferrotick.

## License

MIT License - see [LICENSE](LICENSE) for details.
