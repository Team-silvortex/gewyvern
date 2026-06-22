# Reference: RADIUS Surface

Read this page after the generic protocol surface when the runtime path looks
like RADIUS access control rather than a generic UDP exchange.

Use it for:

- `radius` family lookup
- default entry selection for `access`
- accepted-path aliases such as `login`, `auth`, and `radius-access`
- challenge continuation paths such as `radius-challenge`, `otp`, and `mfa`
- explicit denial paths such as `radius-denied`, `reject`, and `access-denied`

## Family Aliases

The current registry also accepts these family-level spellings for RADIUS entry
selection:

- `radius-challenge`
- `radius-denied`
- `radius_challenge`
- `radius_denied`

Current canonical entries:

- `access` as the default entry
- `challenge`
- `denied`

Default entry: `access`

## RADIUS Surface Map

### Access

- [docs/book/reference-radius-access-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-access-surface.md)
  Successful `Access-Request` to `Access-Accept` path.

Typical entries:

- `access`

### Challenge

- [docs/book/reference-radius-challenge-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-challenge-surface.md)
  Continuation branch for `Access-Challenge` responses.

Typical entries:

- `challenge`

### Denied

- [docs/book/reference-radius-denied-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-denied-surface.md)
  Explicit refusal branch for `Access-Reject` responses.

Typical entries:

- `denied`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-radius-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-surface.md)
3. one exact RADIUS subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
