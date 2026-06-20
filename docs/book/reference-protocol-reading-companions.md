# Reference: Protocol Reading Companions

Use this page when a protocol surface is not enough on its own and you need the
runtime's shortest machine-readable hint for "what should I read next?"

The current API and persisted protocol-surface artifacts now expose:

- `selected_overlay`
- `overlays`
- `reading_companions`

Treat them as three different layers:

- `selected_overlay`
  - says which overlay name was intentionally selected by the caller
- `overlays`
  - lists the nearby overlay interpretations the current canonical surface can
    carry
- `reading_companions`
  - lists the next canonical protocol/entry pairs an operator or UI should jump
    to when the current surface depends on a second shelf

## Current Companion Patterns

The `0.15.x` line currently emits companions for these families:

- `dot` / `dns tcp` -> `tls client`
- `doh` / `http request` -> `dns tcp`
- `http connect` tunnel surfaces -> `tls client`
- `smtp` / `imap` / `pop3` auth surfaces -> `tls client`
- `https connect` -> `tls client`
- `tls client` -> `https connect`, `dns tcp`
- `http3 request|server` -> `quic initial`
- `quic initial|crypto|stream|bidi` -> `http3 request`

These links are intentionally compact:

- they do not try to encode every possible future application on top of TLS or
  QUIC
- they only point at the nearest canonical shelf that tends to unblock reading
  and diagnosis

## API And Persistence Shape

Machine-facing paths that now carry `reading_companions` include:

- `/v1/protocols/<protocol>/entries/<entry>/surface.json`
- `/v1/latest/targets/<path-segment>/protocol-surface.json`
- scan report JSON `protocol_surface`

Each companion row currently carries:

- `protocol`
- `entry`
- `via_overlay`
- `via_label`

That means a UI can render a stable jump target without having to infer the
relationship from human prose.

## Reading Rule Of Thumb

When a surface exposes `reading_companions`, use this order:

1. read the current canonical surface first
2. check `selected_overlay` if the caller intentionally asked for an overlay-led
   view such as `dot` or `http-connect`
3. follow `reading_companions` in listed order when transport or handshake
   posture is more likely to explain the failure than payload semantics

For the broader routing spine that surrounds these companion jumps, keep nearby:

- [docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-runtime-layout.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-layout.md)
