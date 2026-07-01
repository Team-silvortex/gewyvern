# Reference: OTLP Signal Export Surface

The OTLP signal-export surface tracks telemetry batches leaving an instrumented
process and heading toward an OpenTelemetry collector or compatible gateway.

Family hub: [OTLP surface](docs/book/reference-otlp-surface.md)

Canonical entries: `traces`, `metrics`, `logs`

## Debugging Focus

- Trace export batches and span delivery posture.
- Metrics export batches, aggregation direction, and route/process lineage.
- Logs export batches where volume and resource attribution often explain drops.
- Collector endpoint selection across OTLP/gRPC and OTLP/HTTP deployments.

## Typical Question

Use this surface when an app appears instrumented but traces, metrics, or logs
do not arrive at the expected collector or backend.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `events`
- `log`
- `log-export`
- `log_export`
- `metric`
- `metric-export`
- `metric_export`
- `opentelemetry`
- `otel`
- `otel-logs`
- `otel-metrics`
- `otel-traces`
- `otlp`
- `otlp-logs`
- `otlp-metrics`
- `otlp-traces`
- `otlp_logs`
- `otlp_metrics`
- `otlp_traces`
- `span`
- `spans`
- `timeseries`
- `trace`
- `trace-export`
- `trace_export`

<!-- gewyvern:entry-aliases:end -->
