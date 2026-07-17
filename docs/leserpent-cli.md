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

For authenticated HTTPS, select the remote endpoint instead of the socket:

```bash
export LESERPENT_REMOTE='https://control.example.internal:9443'
export LESERPENT_REMOTE_CA='/etc/leserpent/ca.pem'
export LESERPENT_REMOTE_TOKEN='at-least-32-non-whitespace-bytes'
leserpent --json health
```

The endpoint and CA may instead be supplied as `--remote HTTPS_URL --remote-ca
PATH`. Local and remote transports are mutually exclusive. Remote URLs accept
only `https://HOST[:PORT]` with no path, query, credentials, or redirect. The CA
must be a regular non-symlink PEM file no larger than 1 MiB; hostname/IP
verification is mandatory. The token is accepted only from
`LESERPENT_REMOTE_TOKEN`.

Explicit CA trust is the stable remote CLI policy rather than a fallback for a
missing system-trust adapter. Remote and local transport tokens are retained in
zeroizing memory, and temporary Authorization headers or authenticated IPC
request buffers are cleared on both success and failure paths. Windows can use
the same HTTPS CLI contract; a future named-pipe transport is optional and does
not change command semantics.

## Commands

```bash
leserpent health
leserpent runtime list
leserpent runtime list --environment production --cluster edge --role debugger
leserpent --json runtime list
leserpent runtime inspect runtime-a
leserpent --json runtime inspect runtime-a
leserpent runtime history runtime-a
leserpent --json runtime history runtime-a
leserpent runtime logs runtime-a
leserpent --json runtime logs runtime-a
leserpent runtime watch runtime-a
leserpent --json runtime watch runtime-a --count 100 --interval-ms 500
leserpent runtime list --environment production --export-leselang
leserpent runtime list --role debugger --export-plan
leserpent runtime inspect runtime-a --export-leselang
leserpent runtime inspect runtime-a --export-plan
leserpent runtime history runtime-a --export-leselang
leserpent runtime history runtime-a --export-plan
leserpent runtime logs runtime-a --export-leselang
leserpent runtime logs runtime-a --export-plan
leserpent runtime refresh runtime-a --dry-run --expected-revision 1
leserpent runtime refresh runtime-a --yes --idempotency-key deploy-2026-07-15
leserpent runtime refresh runtime-a --export-leselang
leserpent runtime refresh runtime-a --dry-run --expected-revision 7 \
  --idempotency-key plan-a --export-plan
leserpent runtime refresh-capabilities runtime-a --yes
leserpent runtime refresh-capabilities runtime-a --export-leselang
leserpent runtime refresh-capabilities runtime-a --dry-run \
  --idempotency-key capabilities-plan-a --export-plan
leserpent runtime deploy runtime-a --pipeline-kind http/request \
  --target pid:42 --yes --idempotency-key deploy-a
leserpent runtime deploy runtime-a --pipeline-kind http/request \
  --target pid:42 --export-leselang
```

Human list output is tabular and replaces terminal control characters. JSON
mode emits the complete versioned protocol response envelope without reshaping
fields.

`runtime inspect` performs a dedicated `runtime.read` query and returns one
projection. Missing identifiers fail through the daemon's typed
`RuntimeNotFound` path; the CLI never downloads the fleet and filters it locally.
Human output includes a bounded capability summary. It reports `unobserved`
until discovery completes; observed output contains the service/version, typed
boolean claims, canonical endpoint paths, and sorted boolean extensions. It
never renders adapter request JSON, target origins, secret aliases, or tokens.
The stable `capabilities_observed_for_revision` field is `none` before discovery,
the originating command revision after a current observation, and
`legacy-unknown` only for a compatible projection written before revision
binding existed. `runtime inspect` and every `runtime watch` sample use the same
renderer.

`runtime history` returns at most 32 applied results for one runtime, newest
revision first. Human output is a terminal-safe table; JSON preserves the typed
wire response. The command reads domain history and never opens the SQLite
journal directly.

`runtime logs` returns the newest bounded window of at most 256 typed records
for one runtime. Execution, plan export, and canonical Leselang export all use
the shared `runtime.logs` lowering with no cursor, so CLI and language semantics
cannot drift.

`runtime watch` is a bounded CLI transport loop over the same normalized
`runtime.inspect` query. It emits the first projection and then only changed
revisions, flushing each human line or JSON envelope immediately. The default is
20 polls at one-second intervals; `--count` is limited to 1-1000 and
`--interval-ms` to 50-60000. This keeps daemon requests short-lived and avoids
introducing a second domain or Leselang watch semantic.

Every executable command has identical local IPC and remote HTTPS lowering,
confirmation, rendering, and exit-code semantics. The HTTPS client requires
unique JSON `Content-Length`/`Content-Type` response framing, rejects transfer
encoding and redirects, and retains the wire-v1 1 MiB response limit.

All four read queries support local `--export-leselang` and `--export-plan`. These
paths require neither socket nor token. List exports normalize filters before
rendering. Every source export passes through the shared canonical Leselang
formatter; plan exports use the same shared lowering functions as real IPC
execution, so exported and executed query envelopes cannot drift.

Real refresh execution requires `--yes`; preview uses `--dry-run` and cannot be
combined with confirmation. `--expected-revision` enables optimistic concurrency,
while a caller-supplied `--idempotency-key` makes automation retries stable.
`--export-leselang` is local-only, reads no token, opens no socket, and emits the
canonical equivalent function.

For refresh, `--export-plan` is also local-only and emits the validated, versioned
`CommandPlan` JSON used by the execution path. It requires an explicit
`--idempotency-key` so repeated exports are byte-for-byte deterministic, plus
either `--dry-run` or `--yes` so confirmation intent is never implicit. CLI
execution, plan export, and Leselang lowering share the same runtime-refresh
normalization function.

`runtime refresh-capabilities` follows the same confirmation, optimistic
revision, idempotency, Leselang export, and deterministic plan-export rules. It
submits the shared domain command; only the durable runtime may materialize the
underlying discovery effect.

The authenticated HTTPS vertical proves this command end to end through the
daemon scheduler and real Gewyvern discovery adapter, then reads the observed
projection back with `runtime inspect`. It also verifies that the configured
adapter origin and authorization material are not rendered.

`runtime deploy` promotes the bounded deployment adapter to an operator command.
It requires the independent `runtime.deploy` capability, a valid pipeline kind,
optional bounded target, optimistic revision, caller idempotency, and explicit
`--yes` for execution. The principal, request identity, and confirmation in the
adapter payload come from the shared command envelope; CLI arguments cannot
override them. Dry-run, canonical Leselang export, and deterministic plan export
use the same lowering function.

Exit code `0` means success, `2` means local usage/configuration/transport
failure, and `3` means the daemon returned a protocol error.
