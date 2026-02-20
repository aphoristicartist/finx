# 🦀 Ferrotick Examples

This directory contains practical examples demonstrating how to use Ferrotick in various scenarios.

## 📑 Table of Contents

| Example | Description | Difficulty |
|---------|-------------|------------|
| [`basic_quote.rs`](basic_quote.rs) | Fetch a simple stock quote | 🟢 Beginner |
| [`multi_symbol.rs`](multi_symbol.rs) | Fetch quotes for multiple symbols | 🟢 Beginner |
| [`bars_analysis.rs`](bars_analysis.rs) | Analyze historical price data | 🟡 Intermediate |
| [`warehouse_query.rs`](warehouse_query.rs) | Query the local DuckDB warehouse | 🟡 Intermediate |
| [`custom_adapter.rs`](custom_adapter.rs) | Implement a custom data source | 🔴 Advanced |
| [`streaming_consumer.rs`](streaming_consumer.rs) | Consume NDJSON streaming output | 🔴 Advanced |

## 🚀 Quick Start

```bash
# Run an example
cargo run --example basic_quote

# Run with a specific API key
POLYGON_API_KEY=your_key cargo run --example bars_analysis
```

## 📋 Prerequisites

Most examples require API keys from one or more providers:

```bash
export POLYGON_API_KEY=your_polygon_key
export ALPHAVANTAGE_API_KEY=your_alphavantage_key
export ALPACA_API_KEY=your_alpaca_key
export ALPACA_SECRET_KEY=your_alpaca_secret
```

> **Note:** Some examples work with the `demo` key for testing, but with limited functionality.

## 🔧 Example Categories

### 🟢 Beginner Examples

Start here if you're new to Ferrotick. These examples cover the basics of fetching market data.

- **basic_quote.rs** - The simplest possible example: fetch a quote for a single symbol
- **multi_symbol.rs** - Fetch quotes for multiple symbols in one request

### 🟡 Intermediate Examples

These examples demonstrate more complex usage patterns.

- **bars_analysis.rs** - Fetch historical OHLCV data and compute basic analytics
- **warehouse_query.rs** - Store data locally and run analytical queries

### 🔴 Advanced Examples

For power users building custom integrations.

- **custom_adapter.rs** - Implement the `DataSource` trait for a custom provider
- **streaming_consumer.rs** - Build a real-time data consumer with NDJSON streaming

## 📖 Learning Path

1. Start with `basic_quote.rs` to understand the core concepts
2. Move to `multi_symbol.rs` to see batch operations
3. Explore `bars_analysis.rs` for historical data handling
4. Try `warehouse_query.rs` to learn about local storage
5. Dive into `custom_adapter.rs` for provider integration
6. Master `streaming_consumer.rs` for real-time applications

## 🤝 Contributing

Have a useful example to share? We'd love to include it! Please submit a PR following the existing format.

---

For more details, see the [main documentation](../README.md).
