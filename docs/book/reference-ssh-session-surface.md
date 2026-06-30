# Reference: SSH Session Surface

Use this page when you need the current exact lookup surface for SSH session
startup behavior.

## Covered Entries

### `session`

- Protocol:
  `ssh`
- Aliases:
  `connect`, `handshake`
- Default entry:
  yes

## Operational Shape

The current `session` flow models the early SSH transport conversation:

1. bind the process and resolve the upstream route
2. observe the SSH socket state transition
3. receive the server banner
4. send the client banner
5. send key exchange init

This is the narrowest page to use when you only care about proving that an SSH
session started and reached banner or key exchange posture.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `ssh` resolves to its default entry
- you want the baseline connect-handshake interpretation
- you do not yet care about authentication or channel open behavior

## Stability Notes

The current shape is intentionally transport-oriented. It captures the early
SSH session path without trying to describe later authenticated actions.

For the broader family map, see
[docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `connect`
- `handshake`
- `ssh-session`
- `ssh_session`

<!-- gewyvern:entry-aliases:end -->
