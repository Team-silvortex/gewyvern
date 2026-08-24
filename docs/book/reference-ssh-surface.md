# Reference: SSH Protocol Surface

Use this page when you want the SSH portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current SSH coverage into three narrower operator-facing
surfaces:

- session and banner exchange
- authentication outcome flow
- authenticated channel open flow

## What This Shelf Covers

The current built-in SSH family models a staged client/server conversation:

- establish the SSH socket
- receive the server banner
- send the client banner
- send key exchange init
- authenticate successfully or fail authentication
- optionally open an authenticated channel

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

Default entry: `session`

## SSH Surface Map

### Session

- [docs/book/reference-ssh-session-surface.md](docs/book/reference-ssh-session-surface.md)
  Session establishment, banner exchange, and key exchange init flow.

Typical entries:

- `session`

### Authentication

- [docs/book/reference-ssh-auth-surface.md](docs/book/reference-ssh-auth-surface.md)
  Authentication request flow, including both success and denial branches.

Typical entries:

- `auth`
- `auth-denied`

### Channel

- [docs/book/reference-ssh-channel-surface.md](docs/book/reference-ssh-channel-surface.md)
  Authenticated channel open and confirmation flow.

Typical entries:

- `channel`

## Reading Order

If you are validating current SSH support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
3. one narrower SSH subpage for the branch you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Stability Note

This page is the lookup hub for the SSH family in the current `1.17.x` line.
New SSH conversation branches should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
