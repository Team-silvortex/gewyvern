# Reference: SSH Authentication Surface

Use this page when you need the current exact lookup surface for SSH
authentication behavior.

## Covered Entries

### `auth`

- Protocol:
  `ssh`
- Aliases:
  `login`, `password`
- Default entry:
  no

### `auth-denied`

- Protocol:
  `ssh`
- Aliases:
  `login-denied`, `password-denied`
- Default entry:
  no

## Operational Shape

The current authentication family extends the base SSH session with an auth
request and one of two server outcomes.

### Success Branch

The `auth` entry models:

1. session connect and banner exchange
2. send key exchange init
3. send auth request
4. receive auth success

### Denial Branch

The `auth-denied` entry models:

1. session connect and banner exchange
2. send key exchange init
3. send auth request
4. receive auth denied

Use the success branch when you want a positive authenticated posture. Use the
denial branch when you want an explicit failed-login interpretation rather than
just a short or incomplete session.

## Operator Reading Order

Read this page after the SSH family hub when:

- you need to distinguish successful auth from denied auth
- you are validating alias lookups such as `login` or `password`
- you want a narrower shelf than the full channel-open path

## Stability Notes

The current family is outcome-based rather than method-based. It tells you that
an auth request succeeded or failed, not which higher-level auth mechanism was
used.

For the broader family map, see
[docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md).
