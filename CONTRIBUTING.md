# 🤝 Contributing to Ferrotick

First off, thank you for considering contributing to Ferrotick! It's people like you that make Ferrotick such a great tool.

---

## 📑 Table of Contents

- [Code of Conduct](#-code-of-conduct)
- [🚀 Getting Started](#-getting-started)
  - [Prerequisites](#prerequisites)
  - [Fork and Clone](#fork-and-clone)
  - [Build and Test](#build-and-test)
- [🔄 Development Workflow](#-development-workflow)
  - [Branch Naming](#branch-naming)
  - [Commit Messages](#commit-messages)
  - [Pull Request Process](#pull-request-process)
- [📝 Code Style Guidelines](#-code-style-guidelines)
- [🧪 Testing Requirements](#-testing-requirements)
- [📚 Documentation](#-documentation)
- [🔒 Security](#-security)
- [❓ Questions?](#-questions)

---

## 💜 Code of Conduct

This project and everyone participating in it is governed by our commitment to being **respectful, inclusive, and constructive**. We welcome contributions from everyone, regardless of experience level, background, or identity.

### Our Standards

- ✅ Using welcoming and inclusive language
- ✅ Being respectful of differing viewpoints and experiences
- ✅ Gracefully accepting constructive criticism
- ✅ Focusing on what is best for the community
- ✅ Showing empathy towards other community members

### Unacceptable Behavior

- ❌ Trolling, insulting comments, and personal attacks
- ❌ Public or private harassment
- ❌ Publishing others' private information without permission
- ❌ Other conduct that would be inappropriate in a professional setting

---

## 🚀 Getting Started

### Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| 🦀 Rust | 1.83+ | See `rust-toolchain.toml` |
| 📦 Cargo | Included with Rust | |
| 🐙 Git | Any recent version | |

### Fork and Clone

```bash
# 1. Fork the repository on GitHub
# 2. Clone your fork
git clone https://github.com/YOUR_USERNAME/ferrotick.git
cd ferrotick

# 3. Add upstream remote
git remote add upstream https://github.com/ferrotick/ferrotick.git

# 4. Verify remotes
git remote -v
```

### Build and Test

```bash
# Build the project
cargo build --all

# Run all tests
cargo test --all

# Run with verbose output
cargo test --all -- --nocapture

# Build in release mode (optimized)
cargo build --release
```

---

## 🔄 Development Workflow

### Branch Naming

Use descriptive branch names with prefixes:

| Prefix | Purpose | Example |
|--------|---------|---------|
| `feature/` | New features | `feature/add-new-provider` |
| `fix/` | Bug fixes | `fix/quote-parsing-error` |
| `docs/` | Documentation | `docs/update-readme` |
| `refactor/` | Code refactoring | `refactor/simplify-routing` |
| `test/` | Adding tests | `test/add-integration-tests` |
| `chore/` | Maintenance | `chore/update-dependencies` |

```bash
# Create a new branch
git checkout -b feature/your-amazing-feature
```

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

**Types:**

| Type | Description |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation changes |
| `style` | Code style changes (formatting, etc.) |
| `refactor` | Code refactoring |
| `perf` | Performance improvements |
| `test` | Adding or updating tests |
| `chore` | Maintenance tasks |
| `ci` | CI/CD changes |

**Examples:**

```bash
# Good commit messages
feat(adapters): add support for IEX Cloud provider
fix(warehouse): resolve SQL injection vulnerability in query parser
docs(readme): add installation instructions for Windows
test(core): add unit tests for interval parsing
```

### Pull Request Process

1. **Ensure all checks pass:**

   ```bash
   # Format code
   cargo fmt --all -- --check
   
   # Run clippy with strict warnings
   cargo clippy --all -- -D warnings -D clippy::all
   
   # Run all tests
   cargo test --all
   
   # Check for security vulnerabilities
   cargo audit
   ```

2. **Update documentation:**
   - Add rustdoc comments to public APIs
   - Update README.md if adding user-facing features
   - Add examples for new functionality

3. **Create the PR:**
   - Push to your fork
   - Open a PR against `main` branch
   - Fill out the PR template

4. **PR Review:**
   - Wait for CI checks to pass
   - Address review feedback
   - Keep the PR up to date with `main`

5. **Merge:**
   - Squash and merge is the default
   - Maintainers will handle the merge

---

## 📝 Code Style Guidelines

### Formatting

We use `rustfmt` with default settings:

```bash
# Format all code
cargo fmt --all

# Check formatting without making changes
cargo fmt --all -- --check
```

### Linting

We use `clippy` with strict warnings:

```bash
# Run clippy
cargo clippy --all -- -D warnings -D clippy::all
```

### Code Organization

```
crates/
├── ferrotick-core/       # Domain types, traits, adapters
├── ferrotick-cli/        # CLI commands and output
└── ferrotick-warehouse/  # Storage layer
```

### Best Practices

| ✅ Do | ❌ Don't |
|-------|----------|
| Use descriptive variable names | Use single-letter names (except iterators) |
| Add documentation to public APIs | Leave public items undocumented |
| Handle errors appropriately | Use `unwrap()` in production code |
| Write tests for new features | Skip tests to save time |
| Use `Result` for fallible operations | Panic on expected failures |
| Keep functions focused | Write long, complex functions |

### Error Handling

```rust
// ✅ GOOD: Proper error handling
pub fn parse_symbol(input: &str) -> Result<Symbol, ValidationError> {
    let normalized = input.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return Err(ValidationError::EmptySymbol);
    }
    Ok(Symbol::new(normalized))
}

// ❌ BAD: Panics on invalid input
pub fn parse_symbol(input: &str) -> Symbol {
    Symbol::new(input.trim().to_ascii_uppercase()) // Could panic!
}
```

### SQL Safety

```rust
// ✅ GOOD: Parameterized query
let params: [&dyn ToSql; 2] = [&user_symbol, &price];
connection.execute(
    "INSERT INTO data (symbol, price) VALUES (?, ?)",
    params.as_slice()
)?;

// ❌ BAD: String interpolation (SQL injection risk!)
let sql = format!("INSERT INTO data (symbol) VALUES ('{}')", user_symbol);
```

---

## 🧪 Testing Requirements

All contributions must include appropriate tests:

### Test Categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit Tests | `src/**/tests.rs` or inline `#[cfg(test)]` | Test individual functions |
| Integration Tests | `tests/` directory | Test component interactions |
| Doc Tests | Rustdoc comments | Ensure examples compile |
| Property Tests | `proptest` crate | Test invariants |

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific test suite
cargo test -p ferrotick-core

# Run doc tests
cargo test --doc

# Run with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Test Quality Guidelines

```rust
// ✅ GOOD: Clear test with descriptive name
#[test]
fn rejects_negative_price_in_quote() {
    let result = Quote::new(
        Symbol::new("AAPL"),
        -100.0,  // Negative price
        None, None, None,
        "USD",
        UtcDateTime::now(),
    );
    assert!(matches!(result, Err(ValidationError::NegativeValue { .. })));
}

// ❌ BAD: Unclear test purpose
#[test]
fn test_quote() {
    let q = Quote::new(/* ... */);
    assert!(q.is_ok());
}
```

---

## 📚 Documentation

### Rustdoc Comments

All public items must have rustdoc comments:

```rust
/// Parse a stock symbol from a string.
///
/// # Arguments
///
/// * `input` - The input string to parse
///
/// # Returns
///
/// A validated `Symbol` on success, or a `ValidationError` on failure.
///
/// # Examples
///
/// ```
/// use ferrotick_core::Symbol;
///
/// let symbol = Symbol::parse("aapl")?;
/// assert_eq!(symbol.as_str(), "AAPL");
/// # Ok::<(), ferrotick_core::ValidationError>(())
/// ```
pub fn parse(input: &str) -> Result<Symbol, ValidationError> {
    // Implementation...
}
```

### What to Document

| Item | Documentation Required |
|------|------------------------|
| Public structs | ✅ Yes - purpose and usage |
| Public enums | ✅ Yes - variants and when to use |
| Public functions | ✅ Yes - parameters, returns, examples |
| Public traits | ✅ Yes - purpose and implementors |
| Safety requirements | ✅ Yes - `# Safety` section |
| Panics | ✅ Yes - `# Panics` section |
| Errors | ✅ Yes - `# Errors` section |

---

## 🔒 Security

### Reporting Vulnerabilities

**Do NOT open public issues for security vulnerabilities.**

Instead, report them privately via:
- GitHub Security Advisories
- Email to the maintainers

See [SECURITY.md](SECURITY.md) for the full security policy.

### Security Guidelines

| ✅ Do | ❌ Don't |
|-------|----------|
| Use environment variables for secrets | Commit API keys to version control |
| Use parameterized queries | Use string interpolation for SQL |
| Validate and sanitize inputs | Trust user input |
| Use HTTPS for external requests | Use unencrypted connections |
| Run `cargo audit` regularly | Ignore dependency vulnerabilities |

---

## ❓ Questions?

- 🐛 **Bug reports:** [Open an issue](https://github.com/ferrotick/ferrotick/issues/new?template=bug_report.md)
- 💡 **Feature requests:** [Open an issue](https://github.com/ferrotick/ferrotick/issues/new?template=feature_request.md)
- 💬 **Questions:** [Start a discussion](https://github.com/ferrotick/ferrotick/discussions)

---

<p align="center">
  Thank you for contributing to Ferrotick! 🦀
</p>
