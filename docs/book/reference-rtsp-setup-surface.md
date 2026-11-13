# Reference: RTSP Setup Surface

Use this page when you need the current exact lookup surface for RTSP transport
setup behavior.

## Covered Entries

### `setup`

- Protocol:
  `rtsp`
- Aliases:
  `stream`
- Family aliases:
  `rtsp-setup`, `rtsp_setup`
- Default entry:
  no

## Operational Shape

The current `setup` flow extends the describe path with transport/session
preparation:

1. establish the RTSP socket
2. send `OPTIONS`
3. receive `OPTIONS` success
4. send `DESCRIBE`
5. receive `DESCRIBE` success
6. send `SETUP`
7. receive `SETUP` success with session state

Use this page when you need more than metadata lookup and want explicit session
or transport setup posture before playback begins.

## Operator Reading Order

Read this page after the RTSP family hub when:

- you need the `stream` alias behavior
- you want to distinguish pre-playback session setup from final playback start
- you are validating whether the stream progressed beyond `DESCRIBE`

## Stability Notes

The receive stage requires a packet-local RTSP/1.0 `200`, decimal `CSeq`,
nonempty `Session` and RTP `Transport`. A session on a `461` refusal or a generic
session-only keepalive is not success. Header names are case-insensitive and
order-independent; the complete header must fit within 512 bytes of one packet.
Sparse capture may report unsupported offsets. This does not correlate requests,
validate negotiated transport parameters, or prove later media delivery.
See the [real Linux proof](../fixtures/gewyvern_rtsp_linux_x86_64_20261113.json).

For the broader family map, see
[docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `rtsp-setup`
- `rtsp_setup`
- `stream`

<!-- gewyvern:entry-aliases:end -->
