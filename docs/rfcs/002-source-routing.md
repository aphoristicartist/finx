# RFC-002: Source Routing Strategy

- Status: Accepted
- Authors: Platform Engineering
- Created: 2026-02-16
- Phase: 0

## Context

Different providers vary by endpoint coverage, quota, latency, and reliability. Calling one fixed provider by default is fragile and makes uptime dependent on the weakest upstream.

## Decision

Introduce explicit source strategy semantics at the API boundary:

1. `auto`
- Router selects provider chain based on capability, health, and policy.
- Router may fallback across providers.

2. `priority list`
- User-specified ordered providers are attempted in sequence.
- First successful provider wins.

3. `strict source`
- Single provider is called.
- No fallback occurs.

## Routing Inputs

The router evaluates:
- endpoint capability matrix,
- recent health status,
- quota/rate-limit availability,
- configured timeout budget,
- optional cost policy.

## Routing Outputs

Every envelope must expose:
- `source_chain`: attempted providers in deterministic order,
- `latency_ms`: end-to-end request latency,
- `cache_hit`: cache path indicator.

If a fallback occurs, the output still conforms to the same canonical data schema.

## Failure Semantics

- If all providers fail in `auto`/priority mode, return structured errors for each failed attempt where possible.
- In strict mode, a provider failure is terminal.
- Retryability is explicit per error object.

## Phase 0-1 Implementation Scope

Phase 0-1 only introduces the strategy surface and metadata fields.
- The CLI accepts `--source auto|yahoo|polygon|alphavantage|alpaca`.
- `auto` is stubbed to deterministic placeholder behavior until adapter routing is implemented.

## Consequences

Positive:
- Stable UX regardless of provider-specific outages.
- Strong observability for AI and automation clients.

Tradeoffs:
- Router policy increases implementation complexity.
- Deterministic source ordering requires careful policy design.

## Follow-up

- Implement capability registry and scoring in Phase 2.
- Add integration tests for fallback and strict-mode behavior in Phase 2.
