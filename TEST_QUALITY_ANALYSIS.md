# TEST QUALITY ANALYSIS

## Scope
- Scanned **51 Rust files** containing test attributes (`#[test]`, `#[tokio::test]`, or `#[cfg(test)]`).
- Parsed **323 test functions** across workspace crates and root `tests/`.
- Found **53 weak-assertion occurrences** across **40 tests**.

## 1) Tests Using Weak Assertions
- `assert!(true)`: **0 occurrences**
- `.is_ok()`: **26 occurrences**
- `.is_some()`: **27 occurrences**

### Full Weak-Assertion Inventory
- `crates/ferrotick-agent/src/envelope.rs:493` `validator_accepts_valid_envelope` -> `.is_ok()`
- `crates/ferrotick-agent/src/metadata.rs:266` `request_id_parses_valid_uuid` -> `.is_ok()`
- `crates/ferrotick-agent/src/metadata.rs:293` `trace_id_accepts_valid_format` -> `.is_ok()`
- `crates/ferrotick-agent/src/metadata.rs:346` `metadata_converts_to_envelope_meta` -> `.is_ok()`
- `crates/ferrotick-agent/src/schema_registry.rs:471` `registry_gets_schema_by_full_name` -> `.is_ok()`
- `crates/ferrotick-agent/src/schema_registry.rs:481` `registry_gets_schema_by_alias` -> `.is_ok()`
- `crates/ferrotick-agent/src/schema_registry.rs:525` `validate_accepts_valid_envelope` -> `.is_ok()`
- `crates/ferrotick-agent/src/schema_registry.rs:618` `validate_stream_event` -> `.is_ok()`
- `crates/ferrotick-agent/src/schema_registry.rs:641` `validate_stream_event_error_requires_error_field` -> `.is_ok()`
- `crates/ferrotick-ai/src/validation/sanitizer.rs:112` `test_validate_json_structure` -> `.is_ok()`
- `crates/ferrotick-core/src/cache.rs:187` `test_cache_expiration` -> `.is_some()`
- `crates/ferrotick-core/src/cache.rs:207` `test_cache_ttl_override` -> `.is_some()`
- `crates/ferrotick-core/src/throttling.rs:130` `buffers_when_rate_limit_is_exceeded` -> `.is_ok()`
- `crates/ferrotick-core/src/throttling.rs:131` `buffers_when_rate_limit_is_exceeded` -> `.is_ok()`
- `crates/ferrotick-ml/tests/behavioral_learning.rs:264` `test_models_handle_edge_cases` -> `.is_ok()`
- `crates/ferrotick-ml/tests/behavioral_learning.rs:269` `test_models_handle_edge_cases` -> `.is_ok()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:87` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:88` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:89` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:90` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:91` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-ml/tests/phase7_feature_pipeline.rs:92` `computes_required_phase7_features` -> `.is_some()`
- `crates/ferrotick-strategies/tests/behavioral_signals.rs:127` `test_ma_crossover_requires_warmup` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:24` `test_ma_crossover_construction` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:56` `test_ma_crossover_warmup` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:107` `test_rsi_construction` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:139` `test_rsi_warmup` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:168` `test_macd_construction` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:199` `test_macd_warmup` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:211` `test_bb_squeeze_construction` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:240` `test_bb_squeeze_warmup` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:272` `test_dsl_parse_valid_yaml` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:300` `test_dsl_parse_range_value` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:323` `test_dsl_parse_with_optional_fields` -> `.is_ok()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:447` `test_rsi_memory_bounds` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:463` `test_macd_memory_bounds` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:479` `test_bb_squeeze_memory_bounds` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:560` `test_composite_majority` -> `.is_some()`
- `crates/ferrotick-strategies/tests/strategies_test.rs:591` `test_composite_weighted_uses_strategy_name` -> `.is_some()`
- `tests/contract/provider_contract.rs:111` `quote_returns_valid_structure_for_all_providers` -> `.is_some()`
- `tests/contract/provider_contract.rs:112` `quote_returns_valid_structure_for_all_providers` -> `.is_some()`
- `tests/contract/provider_contract.rs:157` `unsupported_endpoints_return_expected_error` -> `.is_ok()`
- `tests/contract/provider_contract.rs:174` `unsupported_endpoints_return_expected_error` -> `.is_ok()`
- `tests/contract/provider_contract.rs:216` `canonical_output_parity_across_providers` -> `.is_some()`
- `tests/contract/provider_contract.rs:217` `canonical_output_parity_across_providers` -> `.is_some()`
- `tests/contract/provider_contract.rs:218` `canonical_output_parity_across_providers` -> `.is_some()`
- `tests/contract/provider_contract.rs:228` `canonical_output_parity_across_providers` -> `.is_some()`
- `tests/data_provider_behavior.rs:389` `when_all_sources_fail_system_returns_comprehensive_error` -> `.is_some()`
- `tests/edge_cases.rs:90` `test_extreme_prices` -> `.is_ok()`
- `tests/edge_cases.rs:96` `test_extreme_prices` -> `.is_ok()`
- `tests/error_handling_security.rs:174` `when_all_providers_fail_user_sees_comprehensive_error_list` -> `.is_some()`
- `tests/error_handling_security.rs:615` `when_routing_fails_each_error_identifies_its_source` -> `.is_some()`
- `tests/integration_test.rs:39` `test_cross_crate_compilation` -> `.is_ok()`

## 2) Tests That Do Not Verify Actual Behavior
### A. Tests with no runtime assertions and no deterministic expected outcome
- `crates/ferrotick-backtest/tests/vectorized_test.rs:78` `test_vectorized_backtest_creation`
- `crates/ferrotick-backtest/tests/vectorized_test.rs:85` `test_load_bars`
- `tests/warehouse_behavior.rs:595` `when_query_exceeds_timeout_it_is_cancelled`

### B. Tests that rely only on weak assertions (`is_ok`/`is_some`)
- `crates/ferrotick-agent/src/envelope.rs:490` `validator_accepts_valid_envelope`
- `crates/ferrotick-agent/src/metadata.rs:264` `request_id_parses_valid_uuid`
- `crates/ferrotick-agent/src/metadata.rs:291` `trace_id_accepts_valid_format`
- `crates/ferrotick-agent/src/metadata.rs:335` `metadata_converts_to_envelope_meta`
- `crates/ferrotick-agent/src/schema_registry.rs:465` `registry_gets_schema_by_full_name`
- `crates/ferrotick-agent/src/schema_registry.rs:475` `registry_gets_schema_by_alias`
- `crates/ferrotick-agent/src/schema_registry.rs:505` `validate_accepts_valid_envelope`
- `crates/ferrotick-agent/src/schema_registry.rs:603` `validate_stream_event`
- `crates/ferrotick-agent/src/schema_registry.rs:622` `validate_stream_event_error_requires_error_field`
- `crates/ferrotick-ml/tests/behavioral_learning.rs:254` `test_models_handle_edge_cases`
- `crates/ferrotick-strategies/tests/strategies_test.rs:278` `test_dsl_parse_range_value`
- `crates/ferrotick-strategies/tests/strategies_test.rs:304` `test_dsl_parse_with_optional_fields`
- `crates/ferrotick-strategies/tests/strategies_test.rs:435` `test_rsi_memory_bounds`
- `crates/ferrotick-strategies/tests/strategies_test.rs:451` `test_macd_memory_bounds`
- `crates/ferrotick-strategies/tests/strategies_test.rs:467` `test_bb_squeeze_memory_bounds`
- `crates/ferrotick-strategies/tests/strategies_test.rs:566` `test_composite_weighted_uses_strategy_name`

### C. Conditionally skipped tests that can pass without executing validation logic
- The following tests return early (`None => return`) when schema fixtures are unavailable, so CI can show pass without validation coverage:
- `crates/ferrotick-agent/src/schema_registry.rs:452` `registry_lists_schemas`
- `crates/ferrotick-agent/src/schema_registry.rs:465` `registry_gets_schema_by_full_name`
- `crates/ferrotick-agent/src/schema_registry.rs:475` `registry_gets_schema_by_alias`
- `crates/ferrotick-agent/src/schema_registry.rs:485` `registry_returns_error_for_missing_schema`
- `crates/ferrotick-agent/src/schema_registry.rs:495` `schema_has_valid_json`
- `crates/ferrotick-agent/src/schema_registry.rs:505` `validate_accepts_valid_envelope`
- `crates/ferrotick-agent/src/schema_registry.rs:529` `validate_rejects_missing_required_field`
- `crates/ferrotick-agent/src/schema_registry.rs:555` `validate_rejects_wrong_type`
- `crates/ferrotick-agent/src/schema_registry.rs:579` `validate_rejects_empty_source_chain`
- `crates/ferrotick-agent/src/schema_registry.rs:603` `validate_stream_event`
- `crates/ferrotick-agent/src/schema_registry.rs:622` `validate_stream_event_error_requires_error_field`

### D. Ignored tests (`#[ignore]`) reduce active verification
- **27 tests** are ignored and do not run in default `cargo test` runs.
- High-impact ignored suites are concentrated in:
- `tests/cli_user_journeys.rs` (end-to-end user flows)
- `tests/data_provider_behavior.rs` and `tests/contract/provider_contract.rs` (provider behavior/contract parity)
- `tests/error_handling_security.rs` (fallback/error handling under provider failures)

### Notes on assert-less tests that are still valid
- `crates/ferrotick-optimization/src/walk_forward.rs:test_validator_invalid_percentages` is valid due to `#[should_panic]`.
- `crates/ferrotick-strategies/tests/strategies_test.rs:test_strategy_is_send_sync` is a compile-time trait-bound check (no runtime assert required).

## 3) Critical Functionality Lacking Tests
### Critical coverage gaps (prioritized)
- **Web API behavior is largely untested**: only one web test exists (`/health`). No route/handler tests for backtest and strategy endpoints, request validation, or `WebError` response mapping.
  - Relevant files: `crates/ferrotick-web/src/routes/backtest.rs`, `crates/ferrotick-web/src/routes/strategies.rs`, `crates/ferrotick-web/src/error.rs`
- **Trading execution paths are minimally tested**: only two tests in `ferrotick-trading`, both construction/smoke-level; no behavioral tests for paper trading execution loop, position lifecycle, or broker failure handling.
  - Relevant files: `crates/ferrotick-trading/src/paper/engine.rs`, `crates/ferrotick-trading/src/brokers/alpaca.rs`, `crates/ferrotick-trading/src/brokers/ib.rs`, `crates/ferrotick-trading/src/executor/live.rs`
- **AI pipeline core logic lacks tests**: test coverage is limited to `validation/sanitizer`; no tests for strategy compilation, OpenAI client behavior, or backtest report generation.
  - Relevant files: `crates/ferrotick-ai/src/compiler/strategy.rs`, `crates/ferrotick-ai/src/llm/openai.rs`, `crates/ferrotick-ai/src/reporting/backtest.rs`
- **Backtest risk/drawdown and event execution internals have no unit tests** despite containing core financial math and execution behavior.
  - Relevant files: `crates/ferrotick-backtest/src/metrics/risk.rs`, `crates/ferrotick-backtest/src/metrics/drawdown.rs`, `crates/ferrotick-backtest/src/engine/event_driven.rs`, `crates/ferrotick-backtest/src/engine/executor.rs`
- **CLI command handlers are mostly untested**: CLI tests focus on parser/metadata; command implementations for quote/bars/search/sql/fundamentals/etc. have little direct unit coverage.
  - Relevant files: `crates/ferrotick-cli/src/commands/*.rs`

### Crate-level test density snapshot
| Crate | src files | tests/ files | test functions | Observed risk |
| --- | ---: | ---: | ---: | --- |
| `ferrotick-agent` | 5 | 0 | 43 | Lower |
| `ferrotick-ai` | 11 | 0 | 4 | High |
| `ferrotick-backtest` | 19 | 2 | 15 | Lower |
| `ferrotick-cli` | 22 | 0 | 5 | High |
| `ferrotick-core` | 27 | 0 | 43 | Lower |
| `ferrotick-ml` | 19 | 6 | 24 | Lower |
| `ferrotick-optimization` | 5 | 1 | 13 | Medium |
| `ferrotick-strategies` | 17 | 2 | 43 | Lower |
| `ferrotick-trading` | 10 | 1 | 2 | High |
| `ferrotick-warehouse` | 4 | 0 | 7 | Medium |
| `ferrotick-web` | 11 | 1 | 1 | High |
| `root tests/` | n/a | 13 | 123 | Mixed (many ignored integration tests) |

## 4) Recommended Remediation Order
1. Replace weak-only assertions with value/state assertions in the 16 tests listed in section 2B.
2. Fix or rewrite the 3 behaviorless tests in section 2A to assert deterministic outcomes.
3. Remove `None => return` skip logic in schema registry tests by ensuring fixtures are always available in test setup.
4. Convert high-value `#[ignore]` tests to deterministic mock-backed tests that run in CI by default.
5. Add first-pass unit suites for `ferrotick-web`, `ferrotick-trading`, `ferrotick-ai`, and backtest risk/event modules.

