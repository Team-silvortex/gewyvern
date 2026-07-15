# Reference: QUIC Protocol Surface

Use this page when you want the QUIC portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current QUIC coverage into six narrower operator-facing
surfaces:

- client initial datagram posture
- retry/address-validation continuation
- crypto-handshake progression
- explicit connection-close termination
- explicit locally emitted connection-close termination
- stream activity progression

## What This Shelf Covers

The current built-in QUIC family models transport-stage milestones rather than
application-level command verbs:

- send an Initial packet
- receive a Retry packet
- receive a Handshake packet
- exchange CRYPTO frames
- observe a `CONNECTION_CLOSE` frame
- emit a `CONNECTION_CLOSE` frame
- observe stream frames
- eventually observe connection close

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse transport-stage shape
- operator reading order
- validation and lowering posture

Default entry: `initial`

## QUIC Surface Map

### Initial

- [docs/book/reference-quic-initial-surface.md](docs/book/reference-quic-initial-surface.md)
  Initial packet send and handshake-packet receipt posture.

Typical entries:

- `initial`

### Retry

- [docs/book/reference-quic-retry-surface.md](docs/book/reference-quic-retry-surface.md)
  Address-validation continuation after the first client Initial.

Typical entries:

- `retry`

### Crypto

- [docs/book/reference-quic-crypto-surface.md](docs/book/reference-quic-crypto-surface.md)
  Initial plus CRYPTO frame exchange posture.

Typical entries:

- `crypto`

### Close

- [docs/book/reference-quic-close-surface.md](docs/book/reference-quic-close-surface.md)
  Explicit transport termination after early QUIC progression.
- [docs/book/reference-quic-local-close-surface.md](docs/book/reference-quic-local-close-surface.md)
  Explicit locally initiated transport termination after handshake progress.

Typical entries:

- `close`
- `local-close`

### Streams

- [docs/book/reference-quic-stream-surface.md](docs/book/reference-quic-stream-surface.md)
  Stream send plus close-observation posture.
- [docs/book/reference-quic-bidi-surface.md](docs/book/reference-quic-bidi-surface.md)
  Bidirectional request/response stream posture.

Typical entries:

- `stream`
- `bidi`

## Reading Order

If you are validating current QUIC support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)
3. one narrower QUIC subpage for the transport stage you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Stability Note

This page is the lookup hub for the QUIC family in the current `1.2.0` line.
New QUIC transport-stage branches should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
