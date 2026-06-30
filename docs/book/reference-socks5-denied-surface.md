# Reference: SOCKS5 Denied Surface

Use this page when you need the current exact lookup surface for SOCKS5 denial
branches.

## Covered Entries

### `denied`

- Protocol:
  `socks5`
- Aliases:

- `connect-denied`
- Default entry:
  no

Operational shape:

- negotiate a no-auth SOCKS5 session
- send a connect request
- receive connect denial

### `auth-denied`

- Protocol:
  `socks5`
- Aliases:

- `login-denied`
- `userpass-denied`
- Default entry:
  no

Operational shape:

- negotiate username/password auth
- send auth request
- receive auth denial

### `auth-connect-denied`

- Protocol:
  `socks5`
- Aliases:

- `login-connect-denied`
- `userpass-connect-denied`
- Default entry:
  no

Operational shape:

- negotiate username/password auth
- receive auth success
- send a connect request
- receive connect denial

## Operational Shape

The denial-oriented entries currently share a branch-driven staging model:

1. process binding
2. route resolution
3. SOCKS5 socket connect
4. method greeting and selection
5. optional auth exchange
6. connect request or auth branch
7. terminal denial

The current branch points are:

- `denied`: connect denial after no-auth method selection
- `auth-denied`: denial during username/password auth
- `auth-connect-denied`: denial after auth succeeds but connect fails

## Machine-Readable Surface Semantics

The `protocol_surface("socks5", entry)` contract now publishes
`entry_semantics` for the denial-oriented entries on this page so downstream
tools can classify them without scraping prose.

Current semantics posture:

- `denied`
  - `category = failure-path`
  - `operator_focus = upstream connect refusal after no-auth method selection`
  - `primary_failure_mode = server_denied`
  - `primary_failure_detail = access_denied`
  - `primary_failure_basis = direct_protocol_signal`
- `auth-denied`
  - `category = failure-path`
  - `operator_focus = username/password rejection during proxy auth exchange`
  - `primary_failure_mode = server_denied`
  - `primary_failure_detail = access_denied`
  - `primary_failure_basis = direct_protocol_signal`
- `auth-connect-denied`
  - `category = failure-path`
  - `operator_focus = upstream connect refusal after authenticated proxy setup`
  - `primary_failure_mode = server_denied`
  - `primary_failure_detail = access_denied`
  - `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

If you are reviewing SOCKS5 denial coverage, read it in this order:

1. `session`
2. `auth`
3. `denied`
4. `auth-denied`
5. `auth-connect-denied`

That sequence keeps the success-oriented base paths in view before the denial
branches.

## Stability Notes

The current shelf keeps denial-oriented branches together so operators can
separate successful session setup from terminal auth or connect failure without
duplicating the success-path pages.

For the broader family map, see
[docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `connect-denied`
- `login-connect-denied`
- `userpass-connect-denied`

<!-- gewyvern:entry-aliases:end -->
