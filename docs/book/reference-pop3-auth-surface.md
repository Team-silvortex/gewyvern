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

The current authentication family has packet-local candidates for a POP3 banner,
`USER`/`PASS` requests and explicit outcome text. It is not a stateful
command/reply correlator: the stages below are a reference shape, not a promise
that every stage is observable or that the whole exchange was completed.

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

## Machine-Readable Surface Semantics

The `protocol_surface("pop3", "auth-denied")` contract now publishes
`entry_semantics` so higher-level tooling can identify explicit password
rejection without scraping this page.

Current denial semantics:

- `category = failure-path`
- `operator_focus = explicit POP3 password rejection candidate, not generic -ERR`
- `typical_signal = -ERR invalid password`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## Operator Reading Order

Read this page after the POP3 family hub when:

- you need to distinguish successful login from denied login
- you want the `login` or `login-denied` alias behavior
- you do not yet care about mailbox listing

## Stability Notes

The conservative response signatures currently recognize complete English lines:
`+OK Mailpit POP3 server`, `+OK User accepted`, `+OK signed in`, and
`-ERR invalid password`, each followed by CRLF. Generic `+OK` is not a user or
authentication success candidate: Mailpit uses that same response for USER,
NOOP and RSET. `-ERR user not authenticated` and `-ERR no such message` are not
password rejection. Alternative/localized text and split status lines may be
unmatched; full-dialogue correlation and TLS/APOP/SASL remain separate work.

The bounded Linux fixture checks the complete USER/PASS conversation independently
and compares all received bytes to eBPF evidence. Its POP3 mailbox retrieval,
dot-unstuffing and DELE/RSET checks are validation behavior, not new product
retrieval/deletion stages. See [field findings](../field-findings.md).

For the broader family map, see
[docs/book/reference-pop3-surface.md](docs/book/reference-pop3-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `login`
- `login-denied`
- `pop3-auth`
- `pop3-auth-denied`
- `pop3_auth`
- `pop3_auth_denied`

<!-- gewyvern:entry-aliases:end -->
