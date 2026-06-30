# Reference: Kerberos Surface

Read this page after the generic protocol surface when the runtime path is
Kerberos identity exchange rather than a generic directory or access flow.

Use it for:

- `kerberos` family lookup
- default entry selection for `as`
- initial authentication denial posture such as `as-error`
- service-ticket posture such as `tgs`

Primary subpages:

- [docs/book/reference-kerberos-as-surface.md](docs/book/reference-kerberos-as-surface.md)
- [docs/book/reference-kerberos-tgs-surface.md](docs/book/reference-kerberos-tgs-surface.md)

Current canonical entries:

- `as` as the default entry
- `as-error`
- `tgs`

Default entry: `as`

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-kerberos-surface.md](docs/book/reference-kerberos-surface.md)
3. one exact Kerberos subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
