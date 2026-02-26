# 📋 Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- TBD

---

## [0.2.0] - 2026-02-26

### ✨ Added

#### Financial Data Commands
- **`financials` command** - Fetch financial statements (income, balance sheet, cash flow)
  - Support for annual and quarterly periods
  - Full line-item extraction with canonical aliases
  - Automatic free cash flow calculation
  - DuckDB warehouse integration with `financials` table
- **`earnings` command** - Fetch earnings history and upcoming earnings dates
  - EPS actual vs estimate comparison
  - Surprise percentage calculation
  - Support for up to 8 historical quarters
  - DuckDB warehouse integration with `earnings` table

#### Enhanced Fundamentals
- Extended `fundamentals` command with additional metrics:
  - Basic and diluted shares outstanding
  - Forward P/E and PEG ratio
  - Price-to-book and price-to-sales ratios
  - Enterprise value and EV/EBITDA
  - Gross, operating, and net margins
  - Return on equity (ROE) and return on assets (ROA)

#### Warehouse Improvements
- New migration (v3) for `financials` and `earnings` tables
- Warehouse sync support for financial statements and earnings data
- Enhanced fundamentals table with new metric columns

### 🔧 Changed
- **Removed mock mode** - All adapters now use real API calls exclusively
- Yahoo adapter enhanced to fetch comprehensive financial data via quoteSummary API
- Improved error handling for missing/null financial data fields

### 📚 Documentation
- Updated README with financials and earnings usage examples
- Added quick start examples for new commands
- Updated capability matrix to reflect financials/earnings support (Yahoo only)

---

## [0.1.0] - 2026-02-20

### 🎉 Initial Release

The first production-ready release of Ferrotick!

### ✨ Added

#### Core Features
- **Multi-provider support** for market data:
  - Polygon.io adapter with full API support
  - Yahoo Finance adapter
  - Alpha Vantage adapter
  - Alpaca adapter
- **DuckDB warehouse** for local analytics
  - Connection pooling
  - Query guardrails (timeout, row limits)
  - Pre-built views for common queries
- **CLI commands**:
  - `quote` - Fetch real-time/delayed quotes
  - `bars` - Fetch historical OHLCV data
  - `fundamentals` - Fetch company fundamentals
  - `search` - Search for instruments
  - `sql` - Execute SQL against the warehouse
  - `cache sync` - Sync local parquet files
  - `schema` - Inspect JSON schemas
  - `sources` - List provider capabilities

#### Security
- Parameterized SQL queries to prevent injection
- API keys from environment variables only
- TLS encryption for all external requests
- Path validation for schema files

#### Developer Experience
- Structured JSON output with envelope format
- Multiple output formats (JSON, NDJSON, Table)
- `--strict` mode for CI/CD pipelines
- Comprehensive error codes and messages
- AI-agent streaming mode with `--stream`

#### Infrastructure
- Circuit breaker for resilient upstream calls
- Rate limiting with token bucket algorithm
- Smart source routing with fallback
- Provider priority scoring

### 📚 Documentation
- Complete README with badges and examples
- Contributing guidelines
- Security policy
- Rustdoc for all public APIs
- Example programs in `examples/` directory

### 🧪 Testing
- Unit tests for all core modules
- Integration tests for adapters
- SQL injection tests for warehouse
- Performance benchmarks

---

## Version History Summary

| Version | Date | Description |
|---------|------|-------------|
| 0.2.0 | 2026-02-26 | Financial statements, earnings data, enhanced fundamentals |
| 0.1.0 | 2026-02-20 | Initial release |

---

[Unreleased]: https://github.com/ferrotick/ferrotick/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ferrotick/ferrotick/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ferrotick/ferrotick/releases/tag/v0.1.0
