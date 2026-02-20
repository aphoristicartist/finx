# ferrotick

Provider-neutral financial data CLI and core contracts implemented in Rust.

## Project Status

**v1.0.0 Released** - All phases complete. Production-ready financial data CLI.

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | RFCs and Contract Freeze | ✅ Complete |
| Phase 1 | CLI Core and Domain Contracts | ✅ Complete |
| Phase 2 | Source Adapters (Yahoo + Polygon) | ✅ Complete |
| Phase 3 | Local Cache and Parquet Storage | ✅ Complete |
| Phase 4 | DuckDB Warehouse and Analytics | ✅ Complete |
| Phase 5 | Alpha Vantage + Alpaca Adapters | ✅ Complete |
| Phase 6 | AI-Agent UX and Streaming | ✅ Complete |
| Phase 7 | Performance Hardening and Release | ✅ Complete |

## Workspace Layout

- `crates/ferrotick-core`: canonical domain types, envelope, adapters, routing.
- `crates/ferrotick-cli`: `ferrotick` command-line interface.
- `crates/ferrotick-warehouse`: DuckDB integration, migrations, analytics views.
- `schemas/v1`: versioned JSON schemas for machine-readable output.
- `docs`: roadmap and RFCs.

## Build

```bash
cargo build
```

## Test

```bash
cargo test
```

## Run CLI

```bash
cargo run -p ferrotick-cli -- quote AAPL
cargo run -p ferrotick-cli -- bars AAPL --interval 1d --limit 5
cargo run -p ferrotick-cli -- search apple --limit 5
cargo run -p ferrotick-cli -- schema list
cargo run -p ferrotick-cli -- sources
cargo run -p ferrotick-cli -- warehouse sync --symbol AAPL --start 2024-01-01 --end 2024-12-31
```

## Output and Exit Codes

- Envelope output includes metadata (`request_id`, `source_chain`, `latency_ms`, `cache_hit`) plus `data` and `errors`.
- `--strict` returns exit code `5` when warnings/errors are present.
- Exit code contract:
  - `0`: success
  - `2`: validation/command input error
  - `3`: provider/network failure with emitted envelope errors
  - `4`: serialization/schema contract failure
  - `5`: partial result (strict mode)
  - `10`: internal I/O/runtime error

## Source Adapters

`ferrotick-core` uses an async adapter contract (`DataSource`) implemented via boxed futures. The router supports:

- `auto`: scored source selection + fallback
- `strict`: single source without fallback
- `priority`: ordered source chain

Implemented adapters:

- `PolygonAdapter`
- `YahooAdapter`
- `AlphaVantageAdapter`
- `AlpacaAdapter`

Adapters include:

- auth-capable HTTP transport abstraction (`HttpClient`, `HttpAuth`)
- circuit breaker protection (`CircuitBreaker`)
- rate limiting via `governor`
- deterministic normalization into canonical models

## Capability Matrix

| Provider | Quote | Bars | Fundamentals | Search | Score |
| --- | --- | --- | --- | --- | --- |
| Polygon | Yes | Yes | Yes | Yes | 90 |
| Alpaca | Yes | Yes | No | No | 85 |
| Yahoo | Yes | Yes | Yes | Yes | 78 |
| Alpha Vantage | Yes | Yes | Yes | Yes | 70 |

## HTTP Auth Configuration

`PolygonAdapter::default()` reads `FERROTICK_POLYGON_API_KEY` for `x-api-key` auth (falls back to `demo`).
`AlphaVantageAdapter::default()` reads `FERROTICK_ALPHAVANTAGE_API_KEY` and appends `apikey` query auth (falls back to `demo`).
`AlpacaAdapter::default()` reads `FERROTICK_ALPACA_API_KEY` and `FERROTICK_ALPHAVANTAGE_SECRET_KEY` for dual header auth (`APCA-API-KEY-ID`, `APCA-API-SECRET-KEY`, both fallback to `demo`).

Adapters can be explicitly configured in code:

```rust
use std::sync::Arc;
use ferrotick_core::{HttpAuth, NoopHttpClient, PolygonAdapter};

let adapter = PolygonAdapter::with_http_client(
    Arc::new(NoopHttpClient),
    HttpAuth::Header {
        name: "x-api-key".to_string(),
        value: "my-key".to_string(),
    },
);
```

## Circuit Breaker

- Opens after consecutive transport/upstream failures.
- Open state blocks new upstream calls until timeout expires.
- Half-open state probes recovery.
- Health output reflects breaker status:
  - `open` => `unhealthy` and rate unavailable
  - `half-open` => degraded if otherwise healthy

## Warehouse (DuckDB)

The `warehouse sync` command fetches historical bars and stores them in DuckDB for analytics:

```bash
# Sync 1 year of AAPL daily bars
cargo run -p ferrotick-cli -- warehouse sync --symbol AAPL --start 2024-01-01 --end 2024-12-31

# Query via DuckDB
duckdb ~/.local/share/ferrotick/warehouse.duckdb "SELECT * FROM bars WHERE symbol='AAPL' LIMIT 10"
```

### Available Views

- `v_daily_bars`: Daily OHLCV data
- `v_quote_history`: Historical quote snapshots
- `v_fundamentals`: Company fundamentals

## AI-Agent Streaming

Enable NDJSON streaming for AI agent consumption:

```bash
cargo run -p ferrotick-cli -- quote AAPL --stream
```

Stream events follow `schemas/v1/stream.event.schema.json`:
- `start`: Operation initiated
- `progress`: Incremental updates
- `chunk`: Data batch delivered
- `end`: Operation completed
- `error`: Error occurred

## Security Notes

- `schema get` path handling is constrained to files under `schemas/v1` with canonical path checks to prevent traversal.
- API keys are read from environment variables, never logged.
- All HTTP requests use TLS via `rustls`.

## Documentation

- [Roadmap](docs/ROADMAP.md) - Full project roadmap and technical spec
- [RFCs](docs/rfcs/) - Design documents
- [Performance Guide](docs/PERFORMANCE.md) - Benchmarks and optimization

## License

MIT
