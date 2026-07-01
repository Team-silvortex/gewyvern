# Reference: Elasticsearch Health Surface

The Elasticsearch health surface tracks cluster-health probes against an
Elasticsearch-compatible HTTP API.

Family hub: [Elasticsearch surface](docs/book/reference-elasticsearch-surface.md)

Canonical entries: `health`

## Debugging Focus

- Client-to-server health probe request.
- Server-to-client HTTP response framing.
- Route and process lineage around the search cluster endpoint.

## Typical Question

Use this surface when a client reaches the endpoint but operators need to know
whether the cluster is reachable, degraded, or timing out before query traffic
starts.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cluster-health`
- `cluster_health`
- `elasticsearch-health`
- `elasticsearch_health`
- `es-health`
- `healthcheck`
- `opensearch-health`
- `opensearch_health`

<!-- gewyvern:entry-aliases:end -->
