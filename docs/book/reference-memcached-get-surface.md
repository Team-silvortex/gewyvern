# Reference: Memcached Get Surface

Use this page when you need the current exact lookup surface for Memcached
read-side behavior.

## Covered Entries

### `get`

- Protocol:
  `memcached`
- Aliases:
  `read`
- Family aliases:
  `memcached-get`, `memcached_get`
- Default entry:
  yes

### `miss`

- Protocol:
  `memcached`
- Aliases:
  `not-found`, `not_found`, `cache-miss`, `cache_miss`
- Family aliases:
  `memcached-miss`, `memcached_miss`
- Default entry:
  no

## Operational Shape

The current `get` flow models:

1. bind the process and resolve the upstream route
2. observe the Memcached socket transition and established state
3. send a binary `get`
4. receive a binary value response
5. optionally observe a binary `NOT_FOUND` miss response

This is the narrowest Memcached page to use when you want the default
read-oriented lookup posture.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `memcached` resolves to its default entry
- you want the `read` alias behavior
- you need to distinguish a normal cache miss from transport silence
- you only care about lookup or fetch posture

## Stability Notes

The current entry is intentionally compact. It models the coarse binary `get`
exchange and the common `NOT_FOUND` response, not broader caching policy or
multi-key nuances.

The value candidate requires binary response status `0x0000`, four bytes of
flags extras, and an observed complete 24-byte header. A `GET` miss has status
`0x0001` and must not also become a value candidate. These are bounded packet
prefix checks, not a claim of complete stream parsing in the production rule.
The native MySQL/Memcached field validator separately reconstructs full response
frames and checks missing keys, empty values, binary values and error recovery.

For the broader family map, see
[docs/book/reference-memcached-surface.md](docs/book/reference-memcached-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `cache-miss`
- `cache_miss`
- `memcached-get`
- `memcached-miss`
- `memcached-read`
- `memcached_get`
- `memcached_miss`
- `memcached_read`
- `not-found`
- `not_found`
- `read`

<!-- gewyvern:entry-aliases:end -->
