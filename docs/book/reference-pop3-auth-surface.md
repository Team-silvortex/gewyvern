# Reference: POP3 Authentication Surface

Use this page when you need the current exact lookup surface for POP3 login
behavior.

## Covered Entries

### `auth`

- Protocol:
  `pop3`
- Aliases:
  `login`
- Default entry:
  yes

### `auth-denied`

- Protocol:
  `pop3`
- Aliases:
  `login-denied`
- Default entry:
  no

## Operational Shape

The current authentication family extends the POP3 session banner with a
`USER`/`PASS` exchange and one of two outcomes.

### Success Branch

The `auth` entry models:

1. bind the process and resolve the upstream route
2. observe the POP3 socket transition
3. receive the POP3 banner
4. send `USER`
5. receive user acknowledgement
6. send `PASS`
7. receive auth success

### Denial Branch

The `auth-denied` entry models:

1. bind the process and resolve the upstream route
2. observe the POP3 socket transition
3. receive the POP3 banner
4. send `USER`
5. receive user acknowledgement
6. send `PASS`
7. receive auth denied

Use the success branch when you want authenticated mailbox posture. Use the
denial branch when you need an explicit failed-password interpretation.

## Operator Reading Order

Read this page after the POP3 family hub when:

- you need to distinguish successful login from denied login
- you want the `login` or `login-denied` alias behavior
- you do not yet care about mailbox listing

## Stability Notes

The current family is outcome-based and intentionally minimal. It models the
classic `USER`/`PASS` path rather than richer auth extensions.
