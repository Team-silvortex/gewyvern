# Leserpent Protocol Compatibility

This policy governs the migration bridge between the Leserpent 1.x ASP.NET API
and the Rust domain/protocol kernel. The bridge translates representations; it
does not own business rules.

## Supported 1.x Slice

The first compatibility slice covers:

- `GET /v1/runtimes` with `environment`, `cluster`, and `role` filters
- `POST /v1/runtimes/{id}/refresh-status`
- the `runtime_not_found` error returned by those runtime surfaces

Canonical 1.x response fixtures live in `tests/fixtures/legacy-*.json`. The
versioned Rust envelope fixtures remain in `tests/fixtures/*-v1.json`.

## Normalization Rules

- blank filters become absent filters
- non-blank filters are trimmed and matched case-insensitively
- runtime lists are ordered by name case-insensitively, then by runtime ID
- 1.x camelCase status fields map to the domain's snake_case status projection
- refresh requests always use `compatibility_adapter` as audit origin
- refresh observations must match the requested runtime ID and revision
- duplicate mutation requests use the normal principal-scoped idempotency rules
- compatibility JSON is subject to the same 1 MiB message limit as native wire
  envelopes

Unknown 1.x presentation fields may be ignored when they do not change the
normalized runtime identity, tags, status, authorization, event, or revision.
Secrets and token-presence metadata are not copied into the domain projection.

## Change Policy

The compatibility fixture set is append-only within a released wire version.
A changed normalization rule requires a protocol contract version change and a
fixture that demonstrates both the old input and the new normalized result.
The ASP.NET route remains authoritative until a live bridge route passes these
fixtures and the existing Leserpent security tests.

## Live Bridge

`leserpent-compat-bridge` is a persistent line-delimited JSON process. The 1.x
host enables it only through an absolute `LESERPENT_RUST_BRIDGE_BIN` path. Each
request carries a correlation ID and one fixed operation; frames are capped at
1 MiB before allocation growth. The host serializes responses with its normal
source-generated JSON contract, asks Rust to validate that exact payload, and
only then returns or commits it.

The bridge is optional during source migration. When configured, failure is
closed with `502 compatibility_bridge_failed`; the host retries one transport
failure after restarting the child process. Packaging the bridge beside the
1.x host remains required before it can be enabled by default.

## Reproducible Proof

Run `gewyvern_validate leserpent-transport` from the workspace root to prove
the current wire-v1 authenticated local, HTTPS, and WebSocket transport boundaries. The command
retains separate logs for protocol fixtures, legacy adaptation, CLI/Leselang
parity, the native CLI-to-daemon Unix socket, IPC rejection paths, and a real TLS
loopback through `POST /v1/wire`, plus the native remote CLI and revisioned
`/v1/events` vertical paths. Event frames use an independently versioned schema,
omit runtime endpoints, and require the `leserpent.events.v1` subprotocol. Its
summary explicitly excludes Windows named pipes, remote GUI, and mobile clients, so passing this
shelf does not imply full Gate 6 completion.
