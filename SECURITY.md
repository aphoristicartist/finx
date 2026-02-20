# Security Policy

## Supported Versions

We actively support the latest version of Ferrotick with security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security issue, please report it responsibly:

### How to Report

1. **Email**: Send details to the maintainers via GitHub's private vulnerability reporting feature
2. **GitHub Security Advisory**: Use [GitHub's security advisory feature](https://github.com/ferrotick/ferrotick/security/advisories) (if available)
3. **Do NOT** open a public issue for security vulnerabilities

### What to Include

When reporting, please include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Triage & Assessment**: Within 5 business days
- **Fix Development**: Depends on severity and complexity
- **Disclosure**: After fix is released, following responsible disclosure practices

## Security Best Practices

When using Ferrotick:

1. **API Keys**: Store API keys in environment variables, never in code
   - Use `YAHOO_API_KEY`, `POLYGON_API_KEY`, etc.
   - Never commit API keys to version control

2. **Database Security**: 
   - Use appropriate file permissions for DuckDB files
   - Consider encryption at rest for sensitive data

3. **Network Security**:
   - Use HTTPS for all external API calls
   - Validate and sanitize all user inputs

4. **Dependency Security**:
   - Run `cargo audit` regularly
   - Keep dependencies up to date
   - Review dependency updates for security fixes

## Known Security Considerations

### Data Provider APIs

Ferrotick integrates with external data providers (Yahoo Finance, Polygon, etc.):
- API keys are passed via HTTP headers (HTTPS encrypted)
- No sensitive user data is sent to third parties
- Market data is public information

### Local Storage

- DuckDB database files stored locally at `~/.ferrotick/cache/`
- Contains only market data (no personally identifiable information)
- User is responsible for securing their local filesystem

## Security Updates

Security updates will be released as patch versions and announced via:
- GitHub Releases
- CHANGELOG.md

## Contact

For security concerns, contact the maintainers through GitHub's security features.
