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

The current entry is stage-based. It tells you that the peer responded to
`DESCRIBE`, not that every later media transport step succeeded.
