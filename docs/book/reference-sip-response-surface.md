# Reference: SIP Response Surface

Use this page when you need the current exact lookup surface for SIP response
observation without committing to a specific request method.

## Covered Entries

### `response`

- Protocol:
  `sip`
- Aliases:
  `reply`, `provisional`, `final`
- Default entry:
  no

## Operational Shape

The current `response` flow models:

1. bind the process and resolve the peer route
2. receive a UDP datagram whose payload starts with `SIP/`

This is the narrowest SIP page to use when you are debugging whether responses
are arriving at all before classifying success, provisional, or failure state.

## Operator Reading Order

Read this page after the SIP family hub when:

- you need the `reply`, `provisional`, or `final` alias behavior
- you want response visibility without tying the path to `REGISTER`, `INVITE`,
  or `BYE`
- you care about response arrival before richer SIP status-code lowering

## Stability Notes

The current entry identifies the response envelope. It intentionally keeps
status-code class policy on the `denied` entry until a fuller SIP parser lands.

For the broader family map, see
[docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `final`
- `provisional`
- `reply`
- `sip-response`
- `sip_response`

<!-- gewyvern:entry-aliases:end -->
