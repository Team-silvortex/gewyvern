# Reference: Kafka Stream Surface

Use this page for Kafka request paths that read or write records through a
broker.

For the broader family map, see
[docs/book/reference-kafka-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-kafka-surface.md).

## Canonical Entries

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

## Operator Reading Order

Start with `metadata` when topology is uncertain, then choose `produce` or
`fetch` based on whether the workload is writing or consuming topic data.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `broker-read`
- `broker-write`
- `consume`
- `kafka-fetch`
- `kafka-produce`
- `kafka_fetch`
- `kafka_produce`
- `produce`
- `topic-read`
- `topic-write`

<!-- gewyvern:entry-aliases:end -->
