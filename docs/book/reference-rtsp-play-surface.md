# Reference: RTSP Play Surface

Use this page when you need the current exact lookup surface for RTSP playback
start behavior.

## Covered Entries

### `play`

- Protocol:
  `rtsp`
- Aliases:
  `start`
- Family aliases:
  `rtsp-play`, `rtsp_play`
- Default entry:
  no

## Operational Shape

The current `play` flow extends the setup path into playback start:

1. establish the RTSP socket
2. send `OPTIONS`
3. receive `OPTIONS` success
4. send `DESCRIBE`
5. receive `DESCRIBE` success
6. send `SETUP`
7. receive `SETUP` success
8. send `PLAY`
9. receive `PLAY` success with range/playback response

This is the deepest RTSP shelf in the current family and is the right page when
you want to prove that the conversation progressed into actual playback start.

## Operator Reading Order

Read this page after the RTSP family hub when:

- you need the `start` alias behavior
- you want the deepest currently modeled RTSP conversation
- you are validating end-to-end staged progression before IR lowering

## Stability Notes

The current entry is still control-plane oriented. It models successful `PLAY`
startup rather than the later sustained RTP media plane.

For the broader family map, see
[docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `rtsp-play`
- `rtsp_play`
- `start`

<!-- gewyvern:entry-aliases:end -->
