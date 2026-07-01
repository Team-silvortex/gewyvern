# Reference: Elasticsearch Mutation Surface

The Elasticsearch mutation surface tracks single-document indexing and bulk
mutation requests over the Elasticsearch-compatible HTTP API.

Family hub: [Elasticsearch surface](docs/book/reference-elasticsearch-surface.md)

Canonical entries: `index`, `bulk`

## Debugging Focus

- Single-document index or update requests.
- Bulk mutation requests where individual item failures may be embedded in a
  successful HTTP response.
- Route, process, and TCP lineage around write-heavy search datastore traffic.

## Typical Question

Use this surface when writes appear to succeed at the transport layer but
documents are missing, partially indexed, rejected, or routed to the wrong
cluster target.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `batch`
- `bulk-index`
- `bulk_index`
- `document`
- `elasticsearch-bulk`
- `elasticsearch-index`
- `elasticsearch_bulk`
- `elasticsearch_index`
- `es-bulk`
- `es-index`
- `index-document`
- `index_document`
- `opensearch-bulk`
- `opensearch-index`
- `opensearch_bulk`
- `opensearch_index`
- `write`

<!-- gewyvern:entry-aliases:end -->
