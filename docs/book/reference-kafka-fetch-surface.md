# Reference: Kafka Fetch Surface

Use this page for Kafka read paths that fetch records from broker topic
partitions.

For the broader family map, see
[docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md).

## Canonical Entry

### `fetch`

Aliases:

- `kafka-fetch`
- `kafka_fetch`
- `consume`
- `broker-read`
- `topic-read`

Intent:

- resolve the broker route
- observe a Fetch API request
- observe the broker response

## Runtime Shape

The fetch path emits these phases when evidence exists:

1. `resolve_broker`
2. `send_fetch_request`
3. `receive_fetch_response`

## Operator Reading Order

Start with `metadata` when topology is uncertain, then use `fetch` when the
consumer side cannot see records, lags behind, or appears to lose broker
responses.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `broker-read`
- `consume`
- `kafka-fetch`
- `kafka_fetch`
- `topic-read`

<!-- gewyvern:entry-aliases:end -->
