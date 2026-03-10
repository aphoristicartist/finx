# Performance Audit

Date: 2026-03-01
Scope: `crates/ferrotick-*` runtime paths for backtesting, optimization, ML feature pipeline, routing/adapters, and warehouse I/O.

## Executive Summary

- Multiple high-impact regressions are present in hot paths (routing/auth, warehouse query/ingest, optimization loops).
- The largest cost centers are unnecessary I/O, repeated full-data rematerialization, and avoidable allocations.
- Concurrency and vectorization capabilities are available in dependencies but mostly not used in production code paths.

## Findings (Ordered by Severity)

### Critical

1. Yahoo auth caching is effectively disabled.
- Evidence: `is_auth_valid` requires both `cookie` and `crumb` (`crates/ferrotick-core/src/adapters/yahoo.rs:57`), but refresh only stores `crumb` (`crates/ferrotick-core/src/adapters/yahoo.rs:158`) and never stores `cookie` after `fc.yahoo.com` fetch (`crates/ferrotick-core/src/adapters/yahoo.rs:121`-`127`).
- Impact: repeated auth refresh network calls for most Yahoo operations, increased latency, and faster rate-limit exhaustion.
- Fix: persist cookie state during refresh or remove cookie from validity gating if crumb-only is sufficient.

2. Warehouse SELECT path executes the same query twice.
- Evidence: first `statement.query(...)` result is discarded (`crates/ferrotick-warehouse/src/lib.rs:695`), then query is executed again for row iteration (`crates/ferrotick-warehouse/src/lib.rs:710`).
- Impact: ~2x query CPU/I/O for all SELECT workloads.
- Fix: execute once; derive metadata from prepared statement without a throwaway query.

### High

3. Optimization repeatedly rematerializes full backtest input per parameter combination/window.
- Evidence: `GridSearchOptimizer::optimize` rebuilds `Vec<BarEvent>` for each parameter set (`crates/ferrotick-optimization/src/grid_search.rs:119`-`138`) and clones config each iteration (`crates/ferrotick-optimization/src/grid_search.rs:141`).
- Evidence: walk-forward also rebuilds `Vec<BarEvent>` per window test (`crates/ferrotick-optimization/src/walk_forward.rs:187`-`203`).
- Impact: O(combinations * bars) allocation churn dominates CPU/memory for large sweeps.
- Fix: precompute immutable `BarEvent` buffers once per slice and reuse; avoid per-iteration symbol/timestamp cloning.

4. Vectorized backtest repeatedly scans and reallocates immutable data per parameter set.
- Evidence: parameter sweep is sequential (`crates/ferrotick-backtest/src/vectorized/engine.rs:87`-`90`).
- Evidence: each run re-queries full close series (`crates/ferrotick-backtest/src/vectorized/engine.rs:174`-`189`) and repeatedly converts arrays into new vectors/struct vectors (`crates/ferrotick-backtest/src/vectorized/engine.rs:247`-`268`).
- Evidence: `load_bars` inserts row-by-row without explicit transaction batching (`crates/ferrotick-backtest/src/vectorized/engine.rs:59`-`71`).
- Impact: avoidable DB I/O and allocation overhead across sweeps.
- Fix: cache price column once per loaded dataset, compute metrics directly on slices, batch inserts in one transaction (or DuckDB appender/COPY).

5. Warehouse ingest paths are row-at-a-time and do extra per-row work.
- Evidence: `ingest_quotes` performs 3 executes per row (`crates/ferrotick-warehouse/src/lib.rs:431`-`467`).
- Evidence: `ingest_bars` performs per-row SQL string formatting (`crates/ferrotick-warehouse/src/lib.rs:508`-`513`) plus 2 executes per row (`crates/ferrotick-warehouse/src/lib.rs:526`, `532`).
- Evidence: `ingest_fundamentals` performs 2 executes per row (`crates/ferrotick-warehouse/src/lib.rs:564`-`583`).
- Impact: high DB call overhead and reduced ingest throughput.
- Fix: prepare statements once outside loops; batch ingest_log inserts; prefer bulk ingest primitives.

6. Cache sync amplifies I/O by scanning parquet partitions multiple times.
- Evidence: file list is fully materialized before processing (`crates/ferrotick-warehouse/src/lib.rs:393`-`396`).
- Evidence: each partition executes count scan (`crates/ferrotick-warehouse/src/lib.rs:901`-`911`) and min/max probing that may run several failed queries (`crates/ferrotick-warehouse/src/lib.rs:926`-`940`).
- Impact: expensive for large caches; increased wall-clock and disk pressure.
- Fix: stream traversal instead of pre-collecting all files; compute row_count/min/max in one query after discovering the timestamp column once.

### Medium

7. Rolling-window feature functions are O(n*window), degrading to O(n^2) for large windows.
- Evidence: repeated summation/scans per index in `rolling_mean`, `rolling_std`, `rolling_min`, `rolling_max` (`crates/ferrotick-ml/src/features/windows.rs:7`-`10`, `20`-`31`, `43`-`50`, `61`-`68`).
- Impact: CPU hotspot on long time series.
- Fix: use prefix sums (mean/std) and monotonic deque (min/max).

8. Cross-validation copies full matrices each fold.
- Evidence: per-fold concatenation of "before + after" arrays (`crates/ferrotick-ml/src/training/evaluation.rs:141`-`144`, `157`-`160`) inside fold loop (`crates/ferrotick-ml/src/training/evaluation.rs:90`-`129`).
- Impact: high memory bandwidth and allocation overhead for large datasets.
- Fix: use index-based views/slices instead of materializing train/test arrays each fold.

9. Feature export performs many full passes and temporary vectors.
- Evidence: `export_features_parquet` builds each DataFrame column with independent `rows.iter().map(...).collect()` passes (`crates/ferrotick-ml/src/features/store.rs:276`-`346`).
- Impact: extra allocations and cache-miss-heavy iteration.
- Fix: one-pass column builders with preallocated vectors.

10. Event-driven backtest uses channel machinery and cloning in a tight local loop.
- Evidence: publish clones `BarEvent` (`crates/ferrotick-backtest/src/engine/event_driven.rs:178`), stores cloned symbol/bar (`crates/ferrotick-backtest/src/engine/event_driven.rs:216`), and uses `tokio::mpsc::unbounded_channel` for in-thread event flow (`crates/ferrotick-backtest/src/engine/event_driven.rs:307`-`330`).
- Impact: avoidable synchronization and allocation overhead under high event volume.
- Fix: replace with `VecDeque`-based local queue and borrow where possible.

11. Connection pooling is bypassed in ML store paths.
- Evidence: `FeatureStore` opens direct DuckDB connections (`crates/ferrotick-ml/src/features/store.rs:58`-`64`) and uses them for load/upsert/load_features (`crates/ferrotick-ml/src/features/store.rs:81`, `131`, `207`) instead of warehouse pool.
- Impact: repeated open/close overhead and reduced pooling benefits.
- Fix: restore pooled access for stable query paths and isolate only schema-mutation edge cases if needed.

### Low

12. Shared response cache is implemented but not integrated into runtime data flows.
- Evidence: `CacheStore`/`CacheMode` are defined in `crates/ferrotick-core/src/cache.rs` and re-exported (`crates/ferrotick-core/src/lib.rs:144`), with no production call sites outside cache module/tests.
- Impact: repeated upstream calls for identical requests; avoidable latency/cost.
- Fix: add cache keying + TTL policy in routing/adapters for quote/bars/fundamentals/search.

## Checklist Verification

### 1) Algorithms

- O(n^2) or worse detected: yes.
- Confirmed in rolling-window functions (`crates/ferrotick-ml/src/features/windows.rs`).

### 2) Unnecessary Allocations

- Confirmed in optimization rematerialization, feature export column construction, repeated conversions in vectorized backtest, and event cloning.

### 3) Inefficient Data Structures

- Channel-based event bus for local single-thread processing (`tokio::mpsc`) is heavier than needed.
- Repeated `HashMap<String, f64>` cloning for parameter combinations introduces avoidable overhead.

### 4) Missing Caching Opportunities

- Response cache not integrated (`CacheStore` unused in runtime).
- Yahoo auth cache validity bug causes repeated refresh.
- Immutable price series in parameter sweeps not cached between runs.

### 5) Memory Usage Patterns

- High churn from repeated `Vec`/`Array` copies in optimization and cross-validation loops.
- Multiple full-pass collections in feature export.

### 6) CPU Hotspots

- Rolling-window computations with repeated rescans.
- Duplicate SELECT execution in warehouse query path.
- Per-row SQL execution in ingest loops.

### 7) I/O Bottlenecks

- Warehouse ingest: many small SQL statements.
- Cache sync: repeated parquet scans per file.
- Yahoo auth refresh repeated due invalid cache state.

### 8) Concurrency Opportunities

- Independent parameter combinations can be parallelized with bounded workers.
- Per-symbol Yahoo fundamentals requests are currently serial (`crates/ferrotick-core/src/adapters/yahoo.rs:706`).
- File-partition registration in cache sync can be parallelized with I/O limits.

### 9) Vectorized Operations

- Partial use only.
- DuckDB SQL window functions are used for MA signal generation (`crates/ferrotick-backtest/src/vectorized/engine.rs:121`-`146`).
- Most ML feature transforms remain scalar loops; no parallel ndarray/rayon operations detected in source paths.

### 10) Batch Processing

- Not efficient in key ingest paths due per-row execute patterns.

### 11) Connection Pooling

- Warehouse pool exists and appears functionally correct (`crates/ferrotick-warehouse/src/duckdb.rs:74`-`95`).
- ML feature store bypasses pooling, reducing effectiveness.

### 12) Caching Effectiveness

- Ineffective currently for provider responses and Yahoo auth token lifecycle.

## Priority Fix Plan

1. Fix Yahoo auth cache validity/storage bug and remove redundant refresh calls.
2. Remove duplicate SELECT execution in warehouse query path.
3. Batch warehouse ingest using prepared/bulk operations and avoid per-row SQL formatting.
4. Precompute/reuse bar events and immutable price vectors in optimization/backtest sweeps.
5. Replace O(n*window) rolling computations with linear-time algorithms.
