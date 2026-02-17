# finx

Provider-neutral financial data CLI and core contracts implemented in Rust.

## Workspace Layout

- `crates/finx-core`: canonical domain types, envelope, adapters, routing.
- `crates/finx-cli`: `finx` command-line interface.
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
cargo run -p finx-cli -- quote AAPL
cargo run -p finx-cli -- bars AAPL --interval 1d --limit 5
cargo run -p finx-cli -- search apple --limit 5
cargo run -p finx-cli -- schema list
```

## Output and Exit Codes

- Envelope output includes metadata (`request_id`, `source_chain`, `latency_ms`, `cache_hit`) plus `data` and `errors`.
- `--strict` returns exit code `5` when warnings/errors are present.
- Exit code contract:
  - `0`: success
  - `2`: validation/command input error
  - `3`: provider/network failure with emitted envelope errors
  - `4`: serialization/schema contract failure
  - `10`: internal I/O/runtime error

## Source Adapters

`finx-core` uses an async adapter contract (`DataSource`) implemented via boxed futures. The router supports:

- `auto`: scored source selection + fallback
- `strict`: single source without fallback
- `priority`: ordered source chain

Implemented adapters:

- `PolygonAdapter`
- `YahooAdapter`

Both adapters include:

- auth-capable HTTP transport abstraction (`HttpClient`, `HttpAuth`)
- circuit breaker protection (`CircuitBreaker`)
- deterministic normalization into canonical models

## HTTP Auth Configuration

`PolygonAdapter::default()` reads `FINX_POLYGON_API_KEY` for `x-api-key` auth (falls back to `demo`).

Adapters can be explicitly configured in code:

```rust
use std::sync::Arc;
use finx_core::{HttpAuth, NoopHttpClient, PolygonAdapter};

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

## Security Notes

- `schema get` path handling is constrained to files under `schemas/v1` with canonical path checks to prevent traversal.

