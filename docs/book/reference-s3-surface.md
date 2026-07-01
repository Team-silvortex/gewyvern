# Reference: S3 Surface

S3 support gives gewyvern an object-storage view for S3-compatible HTTP API
traffic, including bucket listing, object metadata probes, object reads, object
writes, and deletes.

Default entry: `get-object`

Protocol aliases: `aws-s3`, `minio`, `object-storage`, `s3-get`, `s3_get`, `s3-list`, `s3_list`, `s3-head`, `s3_head`, `s3-put`, `s3_put`, `s3-delete`, `s3_delete`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`list-buckets`](docs/book/reference-s3-object-read-surface.md) tracks service-level bucket listing.
- [`head-object`](docs/book/reference-s3-object-read-surface.md) tracks metadata probes.
- [`get-object`](docs/book/reference-s3-object-read-surface.md) tracks object download requests.
- [`put-object`](docs/book/reference-s3-object-write-surface.md) tracks object upload or replacement requests.
- [`delete-object`](docs/book/reference-s3-object-write-surface.md) tracks object delete requests.

## Operator Use

Start with `get-object` when a caller can connect but object retrieval is
unclear. Use `head-object` to separate existence and permission probes from
body transfer. Use `put-object` and `delete-object` when mutation behavior is
the concern.

## Limits

This surface is HTTP-method-aware, not SigV4-aware yet. It does not parse
bucket names, object keys, XML error bodies, version IDs, or range semantics.
