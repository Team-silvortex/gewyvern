# Reference: HTTP/3 Request Surface

Use this page when you need the current exact lookup surface for client-side
HTTP/3 request behavior.

## Covered Entries

### `request`

- Protocol:
  `http3`
- Aliases:
  none
- Family aliases:
  `h3-request`, `h3_request`
- Default entry:
  yes

## Operational Shape

The current `request` flow models:

1. bind the process and resolve the upstream route
2. send a QUIC Initial packet
3. send Initial-stage CRYPTO
4. receive a QUIC Handshake packet
5. receive Handshake-stage CRYPTO
6. send a request stream
7. receive a response stream
8. receive connection close

This is the narrowest HTTP/3 page to use when you want the client request path
without switching down to the more transport-centric QUIC family shelf.

## Operator Reading Order

Read this page after the HTTP/3 family hub when:

- you are checking whether `http3` resolves to its default request entry
- you want the application-facing view over the underlying QUIC progression
- you care about client request posture rather than local server posture

## Stability Notes

The current entry is client-oriented and intentionally coarse. It models the
request/response progression without unpacking higher-level header semantics.
