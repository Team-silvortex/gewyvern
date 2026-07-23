# Runtime Certificate Policy Reference

This page records the operator-facing certificate policy contract for
`gewyvern` starting in the `0.15.x` line.

Use it when the question is:

- how does `gewyvern` interpret the certificate shelf?
- which policy statuses are possible?
- which reason codes are stable enough for UI, CLI, or automation use?
- what operator action is implied by each reason?

Do not use this page as:

- the filesystem layout page
- the runtime config key page
- the future certificate issuance design

For those, use:

- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/book/reference-runtime-certificate-state.md](docs/book/reference-runtime-certificate-state.md)

## Policy Surface

The runtime now publishes a machine-facing policy summary at:

- `/v1/runtime/certificate-policy.json`

Treat that surface as the certificate-shelf interpretation layer that sits on
top of:

- `/v1/runtime/certificates.json`

The inventory surface reports what exists.

The policy surface reports what that posture means.

For control planes and other operator-facing surfaces, the recommended fetch
pair is now:

- `/v1/runtime/certificates.json`
- `/v1/runtime/certificate-policy.json`

The inventory surface now also carries a compact embedded policy summary for
single-request dashboards, but the dedicated policy surface remains the
authoritative source for full reason records and recommended actions.

The runtime-managed certificate-state shelf is documented separately at:

- [docs/book/reference-runtime-certificate-state.md](docs/book/reference-runtime-certificate-state.md)

## Current Status Contract

The current certificate policy emits these top-level statuses:

- `healthy`
  - severity: `ok`
  - meaning: the current shelf posture is internally consistent for the checks
    that exist today
- `observe`
  - severity: `observe`
  - meaning: the shelf is safe to inspect, but incomplete, empty, or still in a
    bootstrap posture
- `attention`
  - severity: `warning`
  - meaning: operator action is recommended before depending on remote trust,
    runtime identity, or runtime-managed certificate workflows

This is intentionally conservative.

In `0.17.x`, `gewyvern` does not claim full PKI validation.

It reports whether the current shelf shape appears consistent with the runtime's
expected operator contract and whether parsed certificate material is already
expired or approaching expiry.

## Current Reason-Code Contract

These reason codes are the stable identifiers for the current policy layer.

### `explicit_remote_trust_without_anchors`

- severity: `warning`
- meaning: explicit remote trust protection is enabled, but the trust shelf does
  not contain any trust-anchor material
- operator expectation:
  - add at least one trust anchor before relying on protected remote endpoints

### `private_keys_present_in_trust_root`

- severity: `warning`
- meaning: private-key material was found in the trust shelf, which should
  normally contain anchors and bundle material only
- operator expectation:
  - move private keys out of `trust/`
  - keep the trust shelf anchor-only

### `identity_keys_without_certificates`

- severity: `warning`
- meaning: identity private keys are present, but matching identity certificate
  material is not
- operator expectation:
  - pair runtime identity keys with matching certificate material before using
    identity-based transport

### `identity_certificates_without_keys`

- severity: `observe`
- meaning: identity certificates are present, but matching private keys are not
- operator expectation:
  - verify whether the shelf is intentionally public-only or whether the key
    material is missing

### `empty_authority_root`

- severity: `observe`
- meaning: the authority shelf exists, but no local authority material has been
  added yet
- operator expectation:
  - no action is required unless local issuance or authority-managed workflows
    are expected

### `certificate_state_root_missing`

- severity: `observe`
- meaning: the certificate state root is not present yet
- operator expectation:
  - prepare the state shelf before relying on runtime-managed certificate
    workflows, issuance records, or future rotation state

### `certificate_shelf_bootstrap_empty`

- severity: `observe`
- meaning: the shelf is empty and still in a bootstrap posture
- operator expectation:
  - no urgent action is required if the runtime is still being staged locally

### `expired_certificate_material`

- severity: `warning`
- meaning: at least one parsed certificate in the trust, authority, or identity
  shelves has already passed its `notAfter` boundary
- operator expectation:
  - replace or rotate expired material before using remote trust, local
    authority, or runtime identity workflows

### `expiring_certificate_material`

- severity: `observe`
- meaning: parsed certificate material is approaching expiry within the current
  runtime warning window
- operator expectation:
  - schedule rotation soon, even if the shelf is not yet in a hard-failure
    posture

### `overdue_certificate_rotation`

- severity: `warning`
- meaning: the runtime certificate-state shelf records overdue or failed
  rotations
- operator expectation:
  - resolve the overdue rotation workflow before continuing to depend on the
    affected certificate material

### `revoked_certificate_material`

- severity: `warning`
- meaning: the runtime certificate-state shelf records active certificate
  revocations
- operator expectation:
  - remove or replace the revoked material before using the affected workflow

### `distrusted_trust_anchor_material`

- severity: `warning`
- meaning: trust-anchor material has been explicitly distrusted
- operator expectation:
  - stop using the distrusted anchor and replace it with approved trust
    material

## Root-Policy Expectations

The active `1.7.x` policy assumes this root intent:

- `certificates/trust/`
  - should primarily contain trust anchors, bundles, and chain material
  - should not normally contain private keys
- `certificates/authorities/`
  - may remain empty until local authority workflows are introduced
  - can later hold issuing material and local authority context
- `certificates/identities/`
  - can hold runtime identity certificates and matching private keys
- `state/certificates/`
  - can hold runtime-managed state, future issuance state, and rotation metadata

These are policy expectations, not yet a complete cryptographic validation
system.

## Recommended Integration Pattern

If a UI, CLI, or external control plane consumes the policy surface, prefer this
pattern:

1. read `/v1/runtime/certificates.json`
2. read `/v1/runtime/certificate-policy.json`
3. display the raw shelf inventory separately from the interpreted policy result
4. bind workflows to `reason.code`, not to English summary text

That keeps localized UI and automation stable even if summary wording changes.

## Current Scope Limits

The current contract does not yet promise:

- certificate chain verification
- SAN or subject validation
- CA-path validation
- revocation checks
- automatic issuance
- automatic renewal

The current line does include a narrow expiry posture check based on parsed
certificate `notAfter` timestamps, but it is not yet a full certificate-path or
revocation validation system.

Those can be added later without invalidating the current reason-code layer, as
long as new checks are introduced as additive policy reasons.
