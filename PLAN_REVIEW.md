# PLAN.md Review - Phase 7 Implementation

**Reviewer:** Gemini 3.1 Pro
**Date:** 2026-02-26
**Plan Version:** Current

## Critical Issues (must fix before implementation)

### 1. SQL Injection Risk (Step 9)

**Location:** `features/store.rs` - `upsert_features`, `load_features`, `load_daily_bars`

**Issue:** The implementation uses string interpolation (`format!`) to construct SQL queries. While `Symbol::parse` provides some protection, this violates the security mandate established in `ferrotick-warehouse/src/lib.rs` (lines 14-26), which requires parameterized queries.

**Example:**
```rust
// CURRENT (INSECURE)
let sql = format!(
    "INSERT OR REPLACE INTO features (...) VALUES ('{}', TRY_CAST('{}' AS TIMESTAMP), ...)",
    row.symbol, row.timestamp, ...
);
```

**Fix:** Use `connection.prepare()` and pass parameters as a slice. Since `Warehouse` doesn't expose a generic parameterized query method, `FeatureStore` should acquire a connection via `self.warehouse.manager.acquire(AccessMode::ReadWrite)` and use the underlying `duckdb` crate's API.

```rust
// CORRECT (SECURE)
let connection = self.warehouse.manager.acquire(AccessMode::ReadWrite)?;
let mut stmt = connection.prepare(
    "INSERT OR REPLACE INTO features \
     (symbol, timestamp, rsi, ...) VALUES (?, ?, ?, ...)"
)?;
for row in rows {
    stmt.execute(params![
        &row.symbol,
        &row.timestamp,
        &row.rsi,
        ...
    ])?;
}
```

### 2. Performance Degradation (Step 9)

**Location:** `features/store.rs` - `upsert_features`

**Issue:** The method performs an individual `execute_query` call for every single row in a loop. In DuckDB, this will cause significant overhead (acquiring/releasing connections and starting/committing implicit transactions for every bar).

**Example:**
```rust
// CURRENT (SLOW)
for row in rows {
    self.warehouse.execute_query(sql.as_str(), ...)?;  // Transaction per row!
}
```

**Fix:** Wrap the entire loop in a single manual transaction (`BEGIN TRANSACTION` ... `COMMIT`) and use a prepared statement. This will improve ingestion speed by orders of magnitude for large datasets.

```rust
// CORRECT (FAST)
let connection = self.warehouse.manager.acquire(AccessMode::ReadWrite)?;
connection.execute_batch("BEGIN TRANSACTION")?;
let mut stmt = connection.prepare(INSERT_SQL)?;
for row in rows {
    stmt.execute(params![...])?;
}
connection.execute_batch("COMMIT")?;
```

### 3. Schema Mismatch (Step 9 & 13)

**Location:** `features/store.rs` schema definition, `training/dataset.rs` DatasetBuilder

**Issue:** The DuckDB schema in Step 9 only includes P0 features (`rsi`, `macd`, `bb_upper`, `bb_lower`, `atr`, `return_1d`, `return_5d`, `return_20d`, `rolling_mean_20`, `rolling_std_20`), but `DatasetBuilder::build` in Step 13 requires P1 features (`lag_1`, `lag_2`, `lag_3`, `rolling_momentum`) to produce a training matrix. Because `FeatureStore::load_features` returns these as `None`, `DatasetBuilder` will drop all rows, making it impossible to build a dataset from persisted data.

**Fix Option A:** Add P1 feature columns to the `features` table schema:

```sql
CREATE TABLE IF NOT EXISTS features (
    symbol VARCHAR,
    timestamp TIMESTAMP,
    -- P0 features (existing)
    rsi DOUBLE,
    macd DOUBLE,
    macd_signal DOUBLE,
    bb_upper DOUBLE,
    bb_lower DOUBLE,
    atr DOUBLE,
    return_1d DOUBLE,
    return_5d DOUBLE,
    return_20d DOUBLE,
    rolling_mean_20 DOUBLE,
    rolling_std_20 DOUBLE,
    -- P1 features (NEW)
    lag_1 DOUBLE,
    lag_2 DOUBLE,
    lag_3 DOUBLE,
    rolling_momentum DOUBLE,
    PRIMARY KEY (symbol, timestamp)
);
```

**Fix Option B:** Include the `close` price in the `features` table so lags can be recomputed on the fly during loading.

**Recommendation:** Use Option A for simplicity and consistency.

## Suggested Improvements (should fix for better quality)

### 1. CLI Output Management (Step 16)

**Issue:** The `ml features` command prints the entire JSON of computed rows to stdout. For a symbol with 10 years of daily data (~2500 rows), this will overwhelm the terminal.

**Suggestion:** By default, only show a summary (e.g., "Computed 2500 rows for AAPL, stored in DuckDB") and hide the full row list unless a `--verbose` flag is provided.

```rust
// Step 16 modification
let output_json = if args.verbose {
    serde_json::json!({
        "symbol": symbol.as_str(),
        "rows_computed": rows.len(),
        "stored_rows": stored_rows,
        "features": rows,  // Only include if --verbose
    })
} else {
    serde_json::json!({
        "symbol": symbol.as_str(),
        "rows_computed": rows.len(),
        "stored_rows": stored_rows,
    })
};
```

### 2. Indicator Warmup Documentation (Step 6)

**Issue:** The `ta` crate's indicators (like RSI and MACD) require a warmup period before producing valid values. The plan correctly handles this with `None`, but it's worth noting that `macd` and `bollinger` use different warmup logic than simple rolling windows.

**Suggestion:** Add a brief comment in `indicators.rs` explaining that `None` values represent the mathematical warmup period of the specific indicator to ensure data scientists understand why the first N rows are empty.

```rust
// Example comment
/// Compute technical indicators for the given bars.
///
/// # Warmup Period
///
/// Indicators require historical data before producing valid values:
/// - RSI (14-period): first 14 values will be `None`
/// - MACD (12/26/9): first 26 values will be `None`
/// - Bollinger Bands (20-period): first 20 values will be `None`
/// - ATR (14-period): first 14 values will be `None`
```

### 3. Parquet Export Validation (Step 9)

**Issue:** `export_features_parquet` creates the parent directory if it doesn't exist, which is good. However, it doesn't check if the file is currently locked or if there is sufficient disk space.

**Suggestion:** Add a check to return a specific `MlError::Io` if the file cannot be written, ensuring the CLI provides a clean error message.

```rust
let file = std::fs::File::create(path)
    .map_err(|e| MlError::Io(format!("failed to create {}: {}", path.display(), e)))?;
```

## Clarifications Needed (ask Oracle)

### 1. Target Column Context

**Question:** `TargetColumn` in Step 13 defines 1d, 5d, and 20d returns. Should these be "forward returns" (future price change) for training, or "past returns" (historical change)?

**Context:** Usually, ML targets are forward-looking. The current implementation in `FeatureEngineer` computes historical returns. If used for training, you'll need to "shift" these columns.

**Example:**
```rust
// For training targets, we want:
// target_1d = (close_t+1 - close_t) / close_t  // FUTURE return

// But current implementation computes:
// return_1d = (close_t - close_t-1) / close_t-1  // PAST return
```

### 2. Feature Scaling Timing

**Question:** The plan implements `z_score` and `min_max` transforms in Step 7 but doesn't explicitly call them in the `FeatureEngineer`. Should scaling be done per-symbol in the engineering phase or globally during dataset building?

**Context:** Global scaling usually requires the full dataset's mean/std, while per-symbol scaling is independent but may cause train/test distribution mismatch.

**Recommendation:** Implement scaling in `DatasetBuilder` with a configurable mode (per-symbol vs global) for flexibility.

## Summary

The PLAN.md is comprehensive and well-structured. The critical issues (SQL injection, performance, schema mismatch) must be addressed before implementation to ensure production readiness. The suggested improvements will enhance usability and maintainability but are not blocking.

**Recommendation:** Update PLAN.md to address the 3 critical issues, then proceed to implementation.
