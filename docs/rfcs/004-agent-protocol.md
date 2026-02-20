# RFC-004: JSON Envelope and Streaming Protocol

- Status: Accepted
- Authors: Platform Engineering
- Created: 2026-02-16
- Phase: 0

## Context

`ferrotick` must be first-class for machine consumers, including agents that need stable schemas, deterministic metadata, and stream-friendly responses.

## Decision

All machine-readable command outputs use a strict JSON envelope:

```json
{
  "meta": {
    "request_id": "...",
    "schema_version": "v1.0.0",
    "generated_at": "...",
    "source_chain": ["yahoo"],
    "latency_ms": 0,
    "cache_hit": true,
    "warnings": []
  },
  "data": {},
  "errors": []
}
```

## Envelope Rules

1. `meta` and `data` are required.
2. `request_id` must be non-empty and traceable across logs.
3. `schema_version` follows `vMAJOR.MINOR.PATCH`.
4. `source_chain` is never empty.
5. `warnings` are non-fatal informational diagnostics.
6. `errors` are structured objects with at least `code` and `message`.

## Structured Error Contract

Error objects support:
- `code`: stable programmatic identifier
- `message`: human-readable detail
- `retryable`: optional boolean
- `source`: optional provider identifier

## Streaming Contract (NDJSON)

Event schema supports:
- `start`
- `progress`
- `chunk`
- `end`
- `error`

Each event includes:
- `event`
- `seq`
- `ts`
- optional `meta`
- optional `data`
- required `error` when `event == "error"`

## Strict Mode Behavior

`--strict` treats warnings and errors as command failure (`exit code 5`) while still allowing payload emission for machine diagnostics.

## Phase 0-1 Implementation Scope

Phase 0-1 delivers:
- envelope type in core,
- structured error objects,
- schema files under `schemas/v1`,
- CLI format flags for `json|ndjson|table` with strict-mode gate.

Full streaming runtime behavior is delivered in later phases.

## Consequences

Positive:
- Stable machine contract across commands.
- Better traceability and automated remediation.

Tradeoffs:
- Envelope metadata introduces slight output overhead.
- Strict mode requires consumer awareness to avoid false-positive failures during rollout.

## Follow-up

- Add schema conformance tests against golden fixtures in Phase 1.
- Add streaming implementation and parser soak tests in Phase 6.
