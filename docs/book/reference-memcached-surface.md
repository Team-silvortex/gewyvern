# Reference: Memcached Protocol Surface

Use this page when you want the Memcached portion of the built-in protocol
shelf as stable lookup material instead of a tutorial.

This shelf groups the current Memcached coverage into two narrower
operator-facing surfaces:

- key read flow, including explicit cache misses
- key write flow, including explicit not-stored responses

## What This Shelf Covers

The current built-in Memcached family models two coarse binary-protocol actions
over an established TCP session:

- connect and establish the Memcached socket
- send `get` and receive a value response
- observe a binary `get` response with `NOT_FOUND`
- send `set` and receive a stored response
- observe a binary `set` response with `NOT_STORED`

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for Memcached
entry selection:

- `memcached-get`
- `memcached-miss`
- `memcached-not-stored`
- `memcached-set`
- `memcached_get`
- `memcached_miss`
- `memcached_not_stored`
- `memcached_set`

Default entry: `get`

## Memcached Surface Map

### Get

- [docs/book/reference-memcached-get-surface.md](docs/book/reference-memcached-get-surface.md)
  Read-side key lookup path.

Typical entries:

- `get`
- `miss`

### Set

- [docs/book/reference-memcached-set-surface.md](docs/book/reference-memcached-set-surface.md)
  Write-side key storage path.

Typical entries:

- `set`
- `not-stored`

## Reading Order

If you are validating current Memcached support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-memcached-surface.md](docs/book/reference-memcached-surface.md)
3. the get or set subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the Memcached family in the current `1.15.x`
line. New Memcached operation branches should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
