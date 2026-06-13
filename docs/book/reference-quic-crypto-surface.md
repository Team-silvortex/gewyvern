# Reference: QUIC Crypto Surface

Use this page when you need the current exact lookup surface for QUIC CRYPTO
handshake behavior.

## Covered Entries

### `crypto`

- Protocol:
  `quic`
- Aliases:
  none
- Default entry:
  no

## Operational Shape

The current `crypto` flow extends the initial path with explicit CRYPTO-frame
exchange:

1. bind the process and resolve the upstream route
2. send an Initial packet
3. send a CRYPTO frame in the Initial stage
4. receive a Handshake packet
5. receive a CRYPTO frame in the Handshake stage

This is the narrowest QUIC page to use when you want more than “Initial
elicited Handshake” and need explicit handshake payload progression.

## Operator Reading Order

Read this page after the QUIC family hub when:

- you need a stronger handshake interpretation than the default `initial` entry
- you want to distinguish handshake payload exchange from later stream traffic
- you are validating transport-stage progression before IR lowering

## Stability Notes

The current entry is still handshake-centric. It does not attempt to collapse
handshake and application stream behavior into one surface.
