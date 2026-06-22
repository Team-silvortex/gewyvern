# Reference: Kerberos AS Surface

Read this page when the path is clearly in the initial authentication exchange.

Canonical entries covered here:

- `as`
- `as-error`

Current accepted aliases:

- `login`
- `initial-auth`
- `login-denied`
- `initial-auth-error`

Operational split:

- `as` models AS-REQ to AS-REP success posture
- `as-error` models AS-REQ to KRB-ERROR denial posture

## Machine-Readable Surface Semantics

The `protocol_surface("kerberos", "as-error")` contract now publishes
`entry_semantics` so higher-level tooling can classify explicit authentication
exchange failure without scraping this page.

Current failure semantics:

- `category = failure-path`
- `operator_focus = initial Kerberos authentication exchange failed with explicit KRB-ERROR`
- `typical_signal = KRB-ERROR`
- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_error`
- `primary_failure_basis = direct_protocol_signal`

Return to the family hub:

- [docs/book/reference-kerberos-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-kerberos-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `initial-auth`
- `initial-auth-error`
- `login`
- `login-denied`

<!-- gewyvern:entry-aliases:end -->
