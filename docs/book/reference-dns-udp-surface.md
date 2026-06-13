# Reference: DNS UDP Surface

Use this page when you need the current exact lookup surface for UDP-carried
DNS behavior.

## Covered Entries

### `udp`

- Protocol:
  `dns`
- Aliases:
  none
- Default entry:
  yes

## Operational Shape

The current `udp` flow models:

1. bind the process and resolve the upstream route
2. send a UDP datagram query
3. receive a UDP datagram reply

This is the narrowest DNS page to use when you want the default resolver-style
lookup interpretation.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `dns` resolves to its default entry
- you want the common datagram lookup posture
- you do not need TCP-carried query behavior

## Stability Notes

The current entry is transport-oriented and intentionally small. It models the
coarse UDP request/reply path, not detailed resolver semantics.
