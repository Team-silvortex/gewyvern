# Reference: SOCKS5 Protocol Surface

Use this page when you want the SOCKS5 portion of the built-in protocol shelf
as stable lookup material instead of a tutorial.

This shelf groups the current SOCKS5 coverage into three narrower
operator-facing surfaces:

- unauthenticated session/connect flow
- username/password-authenticated session flow
- denial branches for auth and connect

## What This Shelf Covers

The current built-in SOCKS5 family models a staged proxy conversation:

- open the SOCKS5 socket
- negotiate method selection
- optionally authenticate with username/password
- send a connect request
- either succeed or branch into denial

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- machine-readable failure semantics for denial branches
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for SOCKS5 entry
selection:

- `socks`
- `socks5-session`
- `socks5_session`

Default entry: `session`

## SOCKS5 Surface Map

### Session

- [docs/book/reference-socks5-session-surface.md](docs/book/reference-socks5-session-surface.md)
  Unauthenticated session establishment and successful proxy connect.

Typical entries:

- `session`

### Authentication

- [docs/book/reference-socks5-auth-surface.md](docs/book/reference-socks5-auth-surface.md)
  Username/password method negotiation, auth success, and authenticated connect
  success.

Typical entries:

- `auth`

### Denial Branches

- [docs/book/reference-socks5-denied-surface.md](docs/book/reference-socks5-denied-surface.md)
  Connect denial, auth denial, and authenticated-connect denial.

Typical entries:

- `denied`
- `auth-denied`
- `auth-connect-denied`

## Reading Order

If you are validating current SOCKS5 support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)
3. one narrower SOCKS5 subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Stability Note

This page is the lookup hub for the SOCKS5 family in the current `1.4.6`
line. New SOCKS5 command families should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
