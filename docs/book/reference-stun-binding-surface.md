# Reference: STUN Binding Surface

Read this page when the question is specifically about `binding`.

Canonical entry:

- `binding`
- `binding-error`

This slice covers the smallest STUN reachability round-trip:

- emit a binding request
- observe a binding response
- or observe an explicit binding error response

Accepted aliases here:

- `binding-denied`
- `binding-error`

## Machine-Readable Surface Semantics

The `protocol_surface("stun", "binding-error")` contract now publishes
`entry_semantics`, and the `binding-denied` alias resolves into the same
surface, so tooling can treat explicit binding failure as a structured denial
path instead of a timeout-only interpretation.

Current denial semantics:

- `category = failure-path`
- `operator_focus = explicit binding failure response instead of successful reachability confirmation`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

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
