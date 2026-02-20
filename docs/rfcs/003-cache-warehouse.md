# RFC-003: Cache and Warehouse Layout

- Status: Accepted
- Authors: Platform + Data Engineering
- Created: 2026-02-16
- Phase: 0

## Context

The CLI must provide low-latency local reads, reproducible analytics, and reliable cache provenance. A deterministic storage layout is required before provider ingest code lands.

## Decision

Adopt a local filesystem cache rooted at `~/.ferrotick/` (override via `FERROTICK_HOME`) with:
- partitioned Parquet objects,
- manifest metadata,
- DuckDB warehouse file.

## Directory Contract

```text
$FERROTICK_HOME/
  cache/
    parquet/
      source={source}/
        dataset={dataset}/
          symbol={symbol}/
            date={yyyy-mm-dd}/
              part-*.parquet
    warehouse.duckdb
```

## Manifest Contract

Each partition write updates a manifest row with:
- `source`
- `dataset`
- `symbol`
- `partition_date`
- `path`
- `row_count`
- `min_ts`
- `max_ts`
- `checksum`
- `updated_at`

## Retention and TTL Baseline

- `quote`: 5s
- `bars_1m`: 60s
- `bars_1d`: 24h
- `fundamentals`: 24h
- `corporate_actions`: 24h

## Write Safety Requirements

- Use temp-file write then atomic rename.
- Persist checksums in manifest for corruption detection.
- Use fine-grained lock scope keyed by `(source, dataset, symbol, partition)`.

## Warehouse Requirements

`warehouse.duckdb` stores canonical tables and views only.
- SQL endpoint is read-only by default.
- Canonical tables avoid provider-specific columns.

## Phase 0-1 Implementation Scope

Phase 0-1 freezes only the layout and retention contract. Storage engine implementation begins in Phase 3.

## Consequences

Positive:
- Clear migration path from fetch to analytics.
- Better debuggability and reproducibility.

Tradeoffs:
- Manifest consistency logic adds write-path overhead.
- TTL policy tuning may require environment-specific adjustments.

## Follow-up

- Implement cache manifest and atomic writers in Phase 3.
- Add integrity and concurrent write tests in Phase 3.
