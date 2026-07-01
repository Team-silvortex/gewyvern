# Reference: OpenTelemetry OTLP Protocol Surface

Use this page when telemetry traffic should be interpreted as export intent
rather than only generic gRPC or HTTP request/response traffic.

Default entry: `traces`

Protocol aliases: `otlp`, `opentelemetry`, `otel`, `otlp-traces`,
`otlp_traces`, `otel-traces`, `otlp-metrics`, `otlp_metrics`,
`otel-metrics`, `otlp-logs`, `otlp_logs`, `otel-logs`,
`otlp-partial-success`, `otlp_partial_success`, `otel-partial`,
`otlp-error`, `otlp-export-error`, `otlp_export_error`, `otel-error`

## What This Shelf Covers

The current OTLP family models five stable debugger-facing paths:

- trace export batches sent to a collector
- metrics export batches sent to a collector
- logs export batches sent to a collector
- partial-success responses where data was accepted but some items were dropped
- export-error responses where the collector or gateway rejected the request

This is intentionally not a full protobuf decoder. The 0.18.x behavior is to
identify signal type, collector route, export direction, and failure posture
without making payload decoding a dependency.

## OTLP Surface Map

### Signal Export

- [docs/book/reference-otlp-signal-export-surface.md](docs/book/reference-otlp-signal-export-surface.md)
  Trace, metric, and log export flows.

Typical entries:

- `traces`
- `metrics`
- `logs`

### Collector Response

- [docs/book/reference-otlp-collector-response-surface.md](docs/book/reference-otlp-collector-response-surface.md)
  Partial-success and rejected export responses.

Typical entries:

- `partial-success`
- `export-error`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-otlp-surface.md](docs/book/reference-otlp-surface.md)
3. one narrower OTLP subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
