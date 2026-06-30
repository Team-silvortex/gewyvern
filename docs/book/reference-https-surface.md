# Reference: HTTPS Surface

Read this page after the generic protocol surface when the runtime path is an
HTTPS connect-style exchange rather than plain HTTP or arbitrary TLS traffic.

Use it for:

- `https` family lookup
- default entry selection for `connect`
- keeping TLS-protected request setup separate from raw TLS client posture
- jumping into `tls client` when the protocol surface or runtime report exposes
  a structured companion hint

Current canonical entries:

- `connect` as the default entry

Default entry: `connect`

The current line keeps HTTPS as a compact single-slice family:

- establish or observe HTTPS request setup through the connect path
- keep the family hub small until the surface grows beyond one stable entry

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
3. [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

The machine-facing API now records that same jump through
`reading_companions`; see:

- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
