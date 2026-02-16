# Rust Financial Data CLI: Consensus Roadmap and Technical Spec

## 1. Scope and Product Definition

### 1.1 Product Goal
Build a production-grade Rust CLI (`finx`) for market and fundamentals data that:
- Aggregates Yahoo, Polygon, Alpha Vantage, and Alpaca behind one normalized API.
- Caches data locally in Parquet and exposes DuckDB analytics views.
- Starts in under 100ms for local/cached commands.
- Is first-class for AI agents (strict JSON schemas, metadata, and streaming events).

### 1.2 Success Criteria (Improvement Over `yfinance`)
`finx` must exceed `yfinance` on:
- Provider resilience: source routing and fallback, not single-provider dependence.
- Contract stability: versioned JSON schemas and deterministic machine-readable output.
- Local analytics: SQL-first workflow over cached data with DuckDB views.
- Performance: Rust binary startup and lower memory overhead.
- Observability: request IDs, per-source latency, and cache provenance in all responses.

### 1.3 Non-Goals (Initial Release)
- Real-time sub-second websocket market data.
- Order execution/trading operations.
- Full options greeks model parity with dedicated quant platforms.

## 2. Architecture (Target State)

### 2.1 High-Level Components
1. `cli`: command parsing, output formatting, exit codes.
2. `core`: domain models, validation, provider-neutral service layer.
3. `sources/*`: provider adapters (Yahoo, Polygon, Alpha Vantage, Alpaca).
4. `cache`: local object store manager (Parquet), manifest index, TTL policy.
5. `warehouse`: DuckDB integration, table sync, analytics views, SQL endpoint.
6. `agent`: NDJSON streaming protocol and schema registry for AI use.
7. `telemetry`: tracing, metrics, profiling hooks.

### 2.2 Data Flow
1. Command enters `cli`.
2. `core` resolves requested dataset + source strategy (`auto` or fixed provider).
3. `cache` checks TTL policy and partition manifest.
4. On hit: return cached canonical model.
5. On miss: adapter fetches raw payload, `core` normalizes schema, writes Parquet, updates DuckDB.
6. Response emits JSON envelope with metadata (`source`, `latency_ms`, `cache_hit`, `schema_version`).

## 3. Phase-by-Phase Roadmap (Exact Tasks)

## Phase 0: Consensus, RFCs, and Contract Freeze (Week 1)
### Tasks
1. Publish RFC-001 for canonical data model (`Instrument`, `Quote`, `Bar`, `Fundamental`, `CorporateAction`).
2. Publish RFC-002 for source strategy (`auto`, `priority list`, `strict source`).
3. Publish RFC-003 for cache + DuckDB layout and retention policy.
4. Publish RFC-004 for JSON envelope and streaming protocol.
5. Define release criteria and SLO baselines.
6. Create initial schema files under `schemas/v1/`.

### Deliverables
- Signed RFC set (`docs/rfcs/*.md`).
- Baseline schema docs.
- Acceptance checklist for Phase 1.

### Consensus Gate
- Engineering + data + platform sign-off on RFC-001..004.

## Phase 1: CLI Core and Domain Contracts (Weeks 2-3)
### Tasks
1. Create Cargo workspace and crate boundaries.
2. Implement typed CLI (`clap`) commands:
   - `quote`, `bars`, `fundamentals`, `search`, `sql`, `schema`, `sources`.
3. Implement core domain types and validation rules:
   - Symbol grammar, interval enum, timestamp handling in UTC.
4. Implement response envelope + structured errors.
5. Implement output modes:
   - `--format table|json|ndjson`
   - `--pretty` (human), `--strict` (machine fail on warning).
6. Add JSON schema validation tests using golden fixtures.

### Deliverables
- Executable CLI with stub provider.
- Versioned schema registry.
- Stable exit code contract.

### Acceptance Criteria
- `finx --help` p50 startup < 100ms on target machine.
- Schema tests passing for all command responses.

## Phase 2: Source Adapters (Yahoo + Polygon First) (Weeks 4-6)
### Tasks
1. Implement `DataSource` trait and adapter registry.
2. Build Yahoo adapter:
   - Quote, historical bars, basic fundamentals.
   - Cookie/session handling and retry policy.
3. Build Polygon adapter:
   - Quote, aggregates, ticker metadata.
   - API key auth and rate limit handling.
4. Implement normalization pipeline:
   - Source-specific payload -> canonical structs.
5. Implement source health checks and circuit breaker.
6. Add deterministic source selection policy:
   - `auto` uses provider score + endpoint support + rate availability.
7. Build contract tests with recorded fixtures (VCR-style).

### Deliverables
- Working dual-source fetch with fallback.
- Provider capability matrix.

### Acceptance Criteria
- `auto` fallback success rate > 99% for supported endpoints in integration tests.
- Canonical outputs identical shape regardless of source.

## Phase 3: Local Cache (Parquet + Manifest) (Weeks 7-8)
### Tasks
1. Implement cache root config:
   - Default `~/.finx/`, override with `FINX_HOME`.
2. Define partitioned Parquet path strategy:
   - `cache/parquet/source={source}/dataset={dataset}/symbol={symbol}/date={yyyy-mm-dd}/part-*.parquet`
3. Implement write-on-fetch with atomic temp file + rename.
4. Implement TTL strategy per dataset:
   - `quote`: 5s
   - `bars_1m`: 60s
   - `bars_1d`: 24h
   - `fundamentals`: 24h
   - `corporate_actions`: 24h
5. Implement cache manifest table:
   - min/max timestamp, row count, checksum, updated_at.
6. Build cache commands:
   - `cache status`, `cache prune`, `cache warm`.
7. Add corrupted file detection and auto-repair path.

### Deliverables
- Production cache manager with retention and pruning.

### Acceptance Criteria
- Cached quote retrieval p50 < 40ms.
- Cache integrity checks pass under concurrent fetch load.

## Phase 4: DuckDB Warehouse and Analytics Views (Weeks 9-10)
### Tasks
1. Add DuckDB file (`cache/warehouse.duckdb`) creation and migrations.
2. Register Parquet partitions into DuckDB metadata table.
3. Build canonical DuckDB tables:
   - `instruments`
   - `quotes_latest`
   - `bars_1m`
   - `bars_1d`
   - `fundamentals`
   - `corporate_actions`
   - `cache_manifest`
   - `ingest_log`
4. Build analytics views:
   - `vw_returns_daily`
   - `vw_volatility_20d`
   - `vw_gaps_open`
   - `vw_source_latency`
5. Implement `finx sql "<query>" --format json|table|ndjson`.
6. Add query guardrails:
   - read-only mode by default
   - max rows and timeout controls.

### Deliverables
- Local SQL analytics surface over cache.

### Acceptance Criteria
- 1M-row local aggregate query p50 < 150ms.
- DuckDB sync job idempotent and crash-safe.

## Phase 5: Remaining Providers (Alpha Vantage + Alpaca) (Weeks 11-12)
### Tasks
1. Implement Alpha Vantage adapter with throttling-aware queue.
2. Implement Alpaca market data adapter.
3. Extend capability matrix and routing score model.
4. Add per-provider policy:
   - max concurrency
   - quota windows
   - retry backoff parameters.
5. Ensure canonical parity tests across all four providers.

### Deliverables
- Full provider set (Yahoo, Polygon, Alpha Vantage, Alpaca).

### Acceptance Criteria
- All providers pass shared contract suite.
- Source router picks valid provider > 99.9% in simulation.

## Phase 6: AI-Agent UX and Streaming (Weeks 13-14)
### Tasks
1. Implement strict JSON envelope everywhere.
2. Implement NDJSON event stream mode (`--stream`):
   - `start`, `progress`, `chunk`, `end`, `error`.
3. Implement schema introspection commands:
   - `finx schema list`
   - `finx schema get <name>`
4. Add machine metadata:
   - `request_id`, `trace_id`, `source_chain`, `latency_ms`, `cache_hit`, `warnings`.
5. Add deterministic ordering and stable numeric formatting.
6. Add `--explain` mode for query/source-plan diagnostics.

### Deliverables
- AI-ready command protocol with streaming.

### Acceptance Criteria
- 100% commands emit valid schema-compliant JSON in strict mode.
- Streaming consumers can parse 100k events with zero malformed lines.

## Phase 7: Performance Hardening and Release (Weeks 15-16)
### Tasks
1. Profile startup path and remove lazy init bottlenecks.
2. Add feature flags to minimize binary size by default.
3. Optimize JSON parsing with selective SIMD path (`simd-json` feature).
4. Tune HTTP connection pooling and DNS caching.
5. Add criterion benchmarks and regression thresholds in CI.
6. Build release automation:
   - cross-compilation
   - checksums + SBOM
   - signed binaries.

### Deliverables
- Release candidate with benchmark evidence.

### Acceptance Criteria
- Startup p50 < 100ms, p95 < 140ms.
- Cached commands p95 < 80ms.
- No benchmark regression > 10% without explicit approval.

## 4. Technical Specs by Component

## 4.1 CLI Spec
- Binary: `finx`
- Global flags:
  - `--format table|json|ndjson`
  - `--strict`
  - `--source auto|yahoo|polygon|alphavantage|alpaca`
  - `--timeout-ms <n>`
  - `--profile`
  - `--stream`
- Exit codes:
  - `0` success
  - `2` validation error
  - `3` provider/network error
  - `4` schema violation
  - `5` partial result (strict mode treats as failure)
  - `10` internal error

## 4.2 Source Adapter Contract (Rust Trait)
```rust
#[async_trait::async_trait]
pub trait DataSource: Send + Sync {
    fn id(&self) -> SourceId;
    fn capabilities(&self) -> CapabilitySet;
    async fn quote(&self, req: QuoteRequest) -> Result<QuoteBatch, SourceError>;
    async fn bars(&self, req: BarsRequest) -> Result<BarBatch, SourceError>;
    async fn fundamentals(&self, req: FundamentalsRequest) -> Result<FundamentalsBatch, SourceError>;
    async fn search(&self, req: SearchRequest) -> Result<SearchBatch, SourceError>;
    async fn health(&self) -> HealthStatus;
}
```

## 4.3 Canonical Data Model Constraints
- All timestamps are RFC3339 UTC strings in JSON; `TIMESTAMP` in DuckDB.
- Decimal fields serialized as strings when precision > 15 digits.
- Symbols normalized to uppercase; original preserved in metadata.
- Missing numeric values use `null`, never `NaN` or sentinel numbers.

## 4.4 Cache Spec
- Cache root: `~/.finx/` (override `FINX_HOME`).
- Atomic writes: temp file + fsync + rename.
- Locking: per `(source,dataset,symbol,partition)` file lock.
- Manifest row schema:
  - `source TEXT`
  - `dataset TEXT`
  - `symbol TEXT`
  - `partition_date DATE`
  - `path TEXT`
  - `row_count BIGINT`
  - `min_ts TIMESTAMP`
  - `max_ts TIMESTAMP`
  - `checksum TEXT`
  - `updated_at TIMESTAMP`

## 4.5 DuckDB Spec
- Database file: `cache/warehouse.duckdb`.
- Use read-only by default for `sql`; write mode only with `--write`.
- Views must depend only on canonical tables (not provider-specific columns).
- `sql` command limits:
  - default row cap `10_000`
  - default timeout `5s`
  - override with `--max-rows` and `--query-timeout-ms`.

## 5. API Contracts (JSON Schemas)

All schemas use JSON Schema draft 2020-12 with semantic versioning under `schemas/v1/`.

## 5.1 Envelope (`schemas/v1/envelope.schema.json`)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://finx.dev/schemas/v1/envelope.schema.json",
  "title": "FinxEnvelope",
  "type": "object",
  "required": ["meta", "data"],
  "properties": {
    "meta": {
      "type": "object",
      "required": ["request_id", "schema_version", "generated_at", "source_chain", "latency_ms", "cache_hit"],
      "properties": {
        "request_id": { "type": "string", "minLength": 8 },
        "trace_id": { "type": "string" },
        "schema_version": { "type": "string", "pattern": "^v[0-9]+\\.[0-9]+\\.[0-9]+$" },
        "generated_at": { "type": "string", "format": "date-time" },
        "source_chain": {
          "type": "array",
          "items": { "type": "string", "enum": ["yahoo", "polygon", "alphavantage", "alpaca"] },
          "minItems": 1
        },
        "latency_ms": { "type": "integer", "minimum": 0 },
        "cache_hit": { "type": "boolean" },
        "warnings": {
          "type": "array",
          "items": { "type": "string" }
        }
      },
      "additionalProperties": false
    },
    "data": {},
    "errors": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["code", "message"],
        "properties": {
          "code": { "type": "string" },
          "message": { "type": "string" },
          "retryable": { "type": "boolean" },
          "source": { "type": "string" }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
```

## 5.2 Quote Response (`schemas/v1/quote.response.schema.json`)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://finx.dev/schemas/v1/quote.response.schema.json",
  "allOf": [
    { "$ref": "envelope.schema.json" },
    {
      "type": "object",
      "properties": {
        "data": {
          "type": "object",
          "required": ["quotes"],
          "properties": {
            "quotes": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["symbol", "price", "currency", "as_of"],
                "properties": {
                  "symbol": { "type": "string" },
                  "price": { "type": ["number", "string"] },
                  "bid": { "type": ["number", "string", "null"] },
                  "ask": { "type": ["number", "string", "null"] },
                  "volume": { "type": ["integer", "null"] },
                  "currency": { "type": "string", "minLength": 3, "maxLength": 3 },
                  "as_of": { "type": "string", "format": "date-time" }
                },
                "additionalProperties": false
              }
            }
          },
          "additionalProperties": false
        }
      }
    }
  ]
}
```

## 5.3 Bars Response (`schemas/v1/bars.response.schema.json`)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://finx.dev/schemas/v1/bars.response.schema.json",
  "allOf": [
    { "$ref": "envelope.schema.json" },
    {
      "type": "object",
      "properties": {
        "data": {
          "type": "object",
          "required": ["symbol", "interval", "bars"],
          "properties": {
            "symbol": { "type": "string" },
            "interval": { "type": "string", "enum": ["1m", "5m", "15m", "1h", "1d"] },
            "bars": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["ts", "open", "high", "low", "close", "volume"],
                "properties": {
                  "ts": { "type": "string", "format": "date-time" },
                  "open": { "type": ["number", "string"] },
                  "high": { "type": ["number", "string"] },
                  "low": { "type": ["number", "string"] },
                  "close": { "type": ["number", "string"] },
                  "volume": { "type": ["integer", "null"] },
                  "vwap": { "type": ["number", "string", "null"] }
                },
                "additionalProperties": false
              }
            }
          },
          "additionalProperties": false
        }
      }
    }
  ]
}
```

## 5.4 SQL Response (`schemas/v1/sql.response.schema.json`)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://finx.dev/schemas/v1/sql.response.schema.json",
  "allOf": [
    { "$ref": "envelope.schema.json" },
    {
      "type": "object",
      "properties": {
        "data": {
          "type": "object",
          "required": ["columns", "rows", "row_count"],
          "properties": {
            "columns": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["name", "type"],
                "properties": {
                  "name": { "type": "string" },
                  "type": { "type": "string" }
                },
                "additionalProperties": false
              }
            },
            "rows": {
              "type": "array",
              "items": { "type": "array", "items": {} }
            },
            "row_count": { "type": "integer", "minimum": 0 },
            "truncated": { "type": "boolean" }
          },
          "additionalProperties": false
        }
      }
    }
  ]
}
```

## 5.5 Streaming Event (`schemas/v1/stream.event.schema.json`)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://finx.dev/schemas/v1/stream.event.schema.json",
  "type": "object",
  "required": ["event", "seq", "ts"],
  "properties": {
    "event": {
      "type": "string",
      "enum": ["start", "progress", "chunk", "end", "error"]
    },
    "seq": { "type": "integer", "minimum": 1 },
    "ts": { "type": "string", "format": "date-time" },
    "meta": { "type": "object" },
    "data": {},
    "error": {
      "type": "object",
      "required": ["code", "message"],
      "properties": {
        "code": { "type": "string" },
        "message": { "type": "string" },
        "retryable": { "type": "boolean" }
      },
      "additionalProperties": false
    }
  },
  "allOf": [
    {
      "if": { "properties": { "event": { "const": "error" } } },
      "then": { "required": ["error"] }
    }
  ],
  "additionalProperties": false
}
```

## 6. File and Module Structure

```text
finx/
  Cargo.toml
  rust-toolchain.toml
  crates/
    finx-cli/
      src/main.rs
      src/commands/
      src/output/
    finx-core/
      src/domain/
      src/service/
      src/schema/
    finx-sources/
      src/lib.rs
      src/router.rs
      src/common/http.rs
      src/yahoo/
      src/polygon/
      src/alphavantage/
      src/alpaca/
    finx-cache/
      src/parquet_store.rs
      src/manifest.rs
      src/ttl.rs
      src/prune.rs
    finx-warehouse/
      src/duckdb.rs
      src/migrations/
      src/views/
    finx-agent/
      src/stream.rs
      src/envelope.rs
      src/schema_registry.rs
    finx-telemetry/
      src/tracing.rs
      src/metrics.rs
  schemas/
    v1/
      envelope.schema.json
      quote.response.schema.json
      bars.response.schema.json
      fundamentals.response.schema.json
      sql.response.schema.json
      stream.event.schema.json
  tests/
    contract/
    integration/
    e2e/
    fixtures/
  benches/
    startup.rs
    quote_cached.rs
    bars_parse.rs
  docs/
    ROADMAP.md
    rfcs/
      001-canonical-model.md
      002-source-routing.md
      003-cache-warehouse.md
      004-agent-protocol.md
```

## 7. Performance Targets (SLOs)

### 7.1 CLI and Runtime
- `finx --help` startup:
  - p50 < 100ms
  - p95 < 140ms
- `finx quote AAPL --format json` (cache hit):
  - p50 < 40ms
  - p95 < 80ms
- Memory:
  - idle RSS < 35MB
  - simple quote command RSS < 80MB

### 7.2 Data and Query
- Parse + normalize 10k bars:
  - p50 < 20ms
- Local DuckDB query on 1M rows:
  - p50 < 150ms
  - p95 < 300ms
- Cache write throughput:
  - >= 50 MB/s sustained local disk.

### 7.3 Reliability
- Source routing fallback success > 99% for supported data requests.
- Zero corrupted manifest rows in crash-recovery test suite.

## 8. Testing Strategy

## 8.1 Test Layers
1. Unit tests:
   - Validation rules, symbol parsing, time interval mapping, error conversions.
2. Contract tests:
   - Every provider adapter must pass shared canonical output contracts.
3. Integration tests:
   - Fetch -> normalize -> cache -> DuckDB sync -> query end-to-end.
4. E2E CLI tests:
   - Exit codes, JSON schemas, strict mode behavior.
5. Performance tests:
   - `criterion` benchmarks for startup, cache-hit quote, bars parse, SQL query.
6. Soak tests:
   - 24-hour ingest simulation with rolling TTL pruning.
7. Fault injection:
   - HTTP 429/5xx, malformed payload, partial response, file lock contention.

## 8.2 Required CI Jobs
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -D warnings`
- `cargo test --workspace`
- `cargo test --test contract_*`
- `cargo bench --bench startup -- --save-baseline main`
- Schema validation job (fixtures validated against JSON schema).

## 8.3 Quality Gates
- No high-severity contract regressions.
- Benchmark regressions > 10% fail CI unless baseline explicitly updated.
- New provider endpoints require fixtures + contract tests before merge.

## 9. Dependencies

### 9.1 Runtime Crates
- `clap` (CLI parsing)
- `tokio` (async runtime)
- `reqwest` + `rustls` (HTTP client)
- `serde`, `serde_json` (serialization)
- `thiserror` (error typing)
- `tracing`, `tracing-subscriber` (telemetry)
- `uuid` (request IDs)
- `time` (timestamp handling)
- `async-trait` (adapter trait ergonomics)
- `governor` (rate limiting)
- `moka` (in-memory hot cache)
- `duckdb` (embedded DuckDB)
- `arrow-array`, `arrow-schema`, `parquet` (Parquet IO)
- `simd-json` (optional fast JSON parse feature)

### 9.2 Dev/Test Crates
- `criterion` (benchmarks)
- `insta` (snapshot testing for JSON outputs)
- `jsonschema` (schema contract validation)
- `wiremock` (HTTP mocking)
- `proptest` (property-based testing)
- `assert_cmd` + `predicates` (CLI E2E tests)
- `tempfile` (isolated cache/warehouse integration tests)

### 9.3 External Tools
- DuckDB CLI (optional local debugging)
- `hyperfine` (startup benchmarking)
- `cargo-nextest` (faster test execution)

## 10. Execution Order and Critical Path

1. Freeze schemas and canonical model first (Phase 0-1).
2. Deliver two-provider parity before expanding provider count (Phase 2).
3. Build cache and DuckDB before AI streaming enhancements (Phase 3-4 before Phase 6).
4. Add Alpha Vantage and Alpaca only after shared contracts are stable (Phase 5).
5. Final hardening and release only after performance SLOs pass in CI (Phase 7).

This sequence minimizes rework and ensures each added provider/dataset inherits a fixed contract and storage model.
