# Runtime Certificate Policy Contract

Use this page when you want the narrow machine-facing contract candidate for
certificate policy, not the fuller operator explanation.

This page answers:

- which policy fields automation should bind to first
- which vocabulary is stable enough for panels and CLI
- what should remain additive in the next tightening line

Use these nearby pages with it:

- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)
- [docs/book/reference-runtime-certificate-state.md](docs/book/reference-runtime-certificate-state.md)
- [docs/machine-surface-freeze.md](docs/machine-surface-freeze.md)

## Preferred Contract

New consumers should bind to:

- policy surface endpoint:
  `/v1/runtime/certificate-policy.json`
- top-level status vocabulary
- per-reason `reason.code`
- per-reason severity
- recommended operator action mapping

Do not bind automation to English summary wording first.

## Current Stable Reads

Treat these as the current contract candidate:

| Area | Preferred read | Current status |
| --- | --- | --- |
| primary endpoint | `/v1/runtime/certificate-policy.json` | `blessed` |
| inventory companion | `/v1/runtime/certificates.json` | `blessed` |
| top-level status words | `healthy`, `observe`, `attention` | `blessed` |
| machine reason identity | `reason.code` | `blessed` |
| severity posture | `ok`, `observe`, `warning` | `blessed` |
| embedded summary in inventory | compact convenience mirror | `compat` |

## Current Reason-Code Discipline

The reason-code layer should be treated as the primary automation contract.

Current stable examples include:

- `explicit_remote_trust_without_anchors`
- `private_keys_present_in_trust_root`
- `identity_keys_without_certificates`
- `identity_certificates_without_keys`
- `empty_authority_root`
- `certificate_state_root_missing`
- `certificate_shelf_bootstrap_empty`
- `expired_certificate_material`
- `expiring_certificate_material`
- `overdue_certificate_rotation`
- `revoked_certificate_material`
- `distrusted_trust_anchor_material`

## Additive-Only Reading

For the next tightening line, new certificate checks should prefer to land as:

- additive new reason codes
- additive new evidence fields
- additive new state interpretation

They should not silently redefine the meaning of existing status words or
existing reason codes.

## Freeze Gate

Treat the certificate policy surface as frozen enough for the next minor
tightening step only when:

1. status words remain stable
2. reason codes remain the primary machine hook
3. inventory and policy roles remain clearly separated
4. new checks are documented as additive

## Earliest Tightening Reading

For the current planning posture:

- status words and reason codes should remain dependable through `0.18.x`
- summary wording may still improve without changing the machine contract
- richer PKI validation can still arrive later if it is introduced as additive
  policy reasoning
