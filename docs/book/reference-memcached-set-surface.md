# Reference: Memcached Set Surface

Use this page when you need the current exact lookup surface for Memcached
write-side behavior.

## Covered Entries

### `set`

- Protocol:
  `memcached`
- Aliases:
  `write`
- Family aliases:
  `memcached-set`, `memcached_set`
- Default entry:
  no

### `not-stored`

- Protocol:
  `memcached`
- Aliases:
  `not_stored`, `store-miss`, `store_miss`, `write-miss`, `write_miss`
- Family aliases:
  `memcached-not-stored`, `memcached_not_stored`
- Default entry:
  no

## Operational Shape

The current `set` flow models:

1. bind the process and resolve the upstream route
2. observe the Memcached socket transition and established state
3. send a binary `set`
4. receive a binary stored response
5. optionally observe a binary `NOT_STORED` response

This is the narrowest Memcached page to use when you want explicit
write-oriented cache posture rather than the default read path.

## Operator Reading Order

Read this page after the Memcached family hub when:

- you need the `write` alias behavior
- you need to distinguish a write miss from transport silence
- you want to distinguish set behavior from get behavior
- you care about storage posture before IR lowering

## Stability Notes

The current entries capture the coarse binary `set` exchange and common
`NOT_STORED` response. They do not yet split add, replace, or other mutation
commands into their own pages.

For the broader family map, see
[docs/book/reference-memcached-surface.md](docs/book/reference-memcached-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `memcached-not-stored`
- `memcached-set`
- `memcached-write`
- `memcached_not_stored`
- `memcached_set`
- `memcached_write`
- `not_stored`
- `store-miss`
- `store_miss`
- `write`
- `write-miss`
- `write_miss`

<!-- gewyvern:entry-aliases:end -->
