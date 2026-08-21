# Runtime Certificate State Reference

This page records the runtime-managed certificate state shelf starting in the
`0.15.x` line.

Use it when the question is:

- where should certificate rotation state live?
- where should revocation or distrust records live?
- which runtime API exposes that state?
- which record shapes are stable enough for control-plane use?

Do not use this page as:

- the certificate inventory page
- the certificate policy page
- a full PKI issuance design

For those, use:

- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)

## State Shelf

The runtime-managed certificate state root is:

- `state/certificates/`

The active `1.16.x` line reserves these files under that root:

- `rotation-records.tsv`
- `revocation-records.tsv`

Both files are optional.

If they do not exist, the runtime reports an empty certificate-state surface.

## Runtime API

The runtime now publishes:

- `/v1/runtime/certificate-state.json`

Treat this surface as the machine-facing view of rotation and revocation
records.

It is separate from:

- `/v1/runtime/certificates.json`
- `/v1/runtime/certificate-policy.json`

Recommended operator/control-plane fetch trio:

1. `/v1/runtime/certificates.json`
2. `/v1/runtime/certificate-policy.json`
3. `/v1/runtime/certificate-state.json`

## CLI Management

The current line also exposes a light operator CLI:

- `gewyvern certificate-state show --json`
- `gewyvern certificate-state sync-rotation --json`
- `gewyvern certificate-state set-rotation --path <relative-path> --status <active|due|overdue|error>`
- `gewyvern certificate-state clear-rotation --path <relative-path>`
- `gewyvern certificate-state set-revocation --path <relative-path> --scope <trust|authority|identity|other> --status <revoked|distrusted|cleared>`
- `gewyvern certificate-state clear-revocation --path <relative-path>`

Use it as the simplest way to stage or correct certificate rotation and
revocation posture without editing the TSV shelf files by hand.

`sync-rotation` is the first automatic workflow hook in this shelf. It inspects
the current certificate inventory, derives `active` / `due` / `overdue`
rotation state from parsed validity windows, and refreshes the managed
rotation-record set.

## Rotation Records

`rotation-records.tsv` uses a tab-separated line format:

`relative_path<TAB>status<TAB>due_unix_ms<TAB>last_rotated_unix_ms<TAB>updated_unix_ms<TAB>note`

Supported `status` values in the current line:

- `active`
- `due`
- `overdue`
- `error`

Interpretation:

- `active`
  - rotation posture is current
- `due`
  - rotation should happen soon
- `overdue`
  - rotation is late and should surface operator attention
- `error`
  - rotation workflow failed or is otherwise stuck

## Revocation Records

`revocation-records.tsv` uses a tab-separated line format:

`relative_path<TAB>scope<TAB>status<TAB>effective_unix_ms<TAB>updated_unix_ms<TAB>note`

Supported `scope` values in the current line:

- `trust`
- `authority`
- `identity`
- `other`

Supported `status` values:

- `revoked`
- `distrusted`
- `cleared`

Interpretation:

- `revoked`
  - the material should no longer be treated as active
- `distrusted`
  - the trust anchor or related material is intentionally no longer trusted
- `cleared`
  - the record remains for history, but the active posture has been removed

## Policy Link

The certificate policy layer now consumes certificate-state records and may
emit these additive reason codes:

- `overdue_certificate_rotation`
- `revoked_certificate_material`
- `distrusted_trust_anchor_material`

Those reasons are reported through:

- `/v1/runtime/certificate-policy.json`
- `/v1/latest/runtime-cluster-attention.json`
- `/v1/latest/runtime-cluster-attention-reasons.json`
- `/v1/latest/runtime-cluster-attention-summary.json`

## Scope Limits

The current line does not yet promise:

- automatic rotation execution
- CRL or OCSP ingestion
- certificate-path revocation validation
- signed state ledgers
- distributed certificate-state replication

It establishes the shelf shape and the operator-facing state vocabulary so
later minor lines can add stronger workflows without replacing the contract.
