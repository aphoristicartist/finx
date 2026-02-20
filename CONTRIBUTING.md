# Contributing to Ferrotick

Thank you for your interest in contributing to Ferrotick! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Code Quality](#code-quality)
- [Submitting Changes](#submitting-changes)
- [Security](#security)

## Code of Conduct

Be respectful, inclusive, and constructive. We welcome contributions from everyone.

## Getting Started

1. Fork the repository
2. Clone your fork locally
3. Create a new branch for your changes

## Development Setup

### Prerequisites

- Rust 1.83 or later (see `rust-toolchain.toml`)
- Git

### Building

```bash
# Build the project
cargo build --all

# Build in release mode
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific tests
cargo test -p ferrotick-core
cargo test -p ferrotick-warehouse
cargo test -p ferrotick-cli
```

## Making Changes

### Code Style

- Follow standard Rust conventions
- Use `cargo fmt` to format your code
- All code must pass `cargo clippy` with strict warnings

### Commit Messages

- Write clear, descriptive commit messages
- Use the imperative mood ("Add feature" not "Added feature")
- Reference issues when applicable

### Documentation

- Add rustdoc comments to public APIs
- Update README.md if adding user-facing features
- Add inline comments for complex logic

## Testing

### Test Requirements

All contributions must include appropriate tests:

- Unit tests for new functionality
- Integration tests for API changes
- Performance tests for critical paths

### Test Coverage

We aim for high test coverage. Ensure your changes don't decrease coverage.

```bash
# Run all tests including doc tests
cargo test --all --doc
```

## Code Quality

### Mandatory Checks

Before submitting, ensure all checks pass:

```bash
# Format code
cargo fmt --all -- --check

# Run clippy with strict warnings
cargo clippy --all -- -D warnings -D clippy::all

# Check for security vulnerabilities
cargo audit

# Build in release mode
cargo build --release
```

### What We Look For

- **Security**: No SQL injection, proper input validation, secure handling of secrets
- **Performance**: Efficient algorithms, no unnecessary allocations
- **Error Handling**: Proper error types, no panics in library code
- **Documentation**: Public APIs must be documented with rustdoc

## Submitting Changes

1. Push your changes to your fork
2. Create a pull request against the main branch
3. Ensure all CI checks pass
4. Wait for code review
5. Address review feedback

### Pull Request Guidelines

- Keep PRs focused on a single change
- Include tests for new functionality
- Update documentation as needed
- Reference related issues

## Security

### Reporting Vulnerabilities

**Do not open public issues for security vulnerabilities.**

Instead, report them privately via:
- GitHub Security Advisories
- Email to the maintainers

### Security Guidelines

- Never commit API keys or secrets
- Use environment variables for sensitive configuration
- Always use parameterized queries for database operations
- Validate and sanitize all user inputs
- Use HTTPS for all external API calls

### Secure Coding Practices

```rust
// GOOD: Parameterized query
let params: [&dyn ToSql; 2] = [&user_symbol, &price];
connection.execute("INSERT INTO data (symbol, price) VALUES (?, ?)", params.as_slice())?;

// BAD: String interpolation (SQL injection risk)
let sql = format!("INSERT INTO data (symbol) VALUES ('{}')", user_symbol);
```

## Project Structure

```
ferrotick/
├── crates/
│   ├── ferrotick-core/      # Core domain logic and adapters
│   ├── ferrotick-warehouse/ # Database layer (DuckDB)
│   └── ferrotick-cli/       # Command-line interface
├── tests/
│   └── contract/            # Integration tests
├── docs/                    # Documentation
└── schemas/                 # JSON schemas
```

## Questions?

- Open an issue for bugs or feature requests
- Check existing issues before creating new ones

Thank you for contributing to Ferrotick!
