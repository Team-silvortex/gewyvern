# Reference: Protocol Standard Library Layout

This page defines the project organization contract for built-in protocol
support. Treat it as the "standard library" layout for Gewyvern protocol work:
new protocols should land in predictable shelves instead of growing one large
catch-all module.

For the runtime resolution contract, see:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Standard Library Shape

Built-in protocol support has four layers:

```text
protocols/<family>/<entry>/
  -> package manifest and canonical main.gewy
dsl/*.gewy
  -> compatibility-oriented development DSL entrypoints
src/protocol_profiles/*.rs
  -> Rust registry, aliases, shelves, clusters, and semantics
docs/book/reference-*-surface.md
  -> human reference spine
```

The rule of thumb is simple:

- `protocols/` is the canonical package shelf.
- `dsl/` is the compatibility and development shelf.
- `src/protocol_profiles/` is the lookup and metadata shelf.
- `docs/book/` is the reference shelf.

## Registry Shelves

The protocol profile registry is split by capability domain:

- `web_protocols.rs` covers DNS, HTTP, HTTPS, and HTTP/3 style request and
  response surfaces.
- `secure_transport.rs` covers secure session and tunnel transports such as
  TLS, QUIC, Hysteria 2, and WireGuard.
- `network_control.rs` covers discovery, control-plane, neighbor, routing, and
  low-level diagnostic protocols such as STUN, CoAP, NTP, DHCP, ARP, ICMP,
  ICMPv6, NDP, BGP, OSPF, mDNS, and SSDP.
- `data_and_queue.rs` covers databases, cache, queue, stream, and mobile core
  data protocols.
- `access_and_media.rs` covers access, proxy, signaling, and media-transfer
  protocols.
- `mail_and_directory.rs` covers mail, directory, and identity-oriented
  protocol profiles.

When adding a protocol, prefer the narrowest shelf that matches the operator
mental model. If a protocol spans multiple concerns, place its profile where
debuggers first look for it, then use clusters and docs to expose secondary
relationships.

## Package Shelf

Every built-in protocol entry should have a package-shaped canonical location:

```text
protocols/<family>/<entry>/gewy.pkg
protocols/<family>/<entry>/main.gewy
```

The family name and entry name should be lowercase, stable, and suitable for
machine-facing output.

Use aliases for human convenience, but keep canonical names boring and stable.

## DSL Compatibility Shelf

The flat `dsl/` shelf remains supported because many tests, examples, and
operator workflows already use it. New work may add compatibility entrypoints
there, but the preferred long-term source of truth is the package shelf under
`protocols/`.

Avoid treating `dsl/` filenames as the taxonomy. They are entrypoint names, not
the standard-library hierarchy.

## Documentation Shelf

Each family should have:

- one hub page: `docs/book/reference-<family>-surface.md`
- optional subpages for narrower command groups or failure semantics
- links from `reference-protocol-family-shelves.md`
- inclusion in the generated-style alias and surface snapshots when aliases or
  entries change

## File Size Discipline

Protocol standard-library files should stay small enough to review directly.
Keep each Rust source or test file below 600 lines. If a shelf grows close to
that limit, split by operator domain before adding more entries.

The same principle applies to docs: prefer several focused pages over one
monolithic reference dump.

## Adding A Protocol

Use this checklist:

1. Add canonical package entries under `protocols/<family>/<entry>/`.
2. Add or keep compatibility DSL entrypoints under `dsl/`.
3. Register the profile in the narrowest `src/protocol_profiles/*` shelf.
4. Add aliases only when they help humans without hiding canonical names.
5. Add cluster and semantics metadata.
6. Add family docs and link them from the protocol reference spine.
7. Run registry, docs, and validation tests before merging.
