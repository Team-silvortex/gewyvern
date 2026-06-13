# Reference: SIP Bye Surface

Use this page when you need the current exact lookup surface for SIP call
termination behavior.

## Covered Entries

### `bye`

- Protocol:
  `sip`
- Aliases:
  `hangup`, `terminate`
- Default entry:
  no

## Operational Shape

The current `bye` flow models:

1. bind the process and resolve the upstream route
2. send `BYE` over UDP
3. receive a SIP response

This is the narrowest SIP page to use when you want explicit teardown posture
instead of registration or call setup.

## Operator Reading Order

Read this page after the SIP family hub when:

- you need the `hangup` or `terminate` alias behavior
- you want to distinguish call teardown from `INVITE`
- you care about end-of-session SIP posture before IR lowering

## Stability Notes

The current entry records the coarse teardown exchange shape. It does not try
to model every dialog-state nuance around the final response.
