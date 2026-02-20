# 🔒 Security Policy

We take security vulnerabilities seriously. This document outlines our security practices and how to report vulnerabilities.

---

## 📑 Table of Contents

- [🛡️ Supported Versions](#️-supported-versions)
- [🚨 Reporting Vulnerabilities](#-reporting-vulnerabilities)
- [⏱️ Response Timeline](#️-response-timeline)
- [✅ Security Best Practices](#-security-best-practices)
- [🔐 Known Security Considerations](#-known-security-considerations)
- [📦 Dependency Security](#-dependency-security)
- [📢 Security Updates](#-security-updates)

---

## 🛡️ Supported Versions

We actively support the following versions with security updates:

| Version | Supported | Status |
|:-------:|:---------:|:-------|
| `0.1.x` | ✅ | **Active Development** |
| `< 0.1` | ❌ | Not supported |

> **Note:** As we approach v1.0, we will establish a formal LTS (Long-Term Support) policy.

---

## 🚨 Reporting Vulnerabilities

**⛔ Do NOT open public issues for security vulnerabilities.**

### How to Report

| Method | Use When |
|--------|----------|
| [GitHub Security Advisories](https://github.com/ferrotick/ferrotick/security/advisories) | Preferred method for all vulnerabilities |
| Email maintainers | If GitHub is unavailable |

### What to Include

When reporting a vulnerability, please provide:

| Information | Required | Description |
|-------------|:--------:|-------------|
| Vulnerability description | ✅ | Clear description of the issue |
| Affected versions | ✅ | Which version(s) are affected |
| Steps to reproduce | ✅ | How to trigger the vulnerability |
| Proof of concept | ⚠️ | Optional but helpful |
| Impact assessment | ✅ | What can an attacker achieve |
| Suggested fix | ⚠️ | Optional but appreciated |

### Example Report

```markdown
## Vulnerability: SQL Injection in Query Parser

**Affected versions:** 0.1.0 - 0.1.5

**Description:**
The query parser does not properly escape user input when constructing
SQL queries, allowing arbitrary SQL execution.

**Steps to reproduce:**
1. Run `ferrotick sql "SELECT * FROM data WHERE symbol = '${MALICIOUS_INPUT}'"`
2. Observe that arbitrary SQL can be executed

**Impact:**
An attacker could read, modify, or delete data in the local warehouse.

**Suggested fix:**
Use parameterized queries for all user input.
```

---

## ⏱️ Response Timeline

We are committed to responding to security issues promptly:

| Phase | Timeline | SLA |
|-------|----------|-----|
| 📥 **Initial Response** | Within 48 hours | Acknowledge receipt |
| 🔍 **Triage & Assessment** | Within 5 business days | Severity classification |
| 🔧 **Fix Development** | Depends on severity | See below |
| ✅ **Patch Release** | After verification | Coordinated disclosure |

### Severity-Based Response

| Severity | Description | Target Fix Time |
|:--------:|-------------|-----------------|
| 🔴 **Critical** | Remote code execution, data breach | 24-48 hours |
| 🟠 **High** | Significant security bypass | 3-5 days |
| 🟡 **Medium** | Limited security impact | 1-2 weeks |
| 🟢 **Low** | Minor security improvement | Next release |

### Disclosure Policy

We follow **responsible disclosure**:

1. Reporter reports vulnerability privately
2. We acknowledge and assess within 48 hours
3. We develop and test a fix
4. We release the fix and update CHANGELOG.md
5. After 30 days, we publish the advisory (or sooner if already public)

---

## ✅ Security Best Practices

When using Ferrotick, follow these security best practices:

### 🔑 API Key Management

```bash
# ✅ GOOD: Use environment variables
export FERROTICK_POLYGON_API_KEY=your_key_here

# ❌ BAD: Never commit keys to version control
# config.txt:
# POLYGON_API_KEY=pk_live_xxx  <-- DON'T DO THIS!
```

| ✅ Do | ❌ Don't |
|-------|----------|
| Use environment variables | Commit keys to git |
| Use `.env` files with `.gitignore` | Share keys in chat/email |
| Rotate keys regularly | Use demo keys in production |
| Use separate keys per environment | Use production keys in development |

### 🗄️ Database Security

```bash
# Set appropriate permissions on data directory
chmod 700 ~/.ferrotick
chmod 600 ~/.ferrotick/cache/warehouse.duckdb
```

| Practice | Description |
|----------|-------------|
| File permissions | Restrict access to `~/.ferrotick/` |
| Encryption | Consider encrypting data at rest for sensitive data |
| Backups | Regular backups of warehouse data |
| Access control | Limit who can run ferrotick commands |

### 🌐 Network Security

| Practice | Description |
|----------|-------------|
| TLS | All external API calls use HTTPS (enforced) |
| Certificate validation | TLS certificates are validated |
| No sensitive data in URLs | API keys go in headers, not URLs |

### 📝 Input Validation

```rust
// Ferrotick validates all inputs:
// - Symbol names are sanitized
// - SQL uses parameterized queries
// - File paths are canonicalized
```

---

## 🔐 Known Security Considerations

### Data Provider APIs

Ferrotick integrates with external data providers:

| Provider | Security Notes |
|----------|----------------|
| Polygon.io | API keys passed via HTTP headers (TLS encrypted) |
| Yahoo Finance | API keys passed via HTTP headers (TLS encrypted) |
| Alpha Vantage | API keys passed as query params (TLS encrypted) |
| Alpaca | Dual key authentication via headers (TLS encrypted) |

**Key Points:**
- ✅ All requests use HTTPS/TLS
- ✅ API keys are never logged
- ✅ No personally identifiable information sent to providers
- ✅ Market data is public information

### Local Storage

| Location | Contents | Sensitivity |
|----------|----------|-------------|
| `~/.ferrotick/cache/` | DuckDB warehouse | Market data (public) |
| `~/.ferrotick/cache/parquet/` | Parquet files | Market data (public) |
| Environment variables | API keys | **High** - protect these! |

**Recommendations:**
- The warehouse contains only public market data
- No PII (Personally Identifiable Information) is stored
- User is responsible for filesystem security

### SQL Query Execution

```bash
# Read-only mode (default) - safe for user queries
ferrotick sql "SELECT * FROM bars_1d"

# Write mode - use with caution
ferrotick sql "DELETE FROM bars_1d" --write  # Requires --write flag
```

**Safety Features:**
- Read-only mode is the default
- Write operations require explicit `--write` flag
- Query timeout prevents runaway queries
- Row limits prevent memory exhaustion

---

## 📦 Dependency Security

We actively monitor and update dependencies for security vulnerabilities.

### Automated Checks

```bash
# Run security audit locally
cargo audit

# Check for outdated dependencies
cargo outdated
```

| Check | Frequency | Tool |
|-------|-----------|------|
| Vulnerability audit | Every PR | GitHub Dependabot |
| Dependency updates | Weekly | Cargo dependencies |
| License compliance | Every PR | `cargo deny` |

### Supply Chain Security

| Measure | Status |
|---------|--------|
| Dependency pinning | ✅ `Cargo.lock` committed |
| Minimal dependencies | ✅ Only essential crates |
| Regular updates | ✅ Monthly dependency review |
| Security advisories | ✅ `cargo audit` in CI |

---

## 📢 Security Updates

Security updates will be announced via:

| Channel | Use |
|---------|-----|
| [GitHub Releases](https://github.com/ferrotick/ferrotick/releases) | Patch notes and security fixes |
| [CHANGELOG.md](CHANGELOG.md) | Detailed change history |
| GitHub Security Advisories | Vulnerability details (after fix) |

### Notification Preferences

To receive security updates:

1. ⭐ Star the repository
2. 👀 Watch releases on GitHub
3. Subscribe to GitHub Security Advisories

---

## 📞 Contact

For security concerns, contact the maintainers:

| Method | Use For |
|--------|---------|
| [GitHub Security Advisories](https://github.com/ferrotick/ferrotick/security/advisories) | Vulnerability reports (preferred) |
| GitHub Issues | Non-security bug reports only |

---

<p align="center">
  Thank you for helping keep Ferrotick secure! 🔒
</p>
