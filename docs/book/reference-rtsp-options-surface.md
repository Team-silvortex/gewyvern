# Reference: RTSP Options Surface

Use this page when you need the current exact lookup surface for RTSP
capability-probe behavior.

## Covered Entries

### `options`

- Protocol:
  `rtsp`
- Aliases:
  `probe`
- Family aliases:
  `rtsp-options`, `rtsp_options`
- Default entry:
  yes

## Operational Shape

The current `options` flow models:

1. bind the process and resolve the upstream route
2. observe the RTSP socket transition and established state
3. send `OPTIONS`
4. receive `200 OK` for `OPTIONS`

This is the narrowest RTSP page to use when you only care about proving that a
remote endpoint answered the initial capability probe.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `rtsp` resolves to its default entry
- you want the `probe` alias behavior
- you do not yet care about media metadata or playback session setup

## Stability Notes

The receive stage is a packet-local RTSP/1.0 `200` candidate with decimal `CSeq`
and a `Public` method list. It accepts reordered/case-insensitive headers, but
requires a complete header within the first 512 bytes of one packet. It does
not correlate requests by CSeq or infer media delivery. Default sparse capture
may report unsupported offsets; use an adequately sampled prefix, never a
fabricated success. See the [real Linux proof](../fixtures/gewyvern_rtsp_linux_x86_64_20261113.json).

For the broader family map, see
[docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `probe`
- `rtsp-options`
- `rtsp_options`

<!-- gewyvern:entry-aliases:end -->
