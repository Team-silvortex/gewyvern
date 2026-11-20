# Reference: STUN Binding Surface

Read this page when the question is specifically about `binding`.

Canonical entry:

- `binding`
- `binding-error`

This slice observes bounded modern STUN Binding candidates:

- emit a binding request
- observe a binding response
- or observe an explicit binding error response

Accepted aliases here:

- `binding-denied`
- `binding-error`

## Machine-Readable Surface Semantics

The `protocol_surface("stun", "binding-error")` contract now publishes
`entry_semantics`, and the `binding-denied` alias resolves into the same
surface, so tooling can treat explicit binding failure as a structured protocol
error, not necessarily an access denial or a timeout.

Current error semantics:

- `category = failure-path`
- `operator_focus = bounded Binding error candidate with an ERROR-CODE field; not necessarily authentication or access denial`
- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_error`
- `primary_failure_basis = direct_protocol_signal`

The legacy `binding-denied` alias remains accepted for compatibility; the name
does not prove authorization failure. An unknown required attribute can produce
error 420 without any authentication attempt.

Binding rules require the magic cookie, complete declared envelope and padded
attribute boundaries within 128 sampled bytes. Success candidates need an
XOR-MAPPED-ADDRESS; error candidates need ERROR-CODE. Sparse collectors retain
unsupported-offset diagnostics. This is not FINGERPRINT/integrity validation,
transaction correlation, public NAT/ICE connectivity or TURN relay proof.
The separate `allocate`/`refresh` packages are not covered by this Binding test.

Return to the family hub:

- [docs/book/reference-stun-surface.md](docs/book/reference-stun-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `binding-denied`
- `binding-error`
- `stun-binding-error`
- `stun_binding_error`

<!-- gewyvern:entry-aliases:end -->
