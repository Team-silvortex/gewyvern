# Reference: Elasticsearch Surface

Elasticsearch support gives gewyvern an HTTP-API view for search datastore
traffic, including cluster health probes, search queries, document indexing,
and bulk mutation flows.

Default entry: `search`

Protocol aliases: `elastic`, `opensearch`, `es`, `elasticsearch-health`, `elasticsearch_health`, `opensearch-health`, `opensearch_health`, `es-health`, `elasticsearch-search`, `elasticsearch_search`, `opensearch-search`, `opensearch_search`, `es-search`, `elasticsearch-index`, `elasticsearch_index`, `opensearch-index`, `opensearch_index`, `es-index`, `elasticsearch-bulk`, `elasticsearch_bulk`, `opensearch-bulk`, `opensearch_bulk`, `es-bulk`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`health`](docs/book/reference-elasticsearch-health-surface.md) tracks cluster health probes.
- [`search`](docs/book/reference-elasticsearch-search-surface.md) tracks search query requests.
- [`index`](docs/book/reference-elasticsearch-mutation-surface.md) tracks single-document writes.
- [`bulk`](docs/book/reference-elasticsearch-mutation-surface.md) tracks bulk indexing and mutation requests.

## Operator Use

Start with `search` when query behavior is unclear. Use `health` when the
client can connect but the cluster may be degraded. Use `index` for single
document writes and `bulk` when HTTP success may hide per-item failures.

## Limits

This surface is HTTP-method-aware and route-aware, not JSON-body-aware yet. It
does not parse Query DSL, index names, bulk item status arrays, shard routing,
or security exception bodies.
