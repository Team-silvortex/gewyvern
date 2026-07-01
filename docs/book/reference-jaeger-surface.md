# Reference: Jaeger Protocol Surface

Use this page when trace traffic should be interpreted as Jaeger ingest, agent,
query, sampling, or dependency-map intent rather than only generic HTTP, gRPC,
or UDP traffic.

Default entry: `collector`

Protocol aliases: `jaeger`, `jaeger-collector`, `jaeger_collector`,
`trace-collector`, `jaeger-agent`, `jaeger-agent-thrift`, `jaeger_agent`,
`jaeger_agent_thrift`, `jaeger-query`, `jaeger_query`, `trace-query`,
`jaeger-sampling`, `jaeger_sampling`, `sampling-strategy`,
`jaeger-dependencies`, `jaeger_dependencies`, `service-dependencies`

## What This Shelf Covers

The current Jaeger family models five debugger-facing paths:

- collector span ingest
- agent compact-thrift UDP span emission
- query API reads
- sampling strategy reads
- dependency graph reads

This is not a full thrift, protobuf, or trace-storage decoder. The 0.18.x
behavior is to identify trace ingest, read, sampling, and service graph posture
without making payload decoding a dependency.

## Jaeger Surface Map

### Trace Ingest

- [docs/book/reference-jaeger-trace-ingest-surface.md](docs/book/reference-jaeger-trace-ingest-surface.md)
  Collector and agent span ingress.

Typical entries:

- `collector`
- `agent-thrift`

### Trace Query And Dependencies

- [docs/book/reference-jaeger-trace-read-surface.md](docs/book/reference-jaeger-trace-read-surface.md)
  Trace query and service dependency reads.

Typical entries:

- `query`
- `dependencies`

### Sampling Control

- [docs/book/reference-jaeger-sampling-surface.md](docs/book/reference-jaeger-sampling-surface.md)
  Sampling strategy requests.

Typical entries:

- `sampling`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-jaeger-surface.md](docs/book/reference-jaeger-surface.md)
3. one narrower Jaeger subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
