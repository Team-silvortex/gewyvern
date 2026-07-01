# Reference: Prometheus Query Surface

The Prometheus query surface tracks HTTP API calls that read time-series data
through instant queries, range queries, and compatible query frontends.

Family hub: [Prometheus surface](docs/book/reference-prometheus-surface.md)

Canonical entries: `query`

## Debugging Focus

- Query API reachability and response status.
- Query frontend route/process lineage.
- Distinguishing query failures from scrape or storage failures.

## Typical Question

Use this surface when PromQL results look empty, stale, or inconsistent with
the metrics collection path.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `api-query`
- `instant-query`
- `instant_query`
- `prom-query`
- `prometheus-query`
- `prometheus_query`
- `promql`
- `query-range`
- `query_range`

<!-- gewyvern:entry-aliases:end -->
