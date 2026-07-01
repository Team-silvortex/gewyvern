# Reference: Loki Log Ingest Surface

The Loki log-ingest surface tracks log batch pushes from agents, bridges, and
services into a Loki-compatible receiver.

Family hub: [Loki surface](docs/book/reference-loki-surface.md)

Canonical entries: `push`

## Debugging Focus

- Agent or bridge process lineage.
- Receiver route selection.
- Push response status and retry posture.
- Distinguishing log ingest failures from later query or label issues.

## Typical Question

Use this surface when logs are emitted locally but never appear in Loki.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `ingest`
- `ingestion`
- `log-push`
- `logs-push`
- `loki`
- `loki-push`
- `loki_push`
- `push-logs`
- `push_logs`

<!-- gewyvern:entry-aliases:end -->
