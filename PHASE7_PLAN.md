# Phase 7: Performance Hardening and Release

## Overview

Phase 7 focuses on performance optimization, benchmarking infrastructure, and release preparation for finx.

## Status: Planning

- **Depends on**: Phase 6 (AI-Agent UX and streaming) - COMPLETE
- **Target**: Weeks 15-16

## Tasks

### 1. Startup Performance
- [ ] Profile startup path with `tracing` and `cargo flamegraph`
- [ ] Identify and remove lazy initialization bottlenecks
- [ ] Minimize dependency tree at startup
- [ ] Target: p50 < 100ms, p95 < 140ms

### 2. Binary Size Optimization
- [ ] Add feature flags to minimize default binary size
- [ ] Evaluate `strip` and `LTO` settings in `Cargo.toml`
- [ ] Consider `panic = "abort"` for release builds
- [ ] Document feature flag combinations

### 3. JSON Parsing Optimization
- [ ] Add optional `simd-json` feature for fast JSON parsing
- [ ] Benchmark `serde_json` vs `simd-json` on typical payloads
- [ ] Implement fallback path for unsupported platforms
- [ ] Add feature flag documentation

### 4. HTTP Connection Optimization
- [ ] Tune HTTP connection pooling settings
- [ ] Configure DNS caching appropriately
- [ ] Evaluate keep-alive settings
- [ ] Test under concurrent load

### 5. Benchmarking Infrastructure
- [ ] Add `criterion` benchmarks for:
  - Startup time
  - Quote command (cache hit/miss)
  - Bars parsing (10k records)
  - DuckDB queries (1M rows)
- [ ] Set up regression thresholds in CI
- [ ] Document baseline performance numbers
- [ ] Target: No > 10% regression without approval

### 6. Release Automation
- [ ] Set up cross-compilation for target platforms:
  - Linux x86_64 (glibc and musl)
  - macOS x86_64 and aarch64
  - Windows x86_64
- [ ] Generate checksums (SHA256)
- [ ] Generate SBOM (Software Bill of Materials)
- [ ] Set up signed binary releases
- [ ] Create GitHub release workflow

## Performance SLOs

| Metric | Target p50 | Target p95 |
|--------|-----------|-----------|
| `finx --help` startup | < 100ms | < 140ms |
| `finx quote AAPL` (cached) | < 40ms | < 80ms |
| Parse 10k bars | < 20ms | - |
| DuckDB query (1M rows) | < 150ms | < 300ms |

## Memory Targets

| State | Target RSS |
|-------|-----------|
| Idle | < 35MB |
| Simple quote command | < 80MB |

## Reliability Targets

- Source routing fallback success > 99%
- Zero corrupted manifest rows in crash recovery tests

## Deliverables

1. Release candidate binaries
2. Benchmark evidence report
3. Performance regression tests in CI
4. Release automation workflow

## Dependencies

- No new runtime dependencies
- Dev dependencies: `criterion`, `cargo-flamegraph`, `hyperfine`

## Risks

| Risk | Mitigation |
|------|------------|
| Platform-specific SIMD support | Fallback to serde_json |
| Cross-compilation complexity | Focus on primary platforms first |
| Performance regression in dependencies | Pin dependency versions, regular audits |

## Next Steps

1. Set up benchmark harness
2. Profile current startup path
3. Identify top 3 bottlenecks
4. Implement targeted fixes
5. Validate against SLOs
