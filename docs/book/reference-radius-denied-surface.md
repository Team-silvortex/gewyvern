# Reference: RADIUS Denied Surface

Use this page when you need the current exact lookup surface for explicit
RADIUS access refusal.

## Covered Entries

### `denied`

- Protocol:
  `radius`
- Aliases:
  `access-denied`, `login-denied`, `radius-denied`, `radius_denied`, `reject`
- Default entry:
  no

## Operational Shape

The current `denied` flow models:

1. bind the process and resolve the upstream route
2. send an `Access-Request`
3. receive an `Access-Reject`

This is the narrowest RADIUS page to use when the server explicitly refuses
the identity exchange instead of continuing or accepting it.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `denied` currently
publishes:

- category:
  `failure-path`
- operator focus:
  `identity access rejection during RADIUS Access-Reject evaluation`
- typical signal:
  `Access-Reject`
- primary failure mode:
  `server_denied`
- primary failure detail:
  `access_denied`
- primary failure basis:
  `direct_protocol_signal`

## Operator Reading Order

Read this page after the RADIUS family hub when:

- you need the explicit refusal branch
- you are separating policy rejection from challenge continuation
- you want a stable denial path before IR lowering

For the broader family map, see
[docs/book/reference-radius-surface.md](docs/book/reference-radius-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `access-denied`
- `login-denied`
- `radius-denied`
- `radius_denied`
- `reject`

<!-- gewyvern:entry-aliases:end -->
