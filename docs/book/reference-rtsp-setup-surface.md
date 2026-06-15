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

The current entry stops at successful `SETUP`. It does not attempt to collapse
setup and playback into one surface.

For the broader family map, see
[docs/book/reference-rtsp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `rtsp-setup`
- `rtsp_setup`
- `stream`

<!-- gewyvern:entry-aliases:end -->
