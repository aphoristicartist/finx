# Phase 7 Feature Engineering Correctness Review

## Scope
- Requested path `crates/ferrotick-ml/src/indicators/*.rs` does not exist in this repo.
- Reviewed implementation in:
  - `crates/ferrotick-ml/src/features/indicators.rs`
  - `crates/ferrotick-ml/src/features/windows.rs`
  - `crates/ferrotick-ml/src/features/transforms.rs`
  - `crates/ferrotick-ml/src/features/mod.rs`
  - `crates/ferrotick-ml/src/features/store.rs`
- Verified delegated indicator math in dependency source:
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/ta-0.5.0/src/indicators/*.rs`

## Findings (ordered by severity)

### 1. Medium: RSI implementation is an EMA-based variant, not canonical Wilder RSI
- Evidence:
  - RSI in `ta` is implemented as:
    - `RSI_t = 100 * EMA(U)_t / (EMA(U)_t + EMA(D)_t)`
    - Source: `.../relative_strength_index.rs:23-24, 121-123`
  - EMA in this crate uses `alpha = 2/(period+1)`:
    - Source: `.../exponential_moving_average.rs:71, 93`
- Impact:
  - This differs from Wilder RSI smoothing (`alpha = 1/period`).
  - Values are mathematically consistent for the implemented EMA-RSI, but will not match Wilder-reference calculators exactly.
- Verdict:
  - `compute_rsi` is internally consistent with `ta` math, but indicator semantics should be documented as EMA-RSI.

### 2. Medium: No NaN/Inf input sanitization before feature computation and storage
- Evidence:
  - Raw close/high/low values are passed directly into indicators and transforms:
    - `features/mod.rs:132, 167-174`
    - `features/indicators.rs:30-31, 57-58, 83-85, 105-113`
  - `upsert_features` inserts options directly with no finite check:
    - `features/store.rs:146-165`
- Impact:
  - Non-finite input data can propagate to computed features and persist in DuckDB, then contaminate downstream dataset/model training.
- Verdict:
  - Correctness risk exists for dirty market data.

### 3. Low: Denominator guards are exact-zero only for returns/momentum
- Evidence:
  - `simple_returns`: `if prev == 0.0` (`features/transforms.rs:9`)
  - `rolling_momentum`: `if prev != 0.0` (`features/windows.rs:101`)
- Impact:
  - Near-zero prices can produce extreme magnitudes and numerical instability.
- Verdict:
  - Not a divide-by-zero bug, but a robustness gap.

### 4. Low: Standard deviation conventions differ across features
- Evidence:
  - Bollinger uses population SD (`/ N`) via `ta::StandardDeviation`:
    - `.../standard_deviation.rs:109`
  - `rolling_std_20` uses sample SD (`/ (N-1)`):
    - `features/windows.rs:30`
- Impact:
  - If these two are compared or jointly normalized, scale mismatch is expected.
- Verdict:
  - Not a bug by itself; document this intentionally if retained.

## Mathematical Verification

### RSI
- Wrapper correctness:
  - Period guard: `period > 0` (`features/indicators.rs:20-24`)
  - Warmup masking: values exposed when `index + 1 >= period` (`features/indicators.rs:32`)
- Formula correctness:
  - Delegated formula in `ta` matches its own documentation and implementation (`relative_strength_index.rs:23-24, 121-123`).
- Division-by-zero handling:
  - `ta` seeds initial `up/down` with `0.1` to avoid `0/0` (`relative_strength_index.rs:109-112`).
- Off-by-one:
  - First non-`None` RSI at index `period - 1` is consistent with wrapper policy.

### MACD
- Wrapper correctness:
  - Config guard: `fast > 0, slow > 0, signal > 0, fast < slow` (`features/indicators.rs:44-48`)
  - Warmup masking: `warmup = slow + signal - 1`, valid when `index + 1 >= warmup` (`features/indicators.rs:53, 59`)
- Formula correctness:
  - `macd = EMA_fast(price) - EMA_slow(price)`
  - `signal = EMA_signal(macd)`
  - `hist = macd - signal`
  - Source: `.../moving_average_convergence_divergence.rs:89-95`
- Off-by-one:
  - Warmup policy is internally consistent and matches existing tests (`phase7_feature_pipeline.rs:136`).

### Bollinger Bands
- Wrapper correctness:
  - Config guard: `period > 0`, finite positive multiplier (`features/indicators.rs:71-75`)
  - Warmup masking: `index + 1 >= period` (`features/indicators.rs:85`)
- Formula correctness:
  - `upper = mean + multiplier * sd`
  - `lower = mean - multiplier * sd`
  - Source: `.../bollinger_bands.rs:91-95`
- Std-dev correctness:
  - `ta::StandardDeviation` computes population SD over active window (`.../standard_deviation.rs:109`), which is a valid Bollinger convention.

### SMA / EMA
- SMA:
  - `rolling_mean` computes arithmetic mean over exact trailing window (`features/windows.rs:7-10`).
- EMA:
  - `ta::ExponentialMovingAverage` uses `alpha = 2/(period+1)` and recurrence `EMA_t = alpha*x_t + (1-alpha)*EMA_{t-1}` (`.../exponential_moving_average.rs:71, 93`).
- Verdict:
  - Implementations are mathematically correct for their stated formulas.

## Edge Cases and Numerical Stability
- Division-by-zero:
  - Properly guarded in returns/momentum for exact zero denominators (`transforms.rs:9`, `windows.rs:101`).
  - `rolling_std` avoids `window=1` denominator issue via `window < 2` guard (`windows.rs:15-16`).
- NaN/Inf handling:
  - No explicit sanitization at feature-engineering boundary (see Finding #2).
- Numerical stability:
  - Bollinger/SD in `ta` uses stable incremental statistics and clamps negative roundoff in `m2` (`.../standard_deviation.rs:92-107`).
  - Local `rolling_std` recomputes from slices each step and may be less stable for very large-magnitude price levels with tiny spreads (`windows.rs:22-31`).

## Feature Store Verification

### Feature computation mapping
- Row assembly aligns all feature vectors by identical `index` into each `FeatureRow` (`features/mod.rs:176-196`).
- Warmup and `None` propagation behavior is consistent across indicators/transforms.

### Storage correctness
- Schema column order matches insert parameter order (`features/store.rs:12-31` vs `35-41` and `147-165`).
- Writes are transactional and parameterized (`features/store.rs:138-141, 143-169`).
- Primary key `(symbol, timestamp)` supports deterministic upsert behavior (`features/store.rs:31, 36`).

### Retrieval correctness
- `SELECT` column order matches `FeatureRow` field mapping (`features/store.rs:197-200` vs `226-244`).
- Results are ordered by ascending timestamp (`features/store.rs:203`).

### Storage efficiency
- Positives:
  - Prepared statement reuse + single transaction reduces per-row overhead (`features/store.rs:138-147`).
  - Composite primary key enables indexed point/range access by symbol+timestamp.
- Limitations:
  - Inserts are still row-by-row; for very large batches DuckDB appender/COPY would be more throughput-efficient.

## Validation Performed
- Executed:
  - `cargo test -p ferrotick-ml --test behavioral_indicators --test phase7_feature_pipeline`
- Result:
  - All 9 tests passed (`7 + 2`), confirming current behavior and store roundtrip/parquet export path.
