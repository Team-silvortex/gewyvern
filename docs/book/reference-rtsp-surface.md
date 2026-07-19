# Reference: RTSP Protocol Surface

Use this page when you want the RTSP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current RTSP coverage into three narrower operator-facing
surfaces:

- options and server capability probe
- describe and stream metadata lookup
- setup and play progression

## What This Shelf Covers

The current built-in RTSP family models a staged media-session conversation:

- establish the RTSP socket
- send `OPTIONS`
- receive `OPTIONS` success
- optionally send `DESCRIBE`
- optionally send `SETUP`
- optionally send `PLAY`

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for RTSP entry
selection:

- `rtsp-describe`
- `rtsp-options`
- `rtsp-play`
- `rtsp-setup`
- `rtsp_describe`
- `rtsp_options`
- `rtsp_play`
- `rtsp_setup`

Default entry: `options`

## RTSP Surface Map

### Options

- [docs/book/reference-rtsp-options-surface.md](docs/book/reference-rtsp-options-surface.md)
  Baseline RTSP connect and `OPTIONS` capability probe flow.

Typical entries:

- `options`

### Describe

- [docs/book/reference-rtsp-describe-surface.md](docs/book/reference-rtsp-describe-surface.md)
  `OPTIONS` plus `DESCRIBE` metadata lookup flow.

Typical entries:

- `describe`

### Setup And Play

- [docs/book/reference-rtsp-setup-surface.md](docs/book/reference-rtsp-setup-surface.md)
  Transport/session setup flow before media playback.
- [docs/book/reference-rtsp-play-surface.md](docs/book/reference-rtsp-play-surface.md)
  Full progression through `PLAY` acknowledgement.

Typical entries:

- `setup`
- `play`

## Reading Order

If you are validating current RTSP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md)
3. one narrower RTSP subpage for the stage you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the RTSP family in the current `1.5.0` line.
New RTSP conversation branches should prefer landing behind this shelf instead
of being linked from multiple higher-level pages independently.
