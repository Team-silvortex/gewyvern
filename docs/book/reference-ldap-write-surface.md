# Reference: LDAP Write Surface

Use this page when you need the current exact lookup surface for LDAP modify,
write-session, and sync/replication flows.

## Canonical Entries

### `modify`

Aliases:

- `ldap-modify`
- `ldap_modify`

Intent:

- establish an LDAP session
- send modify traffic
- receive successful modify response

### `denied`

Aliases:

- `ldap-denied`
- `ldap_denied`

Intent:

- send modify traffic
- receive modify denial

Machine-readable semantics:

- `category = failure-path`
- `operator_focus = directory write refusal during LDAP modify result evaluation`
- `typical_signal = modifyResponse`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

### `constraint`

Aliases:

- `ldap-constraint`
- `ldap_constraint`

Intent:

- send modify traffic
- receive explicit constraint violation

Machine-readable semantics:

- `category = failure-path`
- `operator_focus = directory constraint violation during LDAP modify result evaluation`
- `typical_signal = modifyResponse`
- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_constraint_violation`
- `primary_failure_basis = direct_protocol_signal`

### `write`

Aliases:

- `ldap-write`
- `ldap_write`

Intent:

- model a directory write session with bind plus modify success

### `sync`

Aliases:

- `replication`
- `ldap-sync`
- `ldap_sync`

Intent:

- model a broader directory synchronization session
- combine bind, search, and modify phases

## Shared Response Shape

The write-oriented entries currently share a staged model built from:

1. process binding
2. socket connect and establish
3. route resolution
4. optional bind exchange
5. modify request
6. success, denial, or constraint outcome

`sync` extends that write-oriented shape with search before modify so the whole
directory synchronization path is visible in one model.

## Machine-Readable Surface Semantics

The `protocol_surface("ldap", "denied")` and
`protocol_surface("ldap", "constraint")` contracts now publish
`entry_semantics` so operators and higher-level tooling can distinguish policy
rejection from constraint violation without scraping lower-level diagnosis
labels.

## Operator Reading Order

If you are reviewing LDAP write coverage, read it in this order:

1. `bind`
2. `modify`
3. `denied`
4. `constraint`
5. `write`
6. `sync`

That sequence moves from the narrow modify outcome paths toward the broader
session-oriented write and replication models.

## Validation Surface

This surface is intended to validate and lower through the standard built-in
registry path:

- `ldap` family resolution
- canonical entry/alias normalization
- package load from the selected entry directory
- lowering into program and reason rules

For the broader family map, see
[docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `ldap-constraint`
- `ldap-denied`
- `ldap-modify`
- `ldap-sync`
- `ldap-write`
- `ldap_constraint`
- `ldap_denied`
- `ldap_modify`
- `ldap_sync`
- `ldap_write`
- `replication`

<!-- gewyvern:entry-aliases:end -->
