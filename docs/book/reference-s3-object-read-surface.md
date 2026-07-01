# Reference: S3 Object Read Surface

The S3 object-read surface tracks bucket listing, object metadata probes, and
object download requests on S3-compatible HTTP endpoints.

Family hub: [S3 surface](docs/book/reference-s3-surface.md)

Canonical entries: `list-buckets`, `head-object`, `get-object`

## Debugging Focus

- Service-root bucket inventory requests.
- Object metadata probes using HEAD.
- Object download requests using GET.
- Response status and route/process lineage around object-storage endpoints.

## Typical Question

Use this surface when an object appears missing, unauthorized, stale, or
reachable only from some clients.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `aws-s3`
- `bucket-list`
- `bucket_list`
- `download`
- `get`
- `get-object`
- `head`
- `head-object`
- `list`
- `list-buckets`
- `list_buckets`
- `metadata`
- `minio`
- `object-get`
- `object-head`
- `object-storage`
- `object_get`
- `object_head`
- `s3-get`
- `s3-head`
- `s3-list`
- `s3_get`
- `s3_head`
- `s3_list`

<!-- gewyvern:entry-aliases:end -->
