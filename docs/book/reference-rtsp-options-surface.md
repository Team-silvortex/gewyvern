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

The current entry is intentionally early-phase. It models successful probe flow
without trying to infer later media-session intent.
