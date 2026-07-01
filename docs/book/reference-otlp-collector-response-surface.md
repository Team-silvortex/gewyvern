# Reference: OTLP Collector Response Surface

The OTLP collector-response surface tracks accepted-with-drops and rejected
export responses from a collector, gateway, or telemetry proxy.

Family hub: [OTLP surface](docs/book/reference-otlp-surface.md)

Canonical entries: `partial-success`, `export-error`

## Debugging Focus

- Partial-success responses that indicate accepted requests with dropped items.
- gRPC status, HTTP status, or gateway rejection around telemetry export.
- Collector-side quota, retryability, and backpressure posture.

## Typical Question

Use this surface when telemetry traffic reaches a collector but the backend
still misses data, or when exporters retry without a clear application error.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `collector-error`
- `dropped-items`
- `dropped_items`
- `error`
- `export-failed`
- `export_failed`
- `otel-error`
- `otel-partial`
- `otlp-error`
- `otlp-export-error`
- `otlp-partial-success`
- `otlp_export_error`
- `otlp_partial_success`
- `partial`
- `partial_success`

<!-- gewyvern:entry-aliases:end -->
