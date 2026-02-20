//! # Warehouse Query Example
//!
//! This example demonstrates how to use the DuckDB warehouse
//! for local data storage and analytical queries.
//!
//! ## Usage
//!
//! ```bash
//! # First, sync some data to the warehouse
//! cargo run -- warehouse sync --symbol AAPL --start 2024-01-01 --end 2024-12-31
//!
//! # Then run this example
//! cargo run --example warehouse_query
//! ```
//!
//! ## What it demonstrates
//!
//! - Opening a warehouse connection
//! - Executing SQL queries with guardrails
//! - Working with query results

use ferrotick_warehouse::{Warehouse, WarehouseConfig, QueryGuardrails};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open the warehouse with default configuration
    // This uses ~/.ferrotick/cache/warehouse.duckdb
    println!("📦 Opening warehouse...");
    let warehouse = Warehouse::open_default()?;

    println!("✅ Warehouse opened at: {:?}", warehouse.db_path());

    // Define query guardrails for safety
    let guardrails = QueryGuardrails {
        max_rows: 1000,
        query_timeout_ms: 5000,
    };

    // Example 1: Count total bars in the database
    println!("\n📊 Counting records...");
    let result = warehouse.execute_query(
        "SELECT COUNT(*) as total FROM bars_1d",
        guardrails,
        false, // read-only
    )?;

    if let Some(row) = result.rows.first() {
        if let Some(total) = &row.first() {
            println!("   Total daily bars: {}", total);
        }
    }

    // Example 2: Get unique symbols
    println!("\n📋 Symbols in warehouse...");
    let result = warehouse.execute_query(
        "SELECT DISTINCT symbol FROM bars_1d ORDER BY symbol",
        guardrails,
        false,
    )?;

    let symbols: Vec<String> = result.rows.iter()
        .filter_map(|row| {
            if let Some(serde_json::Value::String(s)) = row.first() {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    println!("   Found: {}", symbols.join(", "));

    // Example 3: Calculate average daily volume
    println!("\n📈 Average daily volume by symbol...");
    let result = warehouse.execute_query(
        r#"
        SELECT 
            symbol,
            AVG(volume) as avg_volume,
            MIN(volume) as min_volume,
            MAX(volume) as max_volume
        FROM bars_1d
        WHERE volume IS NOT NULL
        GROUP BY symbol
        ORDER BY avg_volume DESC
        "#,
        guardrails,
        false,
    )?;

    println!("┌────────┬──────────────┬─────────────┬─────────────┐");
    println!("│ Symbol │ Avg Volume   │ Min Volume  │ Max Volume  │");
    println!("├────────┼──────────────┼─────────────┼─────────────┤");
    
    for row in &result.rows {
        let symbol = row.get(0).and_then(|v| v.as_str()).unwrap_or("N/A");
        let avg = row.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let min = row.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let max = row.get(3).and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        println!(
            "│ {:6} │ {:12.0} │ {:11.0} │ {:11.0} │",
            symbol, avg, min, max
        );
    }
    println!("└────────┴──────────────┴─────────────┴─────────────┘");

    // Example 4: Price performance
    println!("\n💰 30-day price performance...");
    let result = warehouse.execute_query(
        r#"
        WITH ranked AS (
            SELECT 
                symbol,
                close,
                ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY ts DESC) as rn,
                COUNT(*) OVER (PARTITION BY symbol) as total
            FROM bars_1d
        ),
        latest AS (
            SELECT symbol, close as latest_close
            FROM ranked WHERE rn = 1
        ),
        month_ago AS (
            SELECT symbol, close as month_ago_close
            FROM ranked WHERE rn = 30
        )
        SELECT 
            l.symbol,
            l.latest_close,
            m.month_ago_close,
            ((l.latest_close - m.month_ago_close) / m.month_ago_close * 100) as pct_change
        FROM latest l
        JOIN month_ago m ON l.symbol = m.symbol
        ORDER BY pct_change DESC
        "#,
        guardrails,
        false,
    )?;

    println!("┌────────┬────────────┬──────────────┬────────────┐");
    println!("│ Symbol │ Latest ($) │ 30d Ago ($)  │ Change (%) │");
    println!("├────────┼────────────┼──────────────┼────────────┤");
    
    for row in &result.rows {
        let symbol = row.get(0).and_then(|v| v.as_str()).unwrap_or("N/A");
        let latest = row.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ago = row.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let change = row.get(3).and_then(|v| v.as_f64()).unwrap_or(0.0);
        
        let emoji = if change >= 0.0 { "📈" } else { "📉" };
        println!(
            "│ {:6} │ {:10.2} │ {:12.2} │ {} {:7.2}% │",
            symbol, latest, ago, emoji, change
        );
    }
    println!("└────────┴────────────┴──────────────┴────────────┘");

    println!("\n✅ Query complete!");

    Ok(())
}
