# Reference: RADIUS Surface

Read this page after the generic protocol surface when the runtime path looks
like RADIUS access control rather than a generic UDP exchange.

Use it for:

- `radius` family lookup
- default entry selection for `access`
- accepted entry aliases such as `login` and `auth`

Current canonical entries:

- `access` as the default entry

Default entry: `access`

The current line keeps RADIUS as a compact single-slice family:

- authenticate or authorize through an access-style exchange
- keep the family hub small until the protocol surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-radius-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-radius-surface.md)
3. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
