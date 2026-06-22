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

## Machine-Readable Surface Semantics

The `protocol_surface("ssh", "auth-denied")` contract now publishes
`entry_semantics` so higher-level tooling can classify explicit auth rejection
without scraping this page.

Current denial semantics:

- `category = failure-path`
- `operator_focus = ssh authentication rejection after explicit auth request`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

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

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `login`
- `login-denied`
- `password`
- `password-denied`
- `ssh-auth`
- `ssh-auth-denied`
- `ssh_auth`
- `ssh_auth_denied`

<!-- gewyvern:entry-aliases:end -->
