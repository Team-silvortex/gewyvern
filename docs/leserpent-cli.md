# Leserpent Native CLI

The Rust `leserpent` binary is a replaceable frontend for `leserpentd`. It uses
the same authenticated wire-v1 protocol as other clients and never opens the
runtime SQLite database directly.

## Configuration

```bash
export LESERPENT_SOCKET=/run/user/$UID/leserpent/leserpentd.sock
export LESERPENT_IPC_TOKEN='at-least-32-non-whitespace-bytes'
export LESERPENT_PRINCIPAL='operator-a' # optional audit identity
```

The socket may instead be supplied with `--socket PATH`. The CLI refuses links,
non-socket paths, and sockets that grant group or other permissions. Tokens are
accepted only from `LESERPENT_IPC_TOKEN`, never from command-line arguments.

## Commands

```bash
leserpent health
leserpent runtime list
leserpent runtime list --environment production --cluster edge --role debugger
leserpent --json runtime list
leserpent runtime refresh runtime-a --dry-run --expected-revision 1
leserpent runtime refresh runtime-a --yes --idempotency-key deploy-2026-07-15
leserpent runtime refresh runtime-a --export-leselang
```

Human list output is tabular and replaces terminal control characters. JSON
mode emits the complete versioned protocol response envelope without reshaping
fields.

Real refresh execution requires `--yes`; preview uses `--dry-run` and cannot be
combined with confirmation. `--expected-revision` enables optimistic concurrency,
while a caller-supplied `--idempotency-key` makes automation retries stable.
`--export-leselang` is local-only, reads no token, opens no socket, and emits the
canonical equivalent function.

Exit code `0` means success, `2` means local usage/configuration/transport
failure, and `3` means the daemon returned a protocol error.
