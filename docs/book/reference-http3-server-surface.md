# Reference: HTTP/3 Server Surface

Use this page when you need the current exact lookup surface for local
server-side HTTP/3 response behavior.

## Covered Entries

### `server`

- Protocol:
  `http3`
- Aliases:
  none
- Family aliases:
  `http3-server-response`, `h3-server`, `h3_server`
- Default entry:
  no

## Operational Shape

The current `server` flow models:

1. bind the local server process
2. receive a remote QUIC Initial packet
3. receive Initial-stage CRYPTO
4. send a QUIC Handshake packet
5. send Handshake-stage CRYPTO
6. receive a request stream
7. send a response stream
8. send connection close

This is the narrowest HTTP/3 page to use when you want to distinguish local
server response posture from outbound client request behavior.

## Operator Reading Order

Read this page after the HTTP/3 family hub when:

- you need the local-server side of the current HTTP/3 family
- you want to distinguish remote request handling from client-originated flows
- you care about response-stream posture before IR lowering

## Stability Notes

The current entry is role-based and intentionally narrow. It captures coarse
server response progression without modeling every HTTP/3 control-stream detail.
