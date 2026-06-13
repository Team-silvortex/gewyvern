# Reference: SSH Channel Surface

Use this page when you need the current exact lookup surface for authenticated
SSH channel behavior.

## Covered Entries

### `channel`

- Protocol:
  `ssh`
- Aliases:
  `shell`
- Default entry:
  no

## Operational Shape

The current `channel` path builds on the successful auth flow and then extends
it with authenticated channel open behavior:

1. session connect and banner exchange
2. send key exchange init
3. send auth request
4. receive auth success
5. send channel open
6. receive channel open confirmation

This is the narrowest SSH page to use when you want to prove that the session
progressed past authentication and into an interactive or shell-like channel
phase.

## Operator Reading Order

Read this page after the SSH family hub when:

- you need the `shell` alias behavior
- you want to distinguish auth-only activity from an opened channel
- you are validating a deeper SSH conversation before IR lowering

## Stability Notes

The current channel path is intentionally narrow. It models authenticated
channel establishment, not the full variety of later channel requests or
subsystems.

For the broader family map, see
[docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md).
