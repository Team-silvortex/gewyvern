# Reference: IMAP Authentication Surface

Use this page when you need the current exact lookup surface for IMAP login
behavior.

## Covered Entries

### `auth`

- Protocol:
  `imap`
- Aliases:
  `login`
- Default entry:
  yes

### `auth-denied`

- Protocol:
  `imap`
- Aliases:
  `login-denied`
- Default entry:
  no

## Operational Shape

The current authentication family extends the basic IMAP session with a login
request and one of two server outcomes.

### Success Branch

The `auth` entry models:

1. bind the process and resolve the upstream route
2. observe the IMAP socket transition
3. receive the IMAP banner
4. send `LOGIN`
5. receive `OK`

### Denial Branch

The `auth-denied` entry models:

1. bind the process and resolve the upstream route
2. observe the IMAP socket transition
3. receive the IMAP banner
4. send `LOGIN`
5. receive `NO`

Use the success branch when you want an authenticated mailbox posture. Use the
denial branch when you need an explicit failed-login interpretation.

## Operator Reading Order

Read this page after the IMAP family hub when:

- you need to distinguish successful login from denied login
- you want the `login` or `login-denied` alias behavior
- you do not yet care about mailbox selection

## Stability Notes

The current family is outcome-based rather than auth-mechanism-based. It tells
you that `LOGIN` succeeded or failed, not which richer auth extension was used.
