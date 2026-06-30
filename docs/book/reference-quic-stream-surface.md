# Reference: QUIC Stream Surface

Use this page when you need the current exact lookup surface for QUIC
stream-send behavior.

## Covered Entries

### `stream`

- Protocol:
  `quic`
- Aliases:
  none
- Default entry:
  no

## Operational Shape

The current `stream` flow extends the crypto-handshake posture with later
stream activity:

1. send an Initial packet
2. send an Initial-stage CRYPTO frame
3. receive a Handshake packet
4. receive a Handshake-stage CRYPTO frame
5. send a stream frame
6. receive connection close

Use this page when you want a post-handshake outbound stream posture without
requiring a full bidirectional request/response interpretation.

## Operator Reading Order

Read this page after the QUIC family hub when:

- you want to prove that stream traffic began
- you care about the outbound stream stage more than the reply stage
- you want a narrower page than the bidirectional stream path

## Stability Notes

The current entry records stream-send progression plus eventual close
observation. It does not guarantee that a response stream was observed.

For the broader family map, see
[docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md).
