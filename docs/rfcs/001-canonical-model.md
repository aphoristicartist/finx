# RFC-001: Canonical Data Model

- Status: Accepted
- Authors: Platform Engineering
- Created: 2026-02-16
- Phase: 0

## Context

`finx` aggregates multiple providers that use different field names, numeric precision rules, and nullability conventions. Without a single canonical model, downstream cache, schema, and CLI contracts drift quickly.

## Decision

Adopt a provider-neutral canonical model that all adapters normalize into before data is cached, queried, or emitted.

### Core Entities

1. `Instrument`
- Identity: `symbol` (normalized uppercase), optional metadata (`name`, `exchange`, `asset_class`, `currency`, `is_active`)

2. `Quote`
- Top-of-book snapshot: `symbol`, `price`, optional `bid`, `ask`, optional `volume`, `currency`, `as_of`

3. `Bar`
- OHLCV record: `ts`, `open`, `high`, `low`, `close`, optional `volume`, optional `vwap`
- Grouped in `BarSeries { symbol, interval, bars[] }`

4. `Fundamental`
- Snapshot metrics: `symbol`, `as_of`, optional `market_cap`, optional `pe_ratio`, optional `dividend_yield`

5. `CorporateAction`
- Event: `symbol`, `action_type`, `ex_date`, optional `pay_date`, optional `value`, optional `currency`

## Contract Rules

1. Symbol normalization
- Symbols are trimmed and uppercased.
- Symbols must start with an ASCII letter.
- Allowed characters: `A-Z`, `0-9`, `.` and `-`.
- Maximum length: 15 characters.

2. Interval grammar
- Supported intervals: `1m`, `5m`, `15m`, `1h`, `1d`.
- Unrecognized intervals are validation errors.

3. Timestamp handling
- All timestamps are RFC3339 with explicit UTC (`Z`) offset.
- Non-UTC timestamps are rejected at validation boundaries.

4. Numeric handling
- Missing numeric values are `null`.
- NaN and Infinity are invalid.
- Negative values are rejected for fields defined as non-negative (e.g., prices, volume-like metrics, market cap).

5. Currency normalization
- Currency codes are normalized to 3-letter uppercase ASCII (for example `USD`).

## Error Strategy

Validation failures use typed error variants (`ValidationError`) instead of free-form strings. This allows:
- deterministic exit-code mapping in the CLI,
- stable machine-readable error payloads,
- and explicit test assertions.

## Consequences

Positive:
- Uniform serialization contract across providers.
- Easier schema evolution under versioned envelopes.
- Reduced downstream branching in cache and SQL layers.

Tradeoffs:
- Provider-specific fields outside canonical entities require explicit extension points.
- Strict UTC enforcement rejects payloads that might otherwise be coercible.

## Follow-up

- Add per-entity property tests in Phase 1.
- Add JSON schema golden fixtures for canonical entities in Phase 1.
