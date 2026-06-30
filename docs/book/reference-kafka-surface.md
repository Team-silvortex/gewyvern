# Reference: Kafka Protocol Surface

Use this page when Kafka broker traffic should be treated as a first-class
streaming surface instead of a generic TCP exchange.

Default entry: `metadata`

Protocol aliases: `broker-metadata`, `broker-read`, `broker-write`, `consume`,
`kafka-fetch`, `kafka-metadata`, `kafka-produce`, `kafka_fetch`,
`kafka_metadata`, `kafka_produce`, `produce`, `topic-metadata`, `topic-read`,
`topic-write`

## What This Shelf Covers

The current Kafka family models three lightweight broker paths:

- metadata discovery against TCP port `9092`
- produce requests that write records to broker topic partitions
- fetch requests that read records from broker topic partitions

The implementation intentionally starts with stable packet-shape hints rather
than a full Kafka decoder. That keeps the eBPF-facing path cheap while still
giving the debugger useful protocol intent.

## Kafka Surface Map

### Metadata

- [docs/book/reference-kafka-metadata-surface.md](docs/book/reference-kafka-metadata-surface.md)
  Broker and topic metadata lookup.

Typical entries:

- `metadata`

### Stream

- [docs/book/reference-kafka-stream-surface.md](docs/book/reference-kafka-stream-surface.md)
  Produce and fetch request/response paths.

Typical entries:

- `produce`
- `fetch`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md)
3. one narrower Kafka subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
