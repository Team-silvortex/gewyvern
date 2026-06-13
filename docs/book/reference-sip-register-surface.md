# Reference: SIP Register Surface

Use this page when you need the current exact lookup surface for SIP
registration behavior.

## Covered Entries

### `register`

- Protocol:
  `sip`
- Aliases:
  `login`
- Default entry:
  yes

## Operational Shape

The current `register` flow models:

1. bind the process and resolve the upstream route
2. send `REGISTER` over UDP
3. receive a SIP response

This is the narrowest SIP page to use when you want the default registration
posture without implying call setup or teardown.

## Operator Reading Order

Read this page after the generic protocol surface when:

- you are checking whether `sip` resolves to its default entry
- you want the `login` alias behavior
- you only care about registration posture

## Stability Notes

The current entry is intentionally coarse. It models registration exchange
shape, not detailed SIP response-class or authentication challenge behavior.
