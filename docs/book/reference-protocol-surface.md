# Reference: Protocol Surface Contract

Use this page when you need the current contract candidate for the built-in
protocol shelf:

- how protocol packages are discovered
- how `--protocol <family> --entry <entry>` resolves
- what counts as a canonical entry versus an alias
- how default entries are chosen

This page is not a tutorial for writing `gewylang`.
For package authoring and debugging flow, see:

- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)

For the lowering contract after resolution, see:

- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

For the narrower built-in protocol family shelves, see:

- [docs/book/reference-protocol-volume.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)

For the current generated-style alias index, see:

- [docs/book/reference-protocol-alias-index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-alias-index.md)

## Book Path

This page is the front door to the protocol reference volume.

Read it first when you need:

- canonical family and entry identity
- alias behavior
- default-entry resolution
- the package-resolution contract

Then continue with:

- [docs/book/reference-protocol-volume.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)

## What The Protocol Surface Is

In the current repository shape, the protocol surface is the registry-style
shelf under [protocols](/Users/Shared/chroot/dev/gewyvern/protocols).

At a high level:

```text
protocol family + entry
  -> registry scan / manifest resolution
  -> canonical protocol name
  -> canonical entry name
  -> package root
  -> main.gewy
  -> binding / IR / runtime shell
```

The protocol surface is therefore more than a folder convention.
It is the lookup contract between:

- CLI users
- built-in validation scripts
- package authors
- future tooling that wants stable family/entry resolution

## Current Registry Shape

Each built-in protocol package is expected to look like:

1. one directory under [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
2. one family directory such as `smtp`, `ftp`, or `kerberos`
3. one entry directory such as `auth`, `data`, `session`, or `tgs`
4. one `gewy.pkg`
5. one `main.gewy`

The manifest and registry scan together provide:

- protocol family name
- entry name
- whether the entry is the default for that family
- protocol aliases
- entry aliases
- effective DSL path

<!-- gewyvern:protocol-surface-overview:start -->
## Current Surface Snapshot

- Built-in families: `33`
- Built-in canonical entries: `175`
- Family/default map:
  - `amqp` -> default `session` via [docs/book/reference-amqp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-amqp-surface.md)
  - `coap` -> default `get`
  - `dhcp` -> default `client`
  - `dns` -> default `udp` via [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
  - `ftp` -> default `session` via [docs/book/reference-ftp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ftp-surface.md)
  - `gtpu` -> default `echo`
  - `http` -> default `request` via [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
  - `http3` -> default `request` via [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
  - `https` -> default `connect`
  - `hy2` -> default `auth`
  - `imap` -> default `auth` via [docs/book/reference-imap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-imap-surface.md)
  - `kerberos` -> default `as`
  - `ldap` -> default `sync` via [docs/book/reference-ldap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ldap-surface.md)
  - `mdns` -> default `query`
  - `memcached` -> default `get` via [docs/book/reference-memcached-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-memcached-surface.md)
  - `mqtt` -> default `connect` via [docs/book/reference-mqtt-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mqtt-surface.md)
  - `mysql` -> default `session` via [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)
  - `ntp` -> default `client`
  - `pop3` -> default `auth` via [docs/book/reference-pop3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-pop3-surface.md)
  - `postgres` -> default `query` via [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
  - `quic` -> default `initial` via [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
  - `radius` -> default `access`
  - `redis` -> default `ping` via [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
  - `rtsp` -> default `options` via [docs/book/reference-rtsp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-rtsp-surface.md)
  - `sip` -> default `register` via [docs/book/reference-sip-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-sip-surface.md)
  - `smtp` -> default `session` via [docs/book/reference-smtp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-smtp-surface.md)
  - `snmp` -> default `get`
  - `socks5` -> default `session` via [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)
  - `ssdp` -> default `discovery`
  - `ssh` -> default `session` via [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)
  - `stun` -> default `binding`
  - `tls` -> default `client`
  - `wireguard` -> default `handshake`

<!-- gewyvern:protocol-surface-overview:end -->

## Canonical Names Versus Aliases

The current contract deliberately distinguishes between:

### Canonical protocol names

These are the stable family names used for listing and reporting.

Examples:

- `smtp`
- `ftp`
- `kerberos`
- `radius`

### Canonical entry names

These are the stable mode names the registry prefers to expose.

Examples:

- `smtp auth`
- `smtp mail`
- `smtp rcpt`
- `smtp data`
- `ftp session`
- `ftp list`
- `ftp retr`
- `ftp stor`
- `kerberos as`
- `kerberos tgs`
- `radius access`

### Entry aliases

Aliases exist to make CLI usage and migration friendlier, but they should not
replace canonical names in machine-facing review or documentation.

The complete current alias map is intentionally maintained in:

- [docs/book/reference-protocol-alias-index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-alias-index.md)

That page is meant to stay synchronized with the live registry-backed surface.
This page keeps only a short orientation sample:

- `smtp login -> auth`
- `ftp active-download -> active-retr`
- `kerberos ticket -> tgs`
- `ssh shell -> channel`
- `sip call -> invite`
- `mqtt qos2-complete -> pubcomp`
- `redis sorted-add -> zadd`

The practical rule is:

- humans may type aliases
- the shelf should report canonical entries

## Resolution Rules

The current protocol-resolution path is centered in
[src/protocol_profiles.rs](/Users/Shared/chroot/dev/gewyvern/src/protocol_profiles.rs).

Important behaviors:

### `protocol_names()`

Returns the canonical family list.

Tooling should treat this as a listing surface, not a place to discover alias
spelling.

### `protocol_entries(protocol)`

Returns canonical entry names for the chosen family.

Tooling should expect canonical names like `request`, `response`, `auth`,
`data`, `session`, `retr`, and `tgs`, not alias names like `login` or
`download`.

### `protocol_default_entry(protocol)`

Returns the default entry for a family.

The current expectation is:

- if the registry marks one entry as default, use it
- otherwise fall back to the lexicographically first canonical entry
- legacy built-in profile fallback still exists when registry scan is absent

### `resolve_protocol_profile(protocol, entry)`

This is the effective contract surface for CLI-style resolution.

It is expected to:

1. normalize protocol aliases to the canonical family
2. normalize entry aliases to the canonical entry
3. choose the default entry when `entry` is omitted
4. return the resolved package DSL path

## CLI Contract Candidate

The current user-facing resolution shape is:

```bash
cargo run -- --protocol <family> --entry <entry>
```

Or, when the family has a useful default entry:

```bash
cargo run -- --protocol <family>
```

The contract candidate here is:

- the CLI may accept aliases
- the resolved package should still map to one canonical family/entry pair
- validation and reporting should prefer the canonical pair

That keeps interactive use flexible without making the shelf itself fuzzy.

## Runtime API Catalog

The control-plane/runtime API now exposes the protocol shelf directly, without
requiring an active scan target first.

Use these paths when you want machine-stable protocol discovery:

- `GET /v1/protocols`
  - returns the current catalog with canonical families, default entries, alias
    lists, and per-family entry summaries
- `GET /v1/protocols/<protocol>`
  - returns the resolved summary for one canonical family
- `GET /v1/protocols/<protocol>/entries/<entry>/surface.json`
  - returns the selected entry surface, including sibling entries, aliases, and
    shelf metadata when available

This complements, rather than replaces:

- `/v1/latest/targets`
- `/v1/latest/targets/<path-segment>/protocol-surface.json`

The distinction is deliberate:

- `/v1/protocols...`
  - asks "what protocol surface does this runtime know how to resolve?"
- `/v1/latest/targets...`
  - asks "what protocol surface was attached to this concrete rendered target?"

For automation and external control-plane code, prefer:

1. `/v1/protocols` to discover canonical family and entry names
2. `/v1/protocols/<protocol>/entries/<entry>/surface.json` to inspect the
   shelf shape for one entry
3. `/v1/latest/targets` only after a scan/session has materialized target data

## What Should Stay Stable

For the current maturity line, these behaviors are deliberate:

- protocol packages are registry-discovered
- one family can expose multiple entries
- canonical family and entry names are the primary shelf vocabulary
- aliases are additive compatibility helpers
- entry listing prefers canonical names
- omitted entry resolution prefers an explicit default when available

These are good candidates for downstream tooling assumptions.

## What Is Still Allowed To Evolve

Still evolving:

- how many protocol aliases each family carries
- how broad alias coverage should be for operator convenience
- which long-tail protocols get dedicated shelves
- how much manifest metadata is exposed beyond basic family/entry resolution

Tooling should not depend on:

- every alias continuing forever
- alias ordering
- incidental directory naming beyond the resolved canonical family/entry pair

## Review Checklist

When adding or changing a protocol package, ask:

1. What is the canonical family name?
2. What is the canonical entry name?
3. Does this package deserve a new entry or is it only an alias of an existing
   one?
4. If an alias is added, does listing still return only the canonical entry?
5. If `--entry` is omitted, is the default still the one we actually want
   operators to land on?
6. Does registry validation still pass for the whole family shelf?

If those answers are fuzzy, the protocol surface is usually under-specified.

## Non-Goals For This Page

This page does not try to replace:

- package authoring guidance
- `gewylang` syntax reference
- IR lowering reference
- runtime diagnosis/export contracts

It exists to keep the protocol shelf legible as its own first-class project
surface instead of hiding those rules inside ad hoc tests, aliases, and
validation scripts.
