# Reference: Prometheus Protocol Surface

Use this page when metrics traffic should be interpreted as Prometheus scrape,
remote-write, query, or alerting intent rather than only generic HTTP traffic.

Default entry: `scrape`

Protocol aliases: `prometheus`, `prom`, `prom-scrape`,
`prometheus-scrape`, `metrics-scrape`, `prometheus-remote-write`,
`prom-remote-write`, `prometheus_remote_write`, `prom_remote_write`,
`prometheus-query`, `prom-query`, `promql`, `prometheus_query`,
`alertmanager`, `prometheus-alertmanager`, `prom-alertmanager`,
`prometheus-rules`, `prom-rules`, `prometheus-rule-eval`, `prom_rule_eval`

## What This Shelf Covers

The current Prometheus family models five debugger-facing paths:

- scrape requests against exporters and service metrics endpoints
- remote-write batches sent to a compatible metrics receiver
- query and range-query API calls
- Alertmanager notification/API traffic
- rule and alert state reads used to debug evaluation posture

This is not a PromQL evaluator or remote-write protobuf decoder. The 0.18.x
behavior is to identify metrics collection direction, query/control intent, and
alerting path posture without making payload decoding a dependency.

## Prometheus Surface Map

### Metrics Collection

- [docs/book/reference-prometheus-metrics-collection-surface.md](docs/book/reference-prometheus-metrics-collection-surface.md)
  Scrape and remote-write data movement.

Typical entries:

- `scrape`
- `remote-write`

### Query API

- [docs/book/reference-prometheus-query-surface.md](docs/book/reference-prometheus-query-surface.md)
  Query and range-query API calls.

Typical entries:

- `query`

### Alerting

- [docs/book/reference-prometheus-alerting-surface.md](docs/book/reference-prometheus-alerting-surface.md)
  Alertmanager and rule evaluation posture.

Typical entries:

- `alertmanager`
- `rule-eval`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-prometheus-surface.md](docs/book/reference-prometheus-surface.md)
3. one narrower Prometheus subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
