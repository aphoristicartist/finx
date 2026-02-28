use crate::{BacktestError, PerformanceMetrics};
use duckdb::Connection;
use ndarray::Array1;
use std::collections::HashMap;

/// Vectorized backtesting engine using DuckDB for columnar operations
pub struct VectorizedBacktest {
    db: Connection,
}

/// Result of a single parameter combination backtest
#[derive(Debug, Clone)]
pub struct ParamResult {
    pub params: HashMap<String, f64>,
    pub metrics: PerformanceMetrics,
}

impl VectorizedBacktest {
    /// Create a new vectorized backtest engine with in-memory DuckDB
    pub fn new() -> Result<Self, BacktestError> {
        let db = Connection::open_in_memory()
            .map_err(|e| BacktestError::EngineError(format!("Failed to create DuckDB: {}", e)))?;
        Ok(Self { db })
    }

    /// Load price data into DuckDB for columnar operations
    /// symbol parameter is stored alongside each bar for filtering/grouping
    pub fn load_bars(&self, symbol: &str, bars: &[ferrotick_core::Bar]) -> Result<(), BacktestError> {
        self.db
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS bars (
                symbol VARCHAR,
                ts TIMESTAMP,
                open DOUBLE,
                high DOUBLE,
                low DOUBLE,
                close DOUBLE,
                volume BIGINT
            );
            CREATE INDEX IF NOT EXISTS idx_ts ON bars(ts);",
            )
            .map_err(|e| BacktestError::EngineError(format!("Failed to create table: {}", e)))?;

        // Clear existing data
        self.db
            .execute("DELETE FROM bars", [])
            .map_err(|e| BacktestError::EngineError(format!("Failed to clear table: {}", e)))?;

        // Insert bars using prepared statement
        let mut stmt = self
            .db
            .prepare("INSERT INTO bars VALUES (?, ?, ?, ?, ?, ?, ?)")
            .map_err(|e| BacktestError::EngineError(format!("Failed to prepare insert: {}", e)))?;

        for bar in bars {
            let volume = bar.volume.unwrap_or(0) as i64;
            stmt.execute(duckdb::params![
                symbol,
                bar.ts.format_rfc3339(),
                bar.open,
                bar.high,
                bar.low,
                bar.close,
                volume
            ])
            .map_err(|e| BacktestError::EngineError(format!("Failed to insert bar: {}", e)))?;
        }

        Ok(())
    }

    /// Run backtest on parameter grid using vectorized operations
    pub fn run_parameter_sweep(
        &self,
        strategy_type: &str,
        param_grid: HashMap<String, Vec<f64>>,
    ) -> Result<Vec<ParamResult>, BacktestError> {
        // Generate all parameter combinations
        let combinations = self.generate_param_combinations(&param_grid);

        // Process each parameter combination (sequential, but DuckDB handles vectorization internally)
        let mut results = Vec::new();
        for params in &combinations {
            let result = self.run_single_backtest(strategy_type, params)?;
            results.push(result);
        }

        Ok(results)
    }

    fn run_single_backtest(
        &self,
        strategy_type: &str,
        params: &HashMap<String, f64>,
    ) -> Result<ParamResult, BacktestError> {
        // Execute strategy logic using DuckDB queries
        let signals = self.generate_signals_vectorized(strategy_type, params)?;
        let equity_curve = self.calculate_equity_curve(&signals)?;
        let metrics = self.calculate_metrics(&equity_curve)?;

        Ok(ParamResult {
            params: params.clone(),
            metrics,
        })
    }

    fn generate_signals_vectorized(
        &self,
        strategy_type: &str,
        params: &HashMap<String, f64>,
    ) -> Result<Array1<i8>, BacktestError> {
        match strategy_type {
            "ma_crossover" => {
                let short_period = params
                    .get("short_period")
                    .copied()
                    .unwrap_or(10.0) as i64;
                let long_period = params
                    .get("long_period")
                    .copied()
                    .unwrap_or(30.0) as i64;

                // Use DuckDB window functions for moving averages
                let query = format!(
                    r#"WITH ma_data AS (
                        SELECT 
                            ts,
                            close,
                            AVG(close) OVER (ORDER BY ts ROWS BETWEEN {} PRECEDING AND CURRENT ROW) as short_ma,
                            AVG(close) OVER (ORDER BY ts ROWS BETWEEN {} PRECEDING AND CURRENT ROW) as long_ma
                        FROM bars
                        ORDER BY ts
                    )
                    SELECT 
                        CASE 
                            WHEN short_ma > long_ma 
                                AND LAG(short_ma) OVER (ORDER BY ts) <= LAG(long_ma) OVER (ORDER BY ts) 
                            THEN 1
                            WHEN short_ma < long_ma 
                                AND LAG(short_ma) OVER (ORDER BY ts) >= LAG(long_ma) OVER (ORDER BY ts) 
                            THEN -1
                            ELSE 0
                        END as signal
                    FROM ma_data
                    ORDER BY ts"#,
                    short_period.saturating_sub(1),
                    long_period.saturating_sub(1)
                );

                // Execute query and extract signals
                let mut stmt = self
                    .db
                    .prepare(&query)
                    .map_err(|e| BacktestError::EngineError(format!("Failed to prepare signal query: {}", e)))?;

                let signal_rows: Result<Vec<i8>, _> = stmt
                    .query_map([], |row| row.get::<_, i8>(0))
                    .map(|iter| iter.collect())
                    .map_err(|e| BacktestError::EngineError(format!("Failed to execute signal query: {}", e)))?;

                let signals = signal_rows.map_err(|e| {
                    BacktestError::EngineError(format!("Failed to read signals: {}", e))
                })?;

                Ok(Array1::from_vec(signals))
            }
            _ => Err(BacktestError::UnsupportedStrategy(strategy_type.to_string())),
        }
    }

    fn calculate_equity_curve(&self, signals: &Array1<i8>) -> Result<Array1<f64>, BacktestError> {
        // Get price data
        let mut stmt = self
            .db
            .prepare("SELECT close FROM bars ORDER BY ts")
            .map_err(|e| BacktestError::EngineError(format!("Failed to prepare price query: {}", e)))?;

        let prices: Result<Vec<f64>, _> = stmt
            .query_map([], |row| row.get::<_, f64>(0))
            .map(|iter| iter.collect())
            .map_err(|e| BacktestError::EngineError(format!("Failed to execute price query: {}", e)))?;

        let prices = prices.map_err(|e| BacktestError::EngineError(format!("Failed to read prices: {}", e)))?;

        if prices.is_empty() {
            return Ok(Array1::from_vec(vec![100_000.0]));
        }

        let mut equity = vec![100_000.0]; // Starting capital
        let mut position = 0.0;
        let mut i = 1;

        for &signal in signals.iter().skip(1) {
            if i >= prices.len() {
                break;
            }

            if signal == 1 && position == 0.0 {
                // Buy
                position = equity[i - 1] / prices[i];
                equity.push(position * prices[i]);
            } else if signal == -1 && position > 0.0 {
                // Sell
                equity.push(position * prices[i]);
                position = 0.0;
            } else {
                // Hold
                if position > 0.0 {
                    equity.push(position * prices[i]);
                } else {
                    equity.push(equity[i - 1]);
                }
            }
            i += 1;
        }

        // Fill remaining equity values if needed
        while equity.len() < signals.len() {
            equity.push(*equity.last().unwrap_or(&100_000.0));
        }

        Ok(Array1::from_vec(equity))
    }

    fn calculate_metrics(&self, equity_curve: &Array1<f64>) -> Result<PerformanceMetrics, BacktestError> {
        if equity_curve.len() < 2 {
            // Create a minimal EquityPoint slice for MetricsReport
            let equity_points = vec![
                crate::metrics::EquityPoint {
                    ts: ferrotick_core::UtcDateTime::from_unix_timestamp(0).expect("valid timestamp"),
                    equity: 100_000.0,
                    cash: 100_000.0,
                    position_value: 0.0,
                }
            ];
            return Ok(PerformanceMetrics::from_equity_curve(&equity_points, 252.0));
        }

        // Convert Array1 to Vec<f64>
        let equity_values: Vec<f64> = equity_curve.iter().copied().collect();
        
        // Create EquityPoints from equity curve
        let equity_points: Vec<crate::metrics::EquityPoint> = equity_values
            .iter()
            .enumerate()
            .map(|(i, &equity)| crate::metrics::EquityPoint {
                ts: ferrotick_core::UtcDateTime::from_unix_timestamp((i as i64) * 86400).expect("valid timestamp"),
                equity,
                cash: if equity == equity_values[0] { 100_000.0 } else { 0.0 },
                position_value: if equity == equity_values[0] { 0.0 } else { equity },
            })
            .collect();

        Ok(PerformanceMetrics::from_equity_curve(&equity_points, 252.0))
    }

    fn calculate_max_drawdown(&self, equity_curve: &Array1<f64>) -> f64 {
        if equity_curve.is_empty() {
            return 0.0;
        }
        
        let mut max_drawdown = 0.0;
        let mut peak = equity_curve[0];

        for &equity in equity_curve.iter() {
            if equity > peak {
                peak = equity;
            }
            let drawdown = (peak - equity) / peak;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        max_drawdown
    }

    fn generate_param_combinations(
        &self,
        param_grid: &HashMap<String, Vec<f64>>,
    ) -> Vec<HashMap<String, f64>> {
        let mut combinations = vec![HashMap::new()];

        for (param_name, values) in param_grid {
            let mut new_combinations = Vec::new();

            for combo in &combinations {
                for value in values {
                    let mut new_combo = combo.clone();
                    new_combo.insert(param_name.clone(), *value);
                    new_combinations.push(new_combo);
                }
            }

            combinations = new_combinations;
        }

        combinations
    }
}

impl Default for VectorizedBacktest {
    fn default() -> Self {
        Self::new().expect("Failed to create default VectorizedBacktest")
    }
}
