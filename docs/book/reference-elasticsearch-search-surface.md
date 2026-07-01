# Reference: Elasticsearch Search Surface

The Elasticsearch search surface tracks search requests over the
Elasticsearch-compatible HTTP API.

Family hub: [Elasticsearch surface](docs/book/reference-elasticsearch-surface.md)

Canonical entries: `search`

## Debugging Focus

- Search requests using GET or POST.
- Response direction and HTTP status framing.
- Route, process, and TCP lineage around query traffic.

## Typical Question

Use this surface when query traffic reaches the cluster but results are missing,
slow, denied, or shaped differently than the caller expects.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `elastic`
- `elasticsearch-search`
- `elasticsearch_search`
- `es`
- `es-search`
- `find`
- `lookup`
- `opensearch`
- `opensearch-search`
- `opensearch_search`
- `query`

<!-- gewyvern:entry-aliases:end -->
