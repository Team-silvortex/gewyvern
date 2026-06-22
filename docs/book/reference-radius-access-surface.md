# Reference: RADIUS Access Surface

Use this page when you need the current exact lookup surface for successful
RADIUS access negotiation.

## Covered Entries

### `access`

- Protocol:
  `radius`
- Aliases:
  `auth`, `login`, `radius-access`, `radius_access`
- Default entry:
  yes

## Operational Shape

The current `access` flow models:

1. bind the process and resolve the upstream route
2. send an `Access-Request`
3. receive an `Access-Accept`

This is the narrowest RADIUS page to use when the authentication exchange
completed successfully and you want the accepted-path shape instead of a denial
or continuation challenge.

## Operator Reading Order

Read this page after the RADIUS family hub when:

- you want to validate the default `access` entry resolution
- you are distinguishing success from `denied` or `challenge`
- you care about the accepted response path before IR lowering

For the broader family map, see
[docs/book/reference-radius-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth`
- `login`
- `radius-access`
- `radius_access`

<!-- gewyvern:entry-aliases:end -->
