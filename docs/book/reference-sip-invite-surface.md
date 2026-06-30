# Reference: SIP Invite Surface

Use this page when you need the current exact lookup surface for SIP call setup
behavior.

## Covered Entries

### `invite`

- Protocol:
  `sip`
- Aliases:
  `call`, `session`
- Default entry:
  no

## Operational Shape

The current `invite` flow models:

1. bind the process and resolve the upstream route
2. send `INVITE` over UDP
3. receive a SIP response

This is the narrowest SIP page to use when you want a call-setup or
session-start posture without folding in registration or teardown.

## Operator Reading Order

Read this page after the SIP family hub when:

- you need the `call` or `session` alias behavior
- you want to distinguish invite traffic from registration
- you care about call-setup posture before IR lowering

## Stability Notes

The current entry is intentionally broad at the response level. It models the
invite exchange shape, not full provisional-versus-final response detail.

For the broader family map, see
[docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `call`
- `session`
- `sip-invite`
- `sip_invite`

<!-- gewyvern:entry-aliases:end -->
