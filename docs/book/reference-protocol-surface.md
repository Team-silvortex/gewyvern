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

Current examples include:

- `smtp login -> auth`
- `smtp sender -> mail`
- `smtp recipient -> rcpt`
- `smtp message -> data`
- `smtp recipient-denied -> rcpt-denied`
- `smtp message-denied -> data-denied`
- `ftp login -> session`
- `ftp control -> session`
- `ftp directory -> list`
- `ftp download -> retr`
- `ftp upload -> stor`
- `ftp active-directory -> active-list`
- `ftp active-download -> active-retr`
- `ftp active-upload -> active-stor`
- `ftp login-denied -> denied`
- `kerberos login -> as`
- `kerberos initial-auth -> as`
- `kerberos ticket -> tgs`
- `kerberos service-ticket -> tgs`
- `kerberos login-denied -> as-error`
- `radius login -> access`
- `radius auth -> access`
- `ssh connect -> session`
- `ssh login -> auth`
- `ssh shell -> channel`
- `socks5 proxy -> session`
- `socks5 userpass -> auth`
- `socks5 connect-denied -> denied`
- `ldap login -> bind`
- `ldap directory -> search`
- `ldap directory-session -> session`
- `ldap replication -> sync`
- `snmp query -> get`
- `sip login -> register`
- `sip call -> invite`
- `sip hangup -> bye`
- `sip terminate -> bye`
- `rtsp probe -> options`
- `rtsp metadata -> describe`
- `rtsp stream -> setup`
- `rtsp start -> play`
- `amqp login -> start`
- `amqp connect -> session`
- `amqp send -> publish`
- `amqp receive -> consume`
- `amqp deliver -> consume`
- `postgres query-session -> session`
- `postgres auth-query -> session`
- `mqtt session -> connect`
- `mqtt send -> publish`
- `mqtt message -> publish`
- `mqtt read -> subscribe`
- `mqtt listen -> subscribe`
- `mqtt qos2-receipt -> pubrec`
- `mqtt stage-2 -> pubrec`
- `mqtt qos2-release -> pubrel`
- `mqtt resume -> pubrel`
- `mqtt qos2-complete -> pubcomp`
- `mqtt complete -> pubcomp`
- `mqtt close -> disconnect`
- `mqtt teardown -> disconnect`
- `redis connect -> session`
- `redis delete -> del`
- `redis remove -> del`
- `redis health -> ping`
- `redis read -> get`
- `redis kv-read -> get`
- `redis write -> set`
- `redis kv-write -> set`
- `redis increment -> incr`
- `redis count-up -> incr`
- `redis decrement -> decr`
- `redis count-down -> decr`
- `redis multi-read -> mget`
- `redis bulk-read -> mget`
- `redis multi-write -> mset`
- `redis bulk-write -> mset`
- `redis present -> exists`
- `redis key-check -> exists`
- `redis set-ttl -> expire`
- `redis expiry -> expire`
- `redis time-to-live -> ttl`
- `redis key-ttl -> ttl`
- `redis precise-ttl -> pttl`
- `redis ms-ttl -> pttl`
- `redis hash-read -> hget`
- `redis field-read -> hget`
- `redis hash-write -> hset`
- `redis field-write -> hset`
- `redis hash-multi-read -> hmget`
- `redis fields-read -> hmget`
- `redis hash-multi-write -> hmset`
- `redis fields-write -> hmset`
- `redis list-prepend -> lpush`
- `redis left-push -> lpush`
- `redis list-append -> rpush`
- `redis right-push -> rpush`
- `redis list-pop-left -> lpop`
- `redis left-pop -> lpop`
- `redis list-pop-right -> rpop`
- `redis right-pop -> rpop`
- `sip session -> invite`

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
