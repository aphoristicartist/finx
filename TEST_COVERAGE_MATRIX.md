# TEST_COVERAGE_MATRIX

## Scope (Code-only)
- Analysis is based only on source and test code under `crates/**` and `tests/**`.
- Public API inventory is phase-scoped and built from crate-root exports (`pub use`), user-facing command/route entrypoints, and explicit phase-marked modules.
- Coverage percentages below are API-level ("API has at least one meaningful test reference"), not line/branch coverage.

## Phase Mapping Note
- Explicit phase markers exist in code for: Phase 7, 8, 9, 10, and 17.
- Phases without explicit markers are inferred from crate boundaries and public API layering.

## Test Suite Integrity Findings
- Registered root integration tests in `tests/Cargo.toml`: `integration_test`, `multiasset_behavioral`, `integration_workflows`, `edge_cases`, `mathematical_correctness`, `state_management`, `error_handling_behavior`.
- Unregistered root test files (present but not run by default): `assets_test.rs`, `cli_user_journeys.rs`, `data_provider_behavior.rs`, `error_handling_security.rs`, `provider_contract.rs`, `warehouse_behavior.rs`.
- Compile-broken registered tests:
- `tests/mathematical_correctness.rs` fails with `E0689` (ambiguous float type for `.abs()` / `.powf()`).
- `tests/state_management.rs` fails with `E0308` (`Symbol::from("...")` type mismatch).

## Phase-by-Phase Review

### Phase 0 - Core Domain Models
- Public APIs: `AssetClass`, `Symbol`, `Interval`, `UtcDateTime`, `Quote`, `Bar`, `BarSeries`, `Fundamental`, `Instrument`, `StatementType`, `FinancialPeriod`, `FinancialLineItem`, `FinancialStatement`, `EarningsEntry`, `EarningsReport`, `CorporateActionType`, `CorporateAction`, `ValidationError`.
- Tested: Yes (unit tests in `domain/symbol.rs`, `domain/interval.rs`, `domain/timestamp.rs`, `domain/models.rs`; cross-crate use in integration tests).
- Missing tests: deep validation for `Financial*`/`Earnings*` constructors, `CorporateAction` edge validation, invalid currency-code permutations.
- Quality: Mixed. Domain invariants are tested, but financial-statement surface is mostly coverage-light.

### Phase 1 - Data Source Contracts
- Public APIs: `DataSource`, `Endpoint`, `CapabilitySet`, `HealthState`, `HealthStatus`, `SourceError`, `SourceErrorKind`, `QuoteRequest`, `BarsRequest`, `FundamentalsRequest`, `SearchRequest`, `FinancialsRequest`, `EarningsRequest`, `QuoteBatch`, `FundamentalsBatch`, `SearchBatch`, `FinancialsBatch`, `EarningsBatch`.
- Tested: Partial (`QuoteRequest`/`BarsRequest`/`SearchRequest` well covered; health and capability checks in provider tests).
- Missing tests: `FinancialsRequest`, `EarningsRequest`, `FinancialsBatch`, `EarningsBatch`, `SourceError` helper constructors.
- Quality: Mostly coverage-oriented; behavioral contract depth is limited for non-quote endpoints.

### Phase 2 - Provider Adapters
- Public APIs: `PolygonAdapter`, `YahooAdapter`, `AlpacaAdapter`, `AlphaVantageAdapter`.
- Tested: Yes (unit smoke tests per adapter; additional contract-like tests exist but are not all wired/executed).
- Missing tests: real parsing/regression scenarios, retry/backoff behavior, provider-specific error mapping, latency/rate-limit semantics.
- Quality: Surface coverage only; limited behavioral realism.

### Phase 3 - Routing and Reliability
- Public APIs: `SourceRouter`, `SourceRouterBuilder`, `SourceStrategy`, `SourceSnapshot`, `RouteSuccess`, `RouteFailure`, `RouteResult`, `CircuitBreaker`, `CircuitBreakerConfig`, `CircuitState`, `RetryConfig`, `Backoff`, `ThrottlingQueue`, `ProviderPolicy`, `BackoffPolicy`.
- Tested: Mostly yes (routing unit tests, circuit/retry/throttle/provider-policy tests).
- Missing tests: concurrent routing under load, adapter-failure matrix across all strategies, queue contention behavior.
- Quality: Good for component-level correctness; weaker for end-to-end behavior.

### Phase 4 - CLI Public Surface
- Public APIs: `Cli`, `OutputFormat`, `SourceSelector`, `Command`, `QuoteArgs`, `BarsArgs`, `FundamentalsArgs`, `SearchArgs`, `FinancialsArgs`, `EarningsArgs`, `SqlArgs`, `CacheArgs`, `CacheLoadArgs`, `ExportArgs`, `MlArgs`, `MlCommand`, `MlFeaturesArgs`, `MlExportArgs`, `CacheCommand`, `SchemaArgs`, `SchemaCommand`, `SchemaGetArgs`, `SourcesArgs`, `StrategyArgs`, `StrategyCommand`, `StrategyValidateArgs`, `StrategyBacktestArgs`, `commands::run`, `output::render`, `output::render_stream`.
- Tested: Limited (parser tests exist; command-level behavioral suites largely unregistered).
- Missing tests: command dispatch branches (`financials`, `earnings`, `cache load`, `export`, `strategy`, `schema`, `sources`, `ml export`), strict/explain edge flows, output format compatibility.
- Quality: Mostly coverage/smoke; high-value behavioral CLI tests are present in files but not executed by default.

### Phase 5 - Warehouse
- Public APIs: `Warehouse`, `WarehouseConfig`, `WarehouseError`, `QueryGuardrails`, `SqlColumn`, `QueryResult`, `CacheSyncReport`, `QuoteRecord`, `BarRecord`, `FundamentalRecord`, `AccessMode`, `DuckDbConnectionManager`, `PooledConnection`.
- Tested: Strong (warehouse crate unit tests cover migrations/init, SQL guardrails, ingest safety, sync idempotency, perf target).
- Missing tests: high-contention pool scenarios, partial-failure transaction semantics, corruption recovery tests.
- Quality: Strong behavioral and security coverage at crate level.

### Phase 6 - Agent Protocol
- Public APIs: `EnvelopeBuilder`, `EnvelopeValidator`, `AgentMetadata`, `RequestId`, `TraceId`, `SchemaRegistry`, `SchemaValidationError`, `NdjsonStreamWriter`, `StreamEvent`, `StreamEventType`, `StreamEventError`, `parse_stream_events`, `validate_stream`, `to_deterministic_json`.
- Tested: Strong (unit suites in `envelope.rs`, `metadata.rs`, `stream.rs`, `schema_registry.rs`).
- Missing tests: CLI-to-agent integration and schema-dir deployment path failure scenarios.
- Quality: Predominantly behavioral and contract-oriented.

### Phase 7 - Feature Engineering
- Public APIs: `FeatureConfig`, `IndicatorSelection`, `FeatureEngineer`, `FeatureRow`, `FeatureStore`, `compute_rsi`, `compute_macd`, `compute_bollinger`.
- Tested: Yes (`behavioral_indicators`, `phase7_feature_pipeline`).
- Missing tests: malformed numeric input (`NaN`/`inf`) handling, window-edge parameter validation breadth, store concurrency tests.
- Quality: Behavioral for indicator semantics; moderate gaps in robustness scenarios.

### Phase 8 - Backtesting
- Public APIs: `BacktestConfig`, `BacktestEngine`, `BacktestEvent`, `BacktestReport`, `BarEvent`, `SignalAction`, `SignalEvent`, `Strategy` (backtest trait), `Portfolio`, `Position`, `Order`, `OrderSide`, `OrderType`, `OrderStatus`, `Fill`, `CashLedger`, `FeeModel`, `SlippageModel`, `TransactionCosts`, `MetricsReport`, `PerformanceMetrics`, `VectorizedBacktest`.
- Tested: Broad (`behavioral_portfolio`, `vectorized_test`, `edge_cases`, integration workflows).
- Missing tests: limit/stop order lifecycle behavior, event ordering invariants, detailed risk metric edge validation, more vectorized failure-path coverage.
- Quality: Good behavioral portfolio coverage; some API regions remain coverage-driven.

### Phase 9 - Strategy Framework
- Public APIs: `MovingAverageCrossoverStrategy`, `RsiMeanReversionStrategy`, `MacdTrendStrategy`, `BollingerBandSqueezeStrategy`, `built_in_strategies`, `Strategy`, `Signal`, `SignalAction`, `Order`, `OrderSide`, `parse_and_validate_file`, `validate_strategy_spec`, `ValidationIssue`, `SignalGenerator`, `CompositeSignalGenerator`, `CompositeMode`.
- Tested: Strong (`strategies_test` + `behavioral_signals`).
- Missing tests: file I/O error paths for DSL file parsing, a few invalid-builder branches.
- Quality: High behavioral quality (signal semantics, warmup, memory bounds, composition behavior).

### Phase 10 - Supervised ML
- Public APIs: `SVMClassifier`, `DecisionTreeClassifier`, `Model`, `Dataset`, `DatasetBuilder`, `TargetColumn`, `ModelMetrics`, `cross_validate`, `MlError`, `MlResult`.
- Tested: Partial (`phase10_svm`, `phase10_decision_tree`, `behavioral_learning`).
- Missing tests: `DatasetBuilder::build`, `Dataset::train_test_split`, `Dataset::normalize`, `cross_validate` failure/edge branches, batch-predict failure paths.
- Quality: Behavioral model tests exist, but training/evaluation utility APIs are under-tested.

### Phase 11 - Reinforcement Learning
- Public APIs: `TradingEnvironment`, `Environment`, `Action`, `Position`, `TradingState`, `StepResult`, `QTableAgent`, `QTableConfig`, `RandomAgent`, `RewardCalculator`, `RewardConfig`.
- Tested: Partial (`rl_test`).
- Missing tests: Q-table update math, epsilon decay behavior, state bucketing transitions, terminal-step invariants.
- Quality: Mostly smoke-level with light behavioral checks.

### Phase 12 - Optimization
- Public APIs: `GridSearchOptimizer`, `ParamRange`, `ParamResult`, `OptimizationReport`, `WalkForwardValidator`, `WalkForwardWindow`, `WalkForwardSummary`, `OptimizationStorage`, `OptimizationRun`.
- Tested: Strong (`optimization_test` + module unit tests).
- Missing tests: optimizer failure propagation when backtests error out, more invalid-parameter/empty-window cases.
- Quality: Mostly behavioral and workflow-driven.

### Phase 13 - AI Layer
- Public APIs: `StrategyCompiler`, `BacktestReporter`, `LLMClient`, `OpenAIClient`, `OutputSanitizer`, `StrategySpec` (re-export).
- Tested: Very limited (only `OutputSanitizer` unit tests).
- Missing tests: `StrategyCompiler::compile`, `BacktestReporter::{explain, analyze_risk}`, `OpenAIClient::complete` success/failure mapping, schema-validation feedback behavior.
- Quality: Coverage gap; core behavior untested.

### Phase 14 - Trading Execution
- Public APIs: `PaperTradingEngine`, `PaperAccount`, `Position` (trading), `AlpacaClient`, `AlpacaOrder`, `AlpacaAccount`, `AlpacaOrderResponse`, `TradingError`.
- Tested: Minimal (`trading_test` has basic construction checks).
- Missing tests: paper engine order execution behavior, account/position transitions over bars, broker error handling.
- Quality: Smoke-only.

### Phase 15 - Web API
- Public APIs: `health_check`, `run_backtest`, `list_strategies`, `WebError`, `BacktestRequest`, `BacktestResponse`.
- Tested: Minimal (`web_test` only checks `/health`).
- Missing tests: `/api/backtest/run` and `/api/strategies` behavior, payload validation, error mapping.
- Quality: Smoke-only.

### Phase 16 - System Hardening / Orchestration
- Public APIs: none added as a dedicated crate-level API surface.
- Tested: N/A as phase-owned API set; integration coverage exists but is fragmented.
- Missing tests: full provider->warehouse->feature->model->strategy->backtest end-to-end with executable test harness and no ignored/unwired files.
- Quality: Integration intent exists; execution wiring is incomplete.

### Phase 17 - Multi-Asset
- Public APIs: `OptionContract`, `OptionType`, `Greeks`, `FuturesContract`, `ForexPair`, `CryptoPair`, `CryptoExchange`.
- Tested: Mostly yes (`multiasset_behavioral` + `assets_test.rs` file exists but is currently unregistered in root test crate).
- Missing tests: explicit coverage for `CryptoExchange`, invalid constructor inputs/constraints for derivatives and FX rates.
- Quality: Good behavioral tests for options/futures/forex; crypto exchange semantics remain thin.

## Integration Test Coverage

### Cross-crate workflows found
- Core + Strategies + Backtest compile/use path: `tests/integration_test.rs`.
- Core + ML feature pipeline + SVM classification: `tests/integration_workflows.rs::test_full_data_to_signal_pipeline`.
- ML predictions into Backtest engine: `tests/integration_workflows.rs::test_backtest_with_ml_strategy`.
- Optimization + Backtest loop: `tests/integration_workflows.rs::test_strategy_optimization_workflow` and `crates/ferrotick-optimization/tests/optimization_test.rs`.
- Multi-asset event stream into Backtest: `tests/integration_workflows.rs::test_multi_asset_portfolio_backtest`.
- ML FeatureStore + Warehouse roundtrip: `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs::store_roundtrip_and_parquet_export_work`.

### Integration gaps
- No executed end-to-end CLI command flow from parse -> command -> envelope -> output for most commands.
- No executed adapter real-data contract suite across providers (ignored or unregistered tests).
- No executed AI-to-strategy/backtest integration workflow.
- No executed trading-engine integration with strategies/backtest feedback loop.
- Web layer is not integration-tested beyond health endpoint.
- Agent crate is not integration-tested against CLI/web emitted payloads as a contract gate.

## Edge Case Coverage

### Empty inputs
- Covered: empty bars to backtest (`NoMarketData`), empty quote/search request validation, empty symbol list checks.
- Missing: empty AI prompt/description behavior, empty web payload validation, empty optimization bars semantics at API boundary.

### Extreme values
- Covered: `f64::MAX`/`f64::MIN` bar checks, large dataset memory estimate tests.
- Missing: extreme ML feature numeric stability (`NaN`/`inf`), extreme warehouse ingest cardinality with contention, extreme trading leverage/position values.

### Error conditions
- Covered: SQL guardrail rejections, unsupported endpoint errors, circuit-breaker states, invalid request validation.
- Missing: AI client transport/parsing failure assertions, web error translation assertions, paper-trading engine error matrix.

### Concurrent access
- Covered: synthetic mutex-based concurrency tests in root suite.
- Missing: real concurrent access tests for `SourceRouter`, `CacheStore`, warehouse connection pool, and stream writer safety boundaries.

## Requested Matrix

| Phase | Public APIs | Tested | Coverage % | Missing |
|---|---:|---:|---:|---|
| 0 | 18 | 12 | 67% | Financial/Earnings/Currency validation depth |
| 1 | 18 | 9 | 50% | Financials/Earnings request+batch contracts |
| 2 | 4 | 4 | 100% | Real provider behavioral/parsing coverage |
| 3 | 15 | 11 | 73% | Concurrent routing + retry/throttle integration |
| 4 | 30 | 7 | 23% | Most CLI command branch behavior |
| 5 | 13 | 11 | 85% | Pool contention + transactional failure paths |
| 6 | 14 | 12 | 86% | CLI/web integration contract coverage |
| 7 | 8 | 6 | 75% | Numeric robustness + store concurrency |
| 8 | 22 | 14 | 64% | Order lifecycle + event ordering + metrics edges |
| 9 | 16 | 14 | 88% | DSL file I/O and invalid-builder edges |
| 10 | 10 | 6 | 60% | Dataset/cross-validate utility coverage |
| 11 | 11 | 6 | 55% | Q-learning update/decay/state transition checks |
| 12 | 9 | 8 | 89% | Optimizer failure/invalid-window edge paths |
| 13 | 6 | 1 | 17% | Compiler/reporter/OpenAI behavior tests |
| 14 | 8 | 2 | 25% | Paper engine and broker behavior tests |
| 15 | 6 | 1 | 17% | Backtest/strategies route behavior tests |
| 16 | 0 | 0 | N/A | Full executable end-to-end orchestration suite |
| 17 | 7 | 6 | 86% | `CryptoExchange` and invalid multi-asset input constraints |

