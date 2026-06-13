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

## Operational Shape

The current `set` flow models:

1. bind the process and resolve the upstream route
2. observe the Memcached socket transition and established state
3. send a binary `set`
4. receive a binary stored response

This is the narrowest Memcached page to use when you want explicit
write-oriented cache posture rather than the default read path.

## Operator Reading Order

Read this page after the Memcached family hub when:

- you need the `write` alias behavior
- you want to distinguish set behavior from get behavior
- you care about storage posture before IR lowering

## Stability Notes

The current entry captures the coarse binary `set` exchange only. It does not
yet try to split add, replace, or other mutation commands into their own pages.
