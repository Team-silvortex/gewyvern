# Reference: Kafka Metadata Surface

Use this page when the debugger sees Kafka broker metadata or capability
exchange on TCP port `9092`.

For the broader family map, see
[docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md).

## Canonical Entries

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

### `api-versions`

Aliases:

- `kafka-api-versions`
- `kafka_api_versions`
- `broker-api-versions`
- `broker_api_versions`
- `api-versions`
- `api_versions`

Intent:

- resolve the broker route
- observe an ApiVersions API request
- observe the broker compatibility response

## Runtime Shape

The metadata path emits these phases when evidence exists:

1. `resolve_broker`
2. `send_metadata_request`
3. `receive_metadata_response`

The ApiVersions path emits:

1. `resolve_broker`
2. `send_api_versions_request`
3. `receive_api_versions_response`

## Notes

The current matcher keys on the Kafka request API key byte after the length
prefix. It is a compact protocol-intent hint, not a complete Kafka parser.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `api-versions`
- `api_versions`
- `broker-api-versions`
- `broker-capabilities`
- `broker-metadata`
- `broker_api_versions`
- `capabilities`
- `kafka-api-versions`
- `kafka-metadata`
- `kafka_api_versions`
- `kafka_metadata`
- `topic-metadata`
- `version-negotiation`
- `version_negotiation`

<!-- gewyvern:entry-aliases:end -->
