# Phase 3 Warehouse Correctness Review

## Scope
- Reviewed `crates/ferrotick-warehouse/src/lib.rs` in detail.
- Cross-checked `crates/ferrotick-warehouse/src/duckdb.rs` and `crates/ferrotick-warehouse/src/migrations.rs` where behavior depends on shared code paths.
- Ran tests: `cargo test -p ferrotick-warehouse` (all passed).

## Findings (ordered by severity)

### High

1. Timeout guardrails do not actually bound query execution time.
- Location: `crates/ferrotick-warehouse/src/lib.rs:671`, `crates/ferrotick-warehouse/src/lib.rs:672`, `crates/ferrotick-warehouse/src/lib.rs:695`, `crates/ferrotick-warehouse/src/lib.rs:714`, `crates/ferrotick-warehouse/src/lib.rs:725`
- Details: Timeout is checked only after `execute_batch` returns for writes and between row fetches for reads. If planning/execution blocks before first row, the query can exceed `query_timeout_ms` without interruption.
- Impact: Long-running queries are not forcibly stopped; timeout is best-effort wall-clock validation, not enforcement.

2. Migration application has a race condition under concurrent initialization.
- Location: `crates/ferrotick-warehouse/src/migrations.rs:143` to `crates/ferrotick-warehouse/src/migrations.rs:158`
- Details: Uses check-then-act (`SELECT COUNT(*)` then apply migration then insert version) without transactional locking.
- Impact: Two processes/threads initializing simultaneously can both see `applied_count == 0`; one may fail on duplicate `schema_migrations.version` insert or observe partial migration state.

3. Parquet sync can silently treat unreadable/corrupt parquet files as successful with `row_count=0`.
- Location: `crates/ferrotick-warehouse/src/lib.rs:901` to `crates/ferrotick-warehouse/src/lib.rs:912`, `crates/ferrotick-warehouse/src/lib.rs:597`, `crates/ferrotick-warehouse/src/lib.rs:614`
- Details: `read_parquet_row_count` swallows query errors via `unwrap_or_default()`.
- Impact: Corrupt/unreadable files can be registered in `cache_manifest` as valid partitions with zero rows, causing silent data-quality issues.

### Medium

4. `register_partition` is not atomic across manifest and ingest log writes.
- Location: `crates/ferrotick-warehouse/src/lib.rs:614` to `crates/ferrotick-warehouse/src/lib.rs:637`
- Details: Two separate statements execute without explicit transaction.
- Impact: Partial success is possible (manifest updated, ingest_log missing) if the second statement fails.

5. SELECT queries are executed twice.
- Location: `crates/ferrotick-warehouse/src/lib.rs:695`, `crates/ferrotick-warehouse/src/lib.rs:710`
- Details: `statement.query(...)` is called once and discarded, then called again for actual iteration.
- Impact: Extra execution cost and potential correctness issues for non-deterministic/volatile SELECTs.

6. Connection pool size is only an idle-cap; concurrent active connections are unbounded.
- Location: `crates/ferrotick-warehouse/src/duckdb.rs:86` to `crates/ferrotick-warehouse/src/duckdb.rs:89`, `crates/ferrotick-warehouse/src/duckdb.rs:143` to `crates/ferrotick-warehouse/src/duckdb.rs:150`
- Details: If no idle connection exists, a new one is always opened. `max_pool_size` only limits what is retained on drop.
- Impact: Under bursty concurrency, connection count can grow without bound and pressure memory/file handles.

### Low

7. Read-only access mode is best-effort and can silently fail.
- Location: `crates/ferrotick-warehouse/src/duckdb.rs:172` to `crates/ferrotick-warehouse/src/duckdb.rs:176`
- Details: Failure to set `SET access_mode = 'READ_ONLY'` is ignored.
- Impact: `acquire_connection(ReadOnly)` may return a writable connection on some embedded versions.

8. Failed rollback is ignored before connection returns to pool.
- Location: `crates/ferrotick-warehouse/src/lib.rs:654`
- Details: Rollback errors are discarded.
- Impact: A potentially unhealthy connection can be reused, increasing risk of follow-on failures.

## Requested Checks

### DuckDB connection pooling
- Verdict: **Partially correct**
- Strengths: Mutex-protected pool state, RAII return via `Drop`.
- Issues: No hard cap on active connections, read-only mode not guaranteed, unhealthy transaction state may be returned to pool.

### Parquet export/import
- Verdict: **Partially correct (metadata sync only)**
- What works: Parquet files are discovered and metadata (`row_count`, `min_ts`, `max_ts`, checksum) is recorded.
- Gap: No full row import into warehouse tables in this phase; error handling can silently downgrade failed parquet reads to `row_count=0`.

### SQL queries
- Verdict: **Mostly correct with caveats**
- Strengths: Ingestion paths are parameterized; dataset name interpolation is whitelisted (`bars_1m` / `bars_1d`).
- Caveats: `execute_query` timeout semantics are weaker than advertised; SELECT is executed twice.

### Transactions
- Verdict: **Partially correct**
- Strengths: Ingest paths (`ingest_quotes`, `ingest_bars`, `ingest_fundamentals`) use explicit begin/commit/rollback flow.
- Issues: `register_partition` is not transactional; rollback failure is ignored.

### Concurrent access safety
- Verdict: **Not fully safe**
- Strengths: Basic mutex protection around idle pools.
- Issues: Migration race and unbounded active connection growth.

## Security / Reliability Checks

### SQL injection vulnerabilities
- No direct SQL injection found in ingest and manifest writes:
  - Parameter binding is used for user-controlled values in ingest and partition registration.
  - Dynamic table name in bars ingest is strict-whitelisted.
- Note: `execute_query` with `allow_write=true` intentionally executes caller-provided SQL.

### Connection leaks
- No direct leak pattern found (connections are RAII-managed and returned/dropped in `Drop`).
- Reliability concern remains for reusing connections after rollback failure.

### Memory leaks
- No Rust-level memory leak pattern found in reviewed code paths.

### Race conditions
- Confirmed migration race in `apply_migrations`.
- Resource race risk from unbounded connection creation under concurrency.

## Test Coverage Gaps
- Missing stress tests for concurrent `Warehouse::open` / migrations.
- Missing tests for timeout behavior on deliberately long-running queries.
- Missing tests for rollback failure handling and connection reuse safety.
- Missing tests for corrupt/unreadable parquet behavior in `sync_cache`.
