<p align="center">
  <img src="https://raw.githubusercontent.com/nickelc/ferrotick/main/docs/logo.png" alt="Ferrotick Logo" width="200" onerror="this.style.display='none'">
</p>

<h1 align="center">🦀 Ferrotick</h1>

<p align="center">
  <strong>Provider-neutral financial data CLI and core contracts implemented in Rust</strong>
</p>

<p align="center">
  <a href="https://github.com/ferrotick/ferrotick/actions/workflows/ci.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/ferrotick/ferrotick/ci.yml?branch=main&label=build&style=flat-square" alt="Build Status">
  </a>
  <a href="https://crates.io/crates/ferrotick">
    <img src="https://img.shields.io/crates/v/ferrotick.svg?style=flat-square" alt="Crates.io Version">
  </a>
  <a href="https://docs.rs/ferrotick">
    <img src="https://img.shields.io/docsrs/ferrotick?style=flat-square" alt="Documentation">
  </a>
  <a href="https://github.com/ferrotick/ferrotick/blob/main/LICENSE">
    <img src="https://img.shields.io/crates/l/ferrotick.svg?style=flat-square" alt="License">
  </a>
  <a href="https://deps.rs/repo/github/ferrotick/ferrotick">
    <img src="https://deps.rs/repo/github/ferrotick/ferrotick/status.svg?style=flat-square" alt="Dependency Status">
  </a>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation">Installation</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-usage-examples">Usage</a> •
  <a href="#-documentation">Documentation</a>
</p>

---

**Ferrotick** is a high-performance, provider-neutral financial data toolkit built in Rust. It provides a unified interface for fetching market data from multiple providers (Polygon, Yahoo Finance, Alpha Vantage, Alpaca) with built-in caching, local analytics via DuckDB, and AI-agent-ready streaming output.

📊 **Get quotes, bars, fundamentals, and search across providers**  
⚡ **Fast, typed, and production-ready**  
🔒 **Secure by design with parameterized queries and TLS**  
🤖 **AI-agent streaming support with NDJSON**  

---

## 📑 Table of Contents

- [✨ Features](#-features)
- [📦 Installation](#-installation)
  - [From Source](#from-source)
  - [Using Cargo](#using-cargo)
  - [Pre-built Binaries](#pre-built-binaries)
- [🚀 Quick Start](#-quick-start)
- [💻 Usage Examples](#-usage-examples)
  - [Fetch Quotes](#fetch-quotes)
  - [Get OHLCV Bars](#get-ohlcv-bars)
  - [Search Instruments](#search-instruments)
  - [Query Warehouse](#query-warehouse)
  - [Streaming for AI Agents](#streaming-for-ai-agents)
- [📊 Capability Matrix](#-capability-matrix)
- [⚙️ Configuration](#️-configuration)
  - [Environment Variables](#environment-variables)
  - [Source Selection](#source-selection)
- [📖 Documentation](#-documentation)
- [📁 Project Structure](#-project-structure)
- [🧪 Testing](#-testing)
- [🔧 Development](#-development)
- [🤝 Contributing](#-contributing)
- [📝 License](#-license)
- [🙏 Acknowledgments](#-acknowledgments)

---

## ✨ Features

| Feature | Description |
|---------|-------------|
| 🔄 **Multi-Provider Support** | Fetch data from Polygon, Yahoo Finance, Alpha Vantage, and Alpaca through a unified interface |
| 📊 **DuckDB Warehouse** | Local analytics with DuckDB for fast aggregations and complex queries |
| 🚀 **High Performance** | Built on Rust's async ecosystem with connection pooling and circuit breakers |
| 🔒 **Security First** | Parameterized queries, TLS encryption, and secure API key handling |
| 📦 **Parquet Caching** | Automatic local caching in Parquet format for offline access |
| 🤖 **AI-Agent Ready** | NDJSON streaming mode for real-time data consumption |
| 📈 **Type-Safe Models** | Strongly-typed domain models with validation |
| ⚡ **Smart Routing** | Automatic source selection with fallback and priority chains |
| 🛡️ **Circuit Breaker** | Resilient upstream failure handling |

---

## 📦 Installation

### From Source

**Prerequisites:** Rust 1.83 or later

```bash
# Clone the repository
git clone https://github.com/ferrotick/ferrotick.git
cd ferrotick

# Build in release mode
cargo build --release

# The binary will be at target/release/ferrotick
```

### Using Cargo

```bash
cargo install ferrotick --locked
```

### Pre-built Binaries

Download the latest release for your platform from [GitHub Releases](https://github.com/ferrotick/ferrotick/releases).

| Platform | Architecture | Download |
|----------|-------------|----------|
| macOS | Apple Silicon (M1/M2/M3) | `ferrotick-aarch64-apple-darwin.tar.gz` |
| macOS | Intel (x86_64) | `ferrotick-x86_64-apple-darwin.tar.gz` |
| Linux | x86_64 | `ferrotick-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | ARM64 | `ferrotick-aarch64-unknown-linux-gnu.tar.gz` |
| Windows | x86_64 | `ferrotick-x86_64-pc-windows-msvc.zip` |

---

## 🚀 Quick Start

```bash
# Set your API key (required for full functionality)
export POLYGON_API_KEY=your_api_key_here

# Fetch the latest quote for AAPL
ferrotick quote AAPL

# Get daily bars for the last 30 days
ferrotick bars AAPL --interval 1d --limit 30

# Search for instruments
ferrotick search apple --limit 10

# Query the local warehouse
ferrotick sql "SELECT * FROM bars_1d WHERE symbol='AAPL' LIMIT 10"
```

**Example Output:**

```json
{
  "data": {
    "symbol": "AAPL",
    "price": 178.52,
    "bid": 178.50,
    "ask": 178.55,
    "volume": 52847392,
    "currency": "USD",
    "as_of": "2024-02-20T16:00:00Z"
  },
  "meta": {
    "request_id": "req_abc123",
    "source_chain": ["polygon"],
    "latency_ms": 142,
    "cache_hit": false
  },
  "errors": [],
  "warnings": []
}
```

---

## 💻 Usage Examples

### Fetch Quotes

Get real-time or delayed quotes for one or more symbols:

```bash
# Single symbol
ferrotick quote AAPL

# Multiple symbols
ferrotick quote AAPL MSFT GOOGL

# Pretty-printed JSON output
ferrotick quote AAPL --pretty

# Use a specific provider
ferrotick quote AAPL --source polygon

# Table format
ferrotick quote AAPL --format table
```

### Get OHLCV Bars

Fetch historical OHLCV (Open, High, Low, Close, Volume) data:

```bash
# Daily bars (default)
ferrotick bars AAPL

# 5-minute bars
ferrotick bars AAPL --interval 5m --limit 100

# Hourly bars
ferrotick bars AAPL --interval 1h --limit 48

# Available intervals: 1m, 5m, 15m, 1h, 1d
```

### Search Instruments

Search for instruments by name or symbol:

```bash
# Search by keyword
ferrotick search apple

# Limit results
ferrotick search microsoft --limit 5
```

### Query Warehouse

Run SQL queries against the local DuckDB warehouse:

```bash
# Query daily bars
ferrotick sql "SELECT * FROM v_daily_bars WHERE symbol='AAPL' ORDER BY ts DESC LIMIT 10"

# Aggregate query
ferrotick sql "SELECT symbol, AVG(close) as avg_close FROM bars_1d GROUP BY symbol"

# With query timeout
ferrotick sql "SELECT COUNT(*) FROM bars_1d" --query-timeout-ms 10000
```

### Sync Historical Data

Fetch and store historical data in the warehouse:

```bash
# Sync a year of daily bars
ferrotick warehouse sync --symbol AAPL --start 2024-01-01 --end 2024-12-31

# Query via DuckDB directly
duckdb ~/.local/share/ferrotick/warehouse.duckdb \
  "SELECT * FROM bars_1d WHERE symbol='AAPL' LIMIT 10"
```

### Streaming for AI Agents

Enable NDJSON streaming for real-time consumption:

```bash
# Enable streaming mode
ferrotick quote AAPL --stream

# Output format (one JSON object per line):
# {"event":"start","request_id":"req_123",...}
# {"event":"progress","message":"Fetching from polygon",...}
# {"event":"chunk","data":{...},...}
# {"event":"end","latency_ms":142,...}
```

### Strict Mode

Treat warnings as errors for CI/CD pipelines:

```bash
# Returns exit code 5 if any warnings/errors present
ferrotick quote AAPL --strict
```

### Inspect Schemas

View the JSON schemas used for validation:

```bash
# List available schemas
ferrotick schema list

# View a specific schema
ferrotick schema get envelope
```

---

## 📊 Capability Matrix

| Provider | Quote | Bars | Fundamentals | Search | Priority Score |
|----------|:-----:|:----:|:------------:|:------:|:--------------:|
| **Polygon** | ✅ | ✅ | ✅ | ✅ | 90 |
| **Alpaca** | ✅ | ✅ | ❌ | ❌ | 85 |
| **Yahoo Finance** | ✅ | ✅ | ✅ | ✅ | 78 |
| **Alpha Vantage** | ✅ | ✅ | ✅ | ✅ | 70 |

The `--source auto` strategy uses priority scores for automatic source selection with fallback.

---

## ⚙️ Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FERROTICK_POLYGON_API_KEY` | Polygon.io API key | `demo` |
| `FERROTICK_YAHOO_API_KEY` | Yahoo Finance API key | `demo` |
| `FERROTICK_ALPHAVANTAGE_API_KEY` | Alpha Vantage API key | `demo` |
| `FERROTICK_ALPACA_API_KEY` | Alpaca API key ID | `demo` |
| `FERROTICK_ALPACA_SECRET_KEY` | Alpaca API secret key | `demo` |
| `FERROTICK_HOME` | Data directory | `~/.ferrotick` |

**Example:**

```bash
# Set multiple API keys
export FERROTICK_POLYGON_API_KEY=pk_live_xxx
export FERROTICK_ALPHAVANTAGE_API_KEY=your_key_here
```

### Source Selection

Control provider selection with the `--source` flag:

```bash
# Automatic selection with fallback (default)
ferrotick quote AAPL --source auto

# Use a specific provider
ferrotick quote AAPL --source polygon

# Available options: auto, yahoo, polygon, alphavantage, alpaca
```

---

## 📖 Documentation

- 📋 [Roadmap](docs/ROADMAP.md) - Full project roadmap and technical specifications
- 📄 [RFCs](docs/rfcs/) - Design documents and proposals
- 📚 [API Documentation](https://docs.rs/ferrotick) - Rust API documentation on docs.rs
- 🔒 [Security Policy](SECURITY.md) - Security guidelines and vulnerability reporting
- 🤝 [Contributing Guide](CONTRIBUTING.md) - How to contribute to Ferrotick

---

## 📁 Project Structure

```
ferrotick/
├── crates/
│   ├── ferrotick-core/       # Core domain types, adapters, routing
│   ├── ferrotick-cli/        # Command-line interface
│   └── ferrotick-warehouse/  # DuckDB storage layer
├── docs/
│   ├── ROADMAP.md            # Project roadmap
│   └── rfcs/                 # Design documents
├── schemas/
│   └── v1/                   # JSON schemas for output validation
├── tests/
│   └── contract/             # Integration tests
├── examples/                 # Usage examples (see below)
├── Cargo.toml                # Workspace configuration
├── CONTRIBUTING.md           # Contribution guidelines
├── SECURITY.md               # Security policy
└── LICENSE                   # MIT License
```

---

## 🧪 Testing

```bash
# Run all tests
cargo test --all

# Run with verbose output
cargo test --all -- --nocapture

# Run specific test suite
cargo test -p ferrotick-core

# Run documentation tests
cargo test --doc

# Run benchmarks
cargo bench
```

---

## 🔧 Development

### Prerequisites

- Rust 1.83+ (see `rust-toolchain.toml`)
- Git

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check code without building
cargo check --all

# Run clippy linter
cargo clippy --all -- -D warnings

# Format code
cargo fmt --all -- --check
```

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `2` | Validation/command input error |
| `3` | Provider/network failure |
| `4` | Serialization/schema failure |
| `5` | Partial result (strict mode) |
| `10` | Internal I/O/runtime error |

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

**Quick Start:**

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests (`cargo test --all`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

---

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
MIT License

Copyright (c) 2024 Ferrotick Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
```

---

## 🙏 Acknowledgments

- Inspired by excellent Rust CLI tools like [ripgrep](https://github.com/BurntSushi/ripgrep), [fd](https://github.com/sharkdp/fd), and [starship](https://github.com/starship/starship)
- Built with [Tokio](https://tokio.rs/), [DuckDB](https://duckdb.org/), and [Clap](https://docs.rs/clap/)
- Data providers: [Polygon.io](https://polygon.io/), [Yahoo Finance](https://finance.yahoo.com/), [Alpha Vantage](https://www.alphavantage.co/), [Alpaca](https://alpaca.markets/)

---

<p align="center">
  Made with ❤️ and 🦀 by the Ferrotick Contributors
</p>
