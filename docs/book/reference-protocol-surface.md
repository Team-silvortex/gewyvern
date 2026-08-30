# Reference: Protocol Surface Contract

Use this page when you need the current contract candidate for the built-in
protocol shelf:

- how protocol packages are discovered
- how `--protocol <family> --entry <entry>` resolves
- what counts as a canonical entry versus an alias
- how default entries are chosen

This page is not a tutorial for writing `gewylang`.
For package authoring and debugging flow, see:

- [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)
- [docs/book/reference-gewylang-package.md](docs/book/reference-gewylang-package.md)

For the lowering contract after resolution, see:

- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

For the narrower built-in protocol family shelves, see:

- [docs/book/reference-protocol-standard-library.md](docs/book/reference-protocol-standard-library.md)
- [docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)

For the current generated-style alias index, see:

- [docs/book/reference-protocol-alias-index.md](docs/book/reference-protocol-alias-index.md)

## Book Path

This page is the front door to the protocol reference volume.

Read it first when you need:

- canonical family and entry identity
- alias behavior
- default-entry resolution
- the package-resolution contract

Then continue with:

- [docs/book/reference-protocol-standard-library.md](docs/book/reference-protocol-standard-library.md)
- [docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)

## What The Protocol Surface Is

In the current repository shape, the protocol surface is the registry-style
shelf under [protocols](protocols).

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

1. one directory under [protocols](protocols)
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

### Strict Directory Validation

Call `validate_protocol_registry_dir(path)` when a caller supplies an explicit
registry directory. The strict API returns either the complete resolved target
set or the original bounded scanner error; it never collapses a malformed
manifest into an empty catalog. CLI `--scan-set <directory>` uses this path and
therefore fails closed with the offending manifest diagnostic.

For compatibility, `default_protocol_scan_set_from_dir(path)` retains its
older `Option` shape. New package tooling should prefer strict validation.

Every manifest entry must be a normalized relative path to a regular file
inside its package directory. Absolute paths, `..`, symlinks in the entry path,
and package-root escapes are rejected before a target is exposed.

### Request-Local Catalog Snapshots

Use `ProtocolCatalogSnapshot::discover()` when one operation needs more than
one catalog lookup. A snapshot scans the registry once and can then serve
protocol summaries, individual summaries, profile resolution, and the default
scan set without touching the directory again.

Snapshots are deliberately request-local and immutable. They are not installed
as a process-wide cache: an existing snapshot remains internally consistent,
while the next `discover()` observes package changes. The standalone helper
functions remain compatible and perform fresh discovery for each call.

The scanner itself uses a bounded iterative traversal. It ignores symlinked
directories, accepts only regular `gewy.pkg` files, validates package entries
component by component, and processes directory names in stable order so strict
diagnostics are reproducible. One scan is capped at 4,096 directories, 16,384
directory entries, 2,048 manifests, and 64 KiB of actual manifest content per
file; exceeding any budget fails the strict scan.

For surfaces that carry explicit failure or denial posture, the current
machine-facing protocol surface can also include:

- `entry_semantics.category`
- `entry_semantics.operator_focus`
- `entry_semantics.typical_signal`
- `entry_semantics.primary_failure_mode`
- `entry_semantics.primary_failure_detail`
- `entry_semantics.primary_failure_basis`

<!-- gewyvern:protocol-surface-overview:start -->
## Current Surface Snapshot

- Built-in families: `70`
- Built-in canonical entries: `361`
- Family/default map:
  - `amqp` -> default `session` in cluster `cache-queue-stream` via [docs/book/reference-amqp-surface.md](docs/book/reference-amqp-surface.md)
  - `arp` -> default `request` in cluster `network-control-discovery` via [docs/book/reference-arp-surface.md](docs/book/reference-arp-surface.md)
  - `bgp` -> default `open` in cluster `network-control-discovery` via [docs/book/reference-bgp-surface.md](docs/book/reference-bgp-surface.md)
  - `cassandra` -> default `query` in cluster `database-query-session` via [docs/book/reference-cassandra-surface.md](docs/book/reference-cassandra-surface.md)
  - `coap` -> default `get` in cluster `network-control-discovery` via [docs/book/reference-coap-surface.md](docs/book/reference-coap-surface.md)
  - `consul` -> default `service` in cluster `database-query-session` via [docs/book/reference-consul-surface.md](docs/book/reference-consul-surface.md)
  - `dhcp` -> default `client` in cluster `network-control-discovery` via [docs/book/reference-dhcp-surface.md](docs/book/reference-dhcp-surface.md)
  - `dhcpv6` -> default `solicit` in cluster `network-control-discovery` via [docs/book/reference-dhcpv6-surface.md](docs/book/reference-dhcpv6-surface.md)
  - `dns` -> default `udp` in cluster `network-control-discovery` via [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
  - `elasticsearch` -> default `search` in cluster `database-query-session` via [docs/book/reference-elasticsearch-surface.md](docs/book/reference-elasticsearch-surface.md)
  - `etcd` -> default `range` in cluster `database-query-session` via [docs/book/reference-etcd-surface.md](docs/book/reference-etcd-surface.md)
  - `ftp` -> default `session` in cluster `session-control-media-transfer` via [docs/book/reference-ftp-surface.md](docs/book/reference-ftp-surface.md)
  - `geneve` -> default `encap` in cluster `network-control-discovery` via [docs/book/reference-geneve-surface.md](docs/book/reference-geneve-surface.md)
  - `graphql` -> default `query` in cluster `web-proxy-request-response` via [docs/book/reference-graphql-surface.md](docs/book/reference-graphql-surface.md)
  - `gre` -> default `encap` in cluster `network-control-discovery` via [docs/book/reference-gre-surface.md](docs/book/reference-gre-surface.md)
  - `grpc` -> default `call` in cluster `web-proxy-request-response` via [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
  - `gtpu` -> default `echo` in cluster `network-control-discovery` via [docs/book/reference-gtpu-surface.md](docs/book/reference-gtpu-surface.md)
  - `http` -> default `request` in cluster `web-proxy-request-response` via [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
  - `http3` -> default `request` in cluster `web-proxy-request-response` via [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
  - `https` -> default `connect` in cluster `web-proxy-request-response` via [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
  - `hy2` -> default `auth` in cluster `secure-transport-session` via [docs/book/reference-hy2-surface.md](docs/book/reference-hy2-surface.md)
  - `icmp` -> default `echo` in cluster `network-control-discovery` via [docs/book/reference-icmp-surface.md](docs/book/reference-icmp-surface.md)
  - `icmpv6` -> default `echo` in cluster `network-control-discovery` via [docs/book/reference-icmpv6-surface.md](docs/book/reference-icmpv6-surface.md)
  - `imap` -> default `auth` in cluster `mail-delivery-mailbox` via [docs/book/reference-imap-surface.md](docs/book/reference-imap-surface.md)
  - `ipsec` -> default `esp` in cluster `secure-transport-session` via [docs/book/reference-ipsec-surface.md](docs/book/reference-ipsec-surface.md)
  - `jaeger` -> default `collector` in cluster `web-proxy-request-response` via [docs/book/reference-jaeger-surface.md](docs/book/reference-jaeger-surface.md)
  - `kafka` -> default `metadata` in cluster `cache-queue-stream` via [docs/book/reference-kafka-surface.md](docs/book/reference-kafka-surface.md)
  - `kerberos` -> default `as` in cluster `identity-directory-access` via [docs/book/reference-kerberos-surface.md](docs/book/reference-kerberos-surface.md)
  - `l2tp` -> default `control` in cluster `network-control-discovery` via [docs/book/reference-l2tp-surface.md](docs/book/reference-l2tp-surface.md)
  - `ldap` -> default `sync` in cluster `identity-directory-access` via [docs/book/reference-ldap-surface.md](docs/book/reference-ldap-surface.md)
  - `llmnr` -> default `query` in cluster `network-control-discovery` via [docs/book/reference-llmnr-surface.md](docs/book/reference-llmnr-surface.md)
  - `loki` -> default `push` in cluster `web-proxy-request-response` via [docs/book/reference-loki-surface.md](docs/book/reference-loki-surface.md)
  - `mdns` -> default `query` in cluster `network-control-discovery` via [docs/book/reference-mdns-surface.md](docs/book/reference-mdns-surface.md)
  - `memcached` -> default `get` in cluster `cache-queue-stream` via [docs/book/reference-memcached-surface.md](docs/book/reference-memcached-surface.md)
  - `mongodb` -> default `command` in cluster `database-query-session` via [docs/book/reference-mongodb-surface.md](docs/book/reference-mongodb-surface.md)
  - `mqtt` -> default `connect` in cluster `cache-queue-stream` via [docs/book/reference-mqtt-surface.md](docs/book/reference-mqtt-surface.md)
  - `mssql` -> default `query` in cluster `database-query-session` via [docs/book/reference-mssql-surface.md](docs/book/reference-mssql-surface.md)
  - `mysql` -> default `session` in cluster `database-query-session` via [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
  - `nats` -> default `connect` in cluster `cache-queue-stream` via [docs/book/reference-nats-surface.md](docs/book/reference-nats-surface.md)
  - `nbns` -> default `query` in cluster `network-control-discovery` via [docs/book/reference-nbns-surface.md](docs/book/reference-nbns-surface.md)
  - `ndp` -> default `solicit` in cluster `network-control-discovery` via [docs/book/reference-ndp-surface.md](docs/book/reference-ndp-surface.md)
  - `ntp` -> default `client` in cluster `network-control-discovery` via [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md)
  - `ospf` -> default `hello` in cluster `network-control-discovery` via [docs/book/reference-ospf-surface.md](docs/book/reference-ospf-surface.md)
  - `otlp` -> default `traces` in cluster `web-proxy-request-response` via [docs/book/reference-otlp-surface.md](docs/book/reference-otlp-surface.md)
  - `pop3` -> default `auth` in cluster `mail-delivery-mailbox` via [docs/book/reference-pop3-surface.md](docs/book/reference-pop3-surface.md)
  - `postgres` -> default `query` in cluster `database-query-session` via [docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md)
  - `pptp` -> default `control` in cluster `network-control-discovery` via [docs/book/reference-pptp-surface.md](docs/book/reference-pptp-surface.md)
  - `prometheus` -> default `scrape` in cluster `web-proxy-request-response` via [docs/book/reference-prometheus-surface.md](docs/book/reference-prometheus-surface.md)
  - `quic` -> default `initial` in cluster `secure-transport-session` via [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)
  - `radius` -> default `access` in cluster `identity-directory-access` via [docs/book/reference-radius-surface.md](docs/book/reference-radius-surface.md)
  - `rdp` -> default `connect` in cluster `identity-directory-access` via [docs/book/reference-rdp-surface.md](docs/book/reference-rdp-surface.md)
  - `redis` -> default `ping` in cluster `cache-queue-stream` via [docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md)
  - `rip` -> default `request` in cluster `network-control-discovery` via [docs/book/reference-rip-surface.md](docs/book/reference-rip-surface.md)
  - `rtsp` -> default `options` in cluster `session-control-media-transfer` via [docs/book/reference-rtsp-surface.md](docs/book/reference-rtsp-surface.md)
  - `s3` -> default `get-object` in cluster `web-proxy-request-response` via [docs/book/reference-s3-surface.md](docs/book/reference-s3-surface.md)
  - `sip` -> default `register` in cluster `session-control-media-transfer` via [docs/book/reference-sip-surface.md](docs/book/reference-sip-surface.md)
  - `smb` -> default `negotiate` in cluster `identity-directory-access` via [docs/book/reference-smb-surface.md](docs/book/reference-smb-surface.md)
  - `smtp` -> default `session` in cluster `mail-delivery-mailbox` via [docs/book/reference-smtp-surface.md](docs/book/reference-smtp-surface.md)
  - `snmp` -> default `get` in cluster `network-control-discovery` via [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)
  - `socks5` -> default `session` in cluster `web-proxy-request-response` via [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)
  - `ssdp` -> default `discovery` in cluster `network-control-discovery` via [docs/book/reference-ssdp-surface.md](docs/book/reference-ssdp-surface.md)
  - `ssh` -> default `session` in cluster `identity-directory-access` via [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
  - `stun` -> default `binding` in cluster `network-control-discovery` via [docs/book/reference-stun-surface.md](docs/book/reference-stun-surface.md)
  - `syslog` -> default `udp` in cluster `web-proxy-request-response` via [docs/book/reference-syslog-surface.md](docs/book/reference-syslog-surface.md)
  - `tftp` -> default `read` in cluster `network-control-discovery` via [docs/book/reference-tftp-surface.md](docs/book/reference-tftp-surface.md)
  - `tls` -> default `client` in cluster `secure-transport-session` via [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
  - `vxlan` -> default `encap` in cluster `network-control-discovery` via [docs/book/reference-vxlan-surface.md](docs/book/reference-vxlan-surface.md)
  - `websocket` -> default `upgrade` in cluster `web-proxy-request-response` via [docs/book/reference-websocket-surface.md](docs/book/reference-websocket-surface.md)
  - `wireguard` -> default `handshake` in cluster `network-control-discovery` via [docs/book/reference-wireguard-surface.md](docs/book/reference-wireguard-surface.md)
  - `zookeeper` -> default `read` in cluster `database-query-session` via [docs/book/reference-zookeeper-surface.md](docs/book/reference-zookeeper-surface.md)

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

- [docs/book/reference-protocol-alias-index.md](docs/book/reference-protocol-alias-index.md)

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
[src/protocol_profiles.rs](src/protocol_profiles.rs).

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

For a batch, call the equivalent method on one `ProtocolCatalogSnapshot`
instead of repeatedly calling the standalone helper.

### `protocol_surface(protocol, entry)`

This is the richer lookup surface used by runtime JSON, reports, and nearby
control planes.

It is expected to expose:

- canonical protocol and entry identity
- family-level cluster hints
- shelf grouping and reading page
- overlay companions when a family rides on another transport
- `entry_semantics` when one entry already carries a stable denial or
  failure-oriented diagnosis contract

Redis failure entries, first tightened during `0.16.x`, established the
current machine-facing `entry_semantics` contract rather than leaving those
semantics only as prose in the book.

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

The API now also exposes `cluster_hint` alongside protocol summaries and entry
surfaces:

- `cluster_hint`
  - coarse protocol-cluster guidance for operators and UIs
  - answers "what broader family shape does this protocol belong to?"
  - carries an operator-facing hint and sibling protocols in the same cluster
- `shelf`
  - entry-local organization inside one canonical protocol family
  - answers "which nearby entries should I read together for this protocol?"

Treat them as complementary:

- `cluster_hint` is for family-level orientation
- `shelf` is for entry-level drill-down

The current line also exposes overlay-led reading helpers on entry surfaces:

- `selected_overlay`
  - records whether the caller intentionally entered the surface through an
    overlay-facing name such as `dot`, `doh`, or `http-connect`
- `overlays`
  - lists the overlay interpretations the current canonical surface can support
- `reading_companions`
  - lists the next canonical protocol/entry pairs a UI or operator should read
    when the current surface depends on a second shelf such as `tls client` or
    `quic initial`

Use the companion contract when you want structured drill-down rules rather than
prose-only guidance:

- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)

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
   shelf shape, overlays, and reading companions for one entry
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
