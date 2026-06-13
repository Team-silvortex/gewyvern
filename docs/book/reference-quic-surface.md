# Reference: QUIC Protocol Surface

Use this page when you want the QUIC portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current QUIC coverage into three narrower operator-facing
surfaces:

- client initial datagram posture
- crypto-handshake progression
- stream activity progression

## What This Shelf Covers

The current built-in QUIC family models transport-stage milestones rather than
application-level command verbs:

- send an Initial packet
- receive a Handshake packet
- exchange CRYPTO frames
- observe stream frames
- eventually observe connection close

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse transport-stage shape
- operator reading order
- validation and lowering posture

## QUIC Surface Map

### Initial

- [docs/book/reference-quic-initial-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-initial-surface.md)
  Initial packet send and handshake-packet receipt posture.

Typical entries:

- `initial`

### Crypto

- [docs/book/reference-quic-crypto-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-crypto-surface.md)
  Initial plus CRYPTO frame exchange posture.

Typical entries:

- `crypto`

### Streams

- [docs/book/reference-quic-stream-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-stream-surface.md)
  Stream send plus close-observation posture.
- [docs/book/reference-quic-bidi-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-bidi-surface.md)
  Bidirectional request/response stream posture.

Typical entries:

- `stream`
- `bidi`

## Reading Order

If you are validating current QUIC support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
3. one narrower QUIC subpage for the transport stage you care about
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current QUIC family in the `0.14.x` line.
New QUIC transport-stage branches should prefer landing behind this shelf
instead of being linked from multiple higher-level pages independently.
