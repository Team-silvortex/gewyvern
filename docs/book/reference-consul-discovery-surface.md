# Reference: Consul Discovery Surface

Navigation: [Consul surface](docs/book/reference-consul-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `health`, `catalog`, and `service` entries for Consul.

Use it when service discovery disagrees with reality: missing instances,
unexpected unhealthy results, catalog drift, or clients resolving stale
addresses.

## Entries

- `health`
- `catalog`
- `service`

## Signals

- Health API request and response direction.
- Catalog query request and response direction.
- Service discovery query request and response direction.

## Operator Notes

Discovery failures often sit between agent state, catalog state, and health
state. Keep these entries together before blaming a downstream application.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `check`
- `checks`
- `consul-agent`
- `consul-catalog`
- `consul-health`
- `consul-service`
- `consul_catalog`
- `consul_health`
- `consul_service`
- `datacenters`
- `discover`
- `health-service`
- `health_service`
- `nodes`
- `resolve`
- `service-discovery`
- `service-health`
- `service_health`
- `services`

<!-- gewyvern:entry-aliases:end -->
