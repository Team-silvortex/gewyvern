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
4. receive a status-zero binary stored-response candidate

The separate `not-stored` entry recognizes status `0x0005` for SET and
APPEND/PREPEND, rather than assuming every write refusal is a SET response.
All candidates require an observed 24-byte header. `E2BIG` (`0x0003`) is not
STORED and is not relabeled NOT_STORED. These remain packet-local candidates,
not proof of complete values or persistent storage.

This is the narrowest Memcached page to use when you want explicit
write-oriented cache posture rather than the default read path.

## Operator Reading Order

Read this page after the Memcached family hub when:

- you need the `write` alias behavior
- you need to distinguish a write miss from transport silence
- you want to distinguish set behavior from get behavior
- you care about storage posture before IR lowering

## Stability Notes

The current entries capture the coarse binary `set` exchange and SET/APPEND/
PREPEND `NOT_STORED` candidates. The real `--mysql-memcached` fixture observes
APPEND to a missing key, an oversized SET rejection, a later successful SET/GET,
and recovery after an unknown opcode. Complete bounded frames independently
carry status and opaque correlation; packet candidates are not reconstructed
at fabricated message boundaries. Add/replace, text/meta, SASL, UDP, TLS and
cache performance remain outside this fixture.

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
