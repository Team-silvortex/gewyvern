# Reference: Prometheus Alerting Surface

The Prometheus alerting surface tracks Alertmanager API traffic and rule or
alert-state reads used to understand evaluation posture.

Family hub: [Prometheus surface](docs/book/reference-prometheus-surface.md)

Canonical entries: `alertmanager`, `rule-eval`

## Debugging Focus

- Alertmanager notification and alert API request posture.
- Rule and alert-state API reads.
- Route/process lineage around alert delivery or rule inspection.

## Typical Question

Use this surface when metrics exist but alerts do not fire, notifications do
not arrive, or rule state differs from operator expectations.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `alert-post`
- `alert-state`
- `alertmanager`
- `alerts`
- `alerts-state`
- `notification`
- `notify`
- `prom-alertmanager`
- `prom-rules`
- `prom_rule_eval`
- `prometheus-alertmanager`
- `prometheus-rule-eval`
- `prometheus-rules`
- `rule_eval`
- `rules`

<!-- gewyvern:entry-aliases:end -->
