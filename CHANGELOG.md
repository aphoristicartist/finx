# 📋 Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- TBD

---

## [0.1.0] - 2024-02-20

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
| 0.1.0 | 2024-02-20 | Initial release |

---

[Unreleased]: https://github.com/ferrotick/ferrotick/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ferrotick/ferrotick/releases/tag/v0.1.0
