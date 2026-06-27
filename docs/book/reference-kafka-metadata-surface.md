# Reference: Kafka Metadata Surface

Use this page when the debugger sees Kafka broker metadata exchange on TCP port
`9092`.

For the broader family map, see
[docs/book/reference-kafka-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-kafka-surface.md).

## Canonical Entry

### `metadata`

Aliases:

- `kafka-metadata`
- `kafka_metadata`
- `broker-metadata`
- `topic-metadata`

Intent:

- resolve the broker route
- observe a Metadata API request
- observe the broker metadata response

## Runtime Shape

The metadata path emits these phases when evidence exists:

1. `resolve_broker`
2. `send_metadata_request`
3. `receive_metadata_response`

## Notes

The current matcher keys on the Kafka request API key byte after the length
prefix. It is a compact protocol-intent hint, not a complete Kafka parser.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `broker-metadata`
- `kafka-metadata`
- `kafka_metadata`
- `topic-metadata`

<!-- gewyvern:entry-aliases:end -->
