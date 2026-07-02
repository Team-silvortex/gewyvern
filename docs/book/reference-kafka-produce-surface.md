# Reference: Kafka Produce Surface

Use this page for Kafka write paths that publish records into broker topic
partitions.

For the broader family map, see
[docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md).

## Canonical Entry

### `produce`

Aliases:

- `kafka-produce`
- `kafka_produce`
- `produce`
- `broker-write`
- `topic-write`

Intent:

- resolve the broker route
- observe a Produce API request
- observe the broker response

## Runtime Shape

The produce path emits these phases when evidence exists:

1. `resolve_broker`
2. `send_produce_request`
3. `receive_produce_response`

## Operator Reading Order

Start with `metadata` when topology is uncertain, then use `produce` when the
workload writes topic data but acknowledgements, routing, or broker visibility
are unclear.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `broker-write`
- `kafka-produce`
- `kafka_produce`
- `produce`
- `topic-write`

<!-- gewyvern:entry-aliases:end -->
