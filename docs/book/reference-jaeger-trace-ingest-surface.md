# Reference: Jaeger Trace Ingest Surface

The Jaeger trace-ingest surface tracks span emission into agents and collector
ingest endpoints.

Family hub: [Jaeger surface](docs/book/reference-jaeger-surface.md)

Canonical entries: `collector`, `agent-thrift`

## Debugging Focus

- Instrumented process lineage.
- Agent versus collector route selection.
- Collector response status.
- UDP agent packet posture when spans disappear before collector ingest.

## Typical Question

Use this surface when spans are emitted by an application but do not arrive in
Jaeger storage or query results.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `agent`
- `collector-grpc`
- `collector-http`
- `compact-thrift`
- `compact_thrift`
- `ingest`
- `jaeger`
- `jaeger-agent`
- `jaeger-agent-thrift`
- `jaeger-collector`
- `jaeger_agent`
- `jaeger_agent_thrift`
- `jaeger_collector`
- `span-ingest`
- `span_ingest`
- `trace-collector`
- `udp-agent`
- `udp_agent`

<!-- gewyvern:entry-aliases:end -->
