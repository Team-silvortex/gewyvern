# Reference: RTSP Describe Surface

Use this page when you need the current exact lookup surface for RTSP metadata
lookup behavior.

## Covered Entries

### `describe`

- Protocol:
  `rtsp`
- Aliases:
  `metadata`
- Family aliases:
  `rtsp-describe`, `rtsp_describe`
- Default entry:
  no

## Operational Shape

The current `describe` flow extends the base RTSP probe with metadata lookup:

1. establish the RTSP socket
2. send `OPTIONS`
3. receive `OPTIONS` success
4. send `DESCRIBE`
5. receive `DESCRIBE` success with content

This is the narrowest RTSP page to use when you want a stream-description or
metadata posture without yet asserting full session setup.

## Operator Reading Order

Read this page after the RTSP family hub when:

- you need the `metadata` alias behavior
- you want to distinguish capability probe from actual stream description
- you care about a deeper control-plane conversation before `SETUP`

## Stability Notes

The receive stage is a packet-local RTSP/1.0 `200` candidate with decimal `CSeq`,
SDP content type and positive content length. It does not validate the body or
correlate requests by CSeq. Header names are case-insensitive and order-independent;
the complete header must fit within 512 bytes of one packet. Sparse capture may
report unsupported offsets. This is not media-delivery or stream-reassembly proof.
See the [real Linux proof](../fixtures/gewyvern_rtsp_linux_x86_64_20261113.json).

For the broader family map, see
[docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `metadata`
- `rtsp-describe`
- `rtsp_describe`

<!-- gewyvern:entry-aliases:end -->
