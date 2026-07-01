# Reference: Loki Protocol Surface

Use this page when log traffic should be interpreted as Loki ingest, query,
tail, metadata, or ruler intent rather than only generic HTTP traffic.

Default entry: `push`

Protocol aliases: `loki`, `loki-push`, `loki_push`, `log-push`,
`logs-push`, `loki-query`, `loki_query`, `logql`, `loki-range-query`,
`loki-tail`, `loki_tail`, `log-tail`, `loki-labels`, `loki_labels`,
`loki-series`, `loki-rules`, `loki_rules`, `loki-ruler`

## What This Shelf Covers

The current Loki family models five debugger-facing paths:

- log batch push requests
- LogQL query and range-query requests
- live tail streams
- label and series metadata reads
- ruler API traffic for log-derived alerting rules

This is not a LogQL evaluator or compressed payload decoder. The 0.18.x
behavior is to identify log ingest, read, metadata, and ruler posture without
making payload decoding a dependency.

## Loki Surface Map

### Log Ingest

- [docs/book/reference-loki-log-ingest-surface.md](docs/book/reference-loki-log-ingest-surface.md)
  Log push and write response posture.

Typical entries:

- `push`

### Log Query And Metadata

- [docs/book/reference-loki-log-read-surface.md](docs/book/reference-loki-log-read-surface.md)
  Query, live tail, label, and series metadata traffic.

Typical entries:

- `query`
- `tail`
- `labels`

### Ruler

- [docs/book/reference-loki-ruler-surface.md](docs/book/reference-loki-ruler-surface.md)
  Loki ruler API reads and mutations.

Typical entries:

- `rules`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-loki-surface.md](docs/book/reference-loki-surface.md)
3. one narrower Loki subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
