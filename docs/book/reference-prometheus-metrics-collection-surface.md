# Reference: Prometheus Metrics Collection Surface

The Prometheus metrics-collection surface tracks scrape requests and
remote-write batches across exporters, Prometheus servers, and compatible
receivers.

Family hub: [Prometheus surface](docs/book/reference-prometheus-surface.md)

Canonical entries: `scrape`, `remote-write`

## Debugging Focus

- Scrape target reachability and response posture.
- Exporter route/process lineage.
- Remote-write sender and receiver selection.
- Write batch response status and retry posture.

## Typical Question

Use this surface when metrics are missing, stale, scraped from the wrong target,
or successfully collected but not reaching long-term storage.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `metrics`
- `metrics-endpoint`
- `metrics-scrape`
- `prom`
- `prom-remote-write`
- `prom-scrape`
- `prom_remote_write`
- `prometheus`
- `prometheus-remote-write`
- `prometheus-scrape`
- `prometheus_remote_write`
- `remote_write`
- `samples`
- `scrape-target`
- `target-scrape`
- `write`
- `write-batch`

<!-- gewyvern:entry-aliases:end -->
