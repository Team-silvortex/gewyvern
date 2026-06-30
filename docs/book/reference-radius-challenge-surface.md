# Reference: RADIUS Challenge Surface

Use this page when you need the current exact lookup surface for RADIUS
challenge/continuation negotiation.

## Covered Entries

### `challenge`

- Protocol:
  `radius`
- Aliases:
  `mfa`, `otp`, `radius-challenge`, `radius_challenge`
- Default entry:
  no

## Operational Shape

The current `challenge` flow models:

1. bind the process and resolve the upstream route
2. send an `Access-Request`
3. receive an `Access-Challenge`

This is the narrowest RADIUS page to use when the server is explicitly asking
for another round of credentials or factors instead of accepting or rejecting
the request.

## Machine-Readable Surface Semantics

When selected through the JSON protocol-surface API, `challenge` currently
publishes:

- category:
  `continuation-path`
- operator focus:
  `identity challenge continuation during RADIUS Access-Challenge evaluation`
- typical signal:
  `Access-Challenge`

## Operator Reading Order

Read this page after the RADIUS family hub when:

- the authentication path did not terminate yet
- you want to separate continuation prompts from explicit denials
- you need a stable challenge-stage surface before IR lowering

For the broader family map, see
[docs/book/reference-radius-surface.md](docs/book/reference-radius-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `mfa`
- `otp`
- `radius-challenge`
- `radius_challenge`

<!-- gewyvern:entry-aliases:end -->
