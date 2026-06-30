# Reference: RDP Denied Surface

Use this page when TCP/3389 is reachable but the RDP setup path is rejected or
terminated before a useful desktop channel appears.

For the broader family map, see
[docs/book/reference-rdp-surface.md](docs/book/reference-rdp-surface.md).

## Canonical Entry

### `denied`

Aliases:

- `rdp-denied`
- `desktop-denied`
- `x224-disconnect`
- `negotiation-failed`
- `negotiation_failure`
- `rdp-failed`

Intent:

- resolve the desktop host
- observe an X.224 disconnect request
- observe an RDP negotiation failure response
- keep setup rejection separate from normal channel data

This is a debugger surface, not a full RDP security decoder. It gives operators
an early breakpoint for “the service answered, but the desktop session did not
start.”

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `desktop-denied`
- `negotiation-failed`
- `negotiation_failure`
- `rdp-denied`
- `rdp-failed`
- `x224-disconnect`

<!-- gewyvern:entry-aliases:end -->
