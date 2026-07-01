# Reference: Loki Log Read Surface

The Loki log-read surface tracks LogQL query, live tail, labels, and series
metadata requests.

Family hub: [Loki surface](docs/book/reference-loki-surface.md)

Canonical entries: `query`, `tail`, `labels`

## Debugging Focus

- Query frontend route/process lineage.
- LogQL query and range-query response posture.
- Live tail stream continuity.
- Label and series metadata used to debug selector mismatch.

## Typical Question

Use this surface when logs exist but queries, dashboards, or live tail sessions
do not show the expected streams.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `instant-query`
- `instant_query`
- `label`
- `label-values`
- `label_values`
- `live-tail`
- `live_tail`
- `log-query`
- `log-tail`
- `logql`
- `loki-labels`
- `loki-query`
- `loki-range-query`
- `loki-series`
- `loki-tail`
- `loki_labels`
- `loki_query`
- `loki_tail`
- `metadata`
- `query-range`
- `query_range`
- `series`
- `tail-stream`
- `tail_stream`

<!-- gewyvern:entry-aliases:end -->
