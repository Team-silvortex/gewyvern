# Reference: S3 Object Write Surface

The S3 object-write surface tracks object upload, replacement, and deletion
requests on S3-compatible HTTP endpoints.

Family hub: [S3 surface](docs/book/reference-s3-surface.md)

Canonical entries: `put-object`, `delete-object`

## Debugging Focus

- Object upload or replacement requests using PUT.
- Object deletion requests using DELETE.
- Response status and route/process lineage around object mutation traffic.

## Typical Question

Use this surface when writes appear to reach the endpoint but objects are not
created, are overwritten unexpectedly, or deletes behave differently from the
caller expectation.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `delete`
- `delete-object`
- `object-delete`
- `object-put`
- `object_delete`
- `object_put`
- `put`
- `put-object`
- `remove`
- `s3-delete`
- `s3-put`
- `s3_delete`
- `s3_put`
- `upload`

<!-- gewyvern:entry-aliases:end -->
