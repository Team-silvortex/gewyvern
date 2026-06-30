# Reference: SIP Denied Surface

Use this page when you need the current exact lookup surface for SIP rejection
or failed-response behavior.

## Covered Entries

### `denied`

- Protocol:
  `sip`
- Aliases:
  `rejected`, `failed`, `4xx`, `5xx`, `6xx`
- Default entry:
  no

## Operational Shape

The current `denied` flow models:

1. bind the process and resolve the peer route
2. receive a SIP response whose status class byte is `4`, `5`, or `6`

This is the narrowest SIP page to use when you want a session-control failure
posture without folding together successful responses or provisional progress.

## Operator Reading Order

Read this page after the SIP family hub when:

- you need the `rejected` or `failed` alias behavior
- you want to distinguish failed SIP responses from generic response arrival
- you care about 4xx/5xx/6xx status-class posture before deeper SIP lowering

## Stability Notes

The current entry classifies the status-code family by payload byte. It does
not yet parse reason phrases, headers, dialog state, or retry policy.

For the broader family map, see
[docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `4xx`
- `5xx`
- `6xx`
- `failed`
- `rejected`
- `sip-denied`
- `sip_denied`

<!-- gewyvern:entry-aliases:end -->
