# Tutorial: Your First Leselang GUI Automation

This tutorial proves one core Leserpent rule: a native control, a CLI operation,
and canonical Leselang are views of the same typed intent. It starts with local
exports that cannot execute or contact a daemon.

Leselang is protocolized GUI and control automation, not a general-purpose
language and not a second privileged control plane.

## What You Will Do

By the end, you will have:

1. exported one read query as canonical Leselang
2. inspected the equivalent typed plan
3. exported a mutation without executing it
4. found the same canonical preview in Desktop
5. understood when presentation automation can run live

## Prerequisites

- the repository root as your working directory
- a working Rust toolchain
- no daemon, token, account, or network connection

The examples use `cargo run -p leserpent-cli --`. An installed `leserpent`
binary accepts the same arguments without that prefix.

## Step 1: Export A Read Query

Run:

```bash
cargo run -p leserpent-cli --quiet -- \
  runtime list --environment production --export-leselang
```

The canonical output is:

```leselang
fn main() = runtime.list(
  environment: "production",
  cluster: none,
  role: none,
)
```

`--export-leselang` is local-only. It opens no socket, reads no token, and
executes no query.

**Checkpoint:** the output contains explicit `none` values rather than hidden
frontend defaults.

## Step 2: Inspect The Equivalent Typed Plan

Export the same query as a plan:

```bash
cargo run -p leserpent-cli --quiet -- \
  runtime list --environment production --export-plan \
  | jq .
```

Read these fields:

- `schema_version`
- `required_capability`
- `operation.kind`
- `operation.payload.query.kind`
- `operation.payload.query.filter`

The plan and the source share the same lowering function. Neither form can add
an endpoint, bearer token, or frontend object reference.

**Checkpoint:** `required_capability` is `runtime.read` and the environment
filter is `production` in both representations.

## Step 3: Export A Mutation Without Applying It

```bash
cargo run -p leserpent-cli --quiet -- \
  runtime refresh runtime-a --export-leselang

cargo run -p leserpent-cli --quiet -- \
  runtime deploy runtime-a \
  --pipeline-kind http/request \
  --target pid:42 \
  --export-leselang
```

These commands still do not connect or mutate. The deployment source contains
the typed runtime, pipeline kind, and target, while principal, capabilities,
revision, and idempotency remain host-owned execution context.

To export a deterministic executable plan instead, a mutation must state both
its confirmation posture and idempotency identity:

```bash
cargo run -p leserpent-cli --quiet -- \
  runtime refresh runtime-a \
  --dry-run \
  --expected-revision 7 \
  --idempotency-key tutorial-refresh-a \
  --export-plan \
  | jq .
```

`--dry-run` and `--yes` are mutually exclusive. Exporting a plan is not
permission to apply it.

**Checkpoint:** you can distinguish canonical source, a typed plan, dry-run,
and confirmed execution.

## Step 4: Find The Same Path In Desktop

Complete [Your first Leserpent Desktop session](tutorial-leserpent-desktop.md),
open a live runtime workspace, then choose `Workspace Leselang`.

- Inspect, history, logs, refresh, capability refresh, and deployment use the
  same Rust-owned canonical formatter as the CLI.
- A parameterized form preview is cancellable and appears only while every
  field is valid.
- `Copy Leselang` copies reviewed source and never substitutes a C# template.
- A failed export disables copying; the frontend does not invent fallback
  source.

Choose one read-only workspace action and compare its visible identity with the
CLI export. Runtime and node IDs are opaque values; do not parse command meaning
out of their spelling.

**Checkpoint:** the Desktop preview and CLI source express the same operation
without embedding endpoint or credential data.

## Step 5: Understand Presentation Operations

Leselang can also describe frontend-local operations such as:

```leselang
fn main() = ui.focus(node_id: "runtime-runtime-a-refresh")
```

```leselang
fn main() = ui.assert_action_available(
  node_id: "runtime-runtime-a-refresh",
)
```

Presentation operations require `ui.presentation`. They resolve stable semantic
node IDs through the current validated `UiDocument`; they cannot become domain
queries or commands. Missing, stale, hidden, unrealized, disabled, or mismatched
controls fail explicitly.

Wait operations use protocol-fixed bounded deadlines while source remains
synchronous. Leselang deliberately exposes no `async`/`await`, GUI object,
thread, event loop, reflection, raw HTTP, or shell escape.

## Step 6: Run Live Only Through A Bound Debugger Session

The Desktop debugger workspace exposes `Run live` for a daemon-owned suspended
Leselang VM. The authority sends one typed presentation effect at a time; the
Avalonia adapter applies it to the current native daemon window and returns a
bound result for deterministic re-entry.

The live loop:

- is capped at 64 effects
- binds effect, revision, principal, capability, and node identity
- keeps source, locals, continuation images, endpoints, and bearer credentials
  out of acknowledgements
- turns adapter rejection into visible terminal failure

Do not treat `Run live` as an arbitrary source console. It is a bounded
authority-owned debugger path over the same presentation protocol.

## Completion Checkpoint

You have completed the tutorial when you can trace:

```text
native control <-> typed UI action <-> Leselang
```

and explain why canonical export does not execute, why execution needs host
context and capabilities, and why presentation effects cannot bypass domain
authority.

## Verify The Equivalence Contracts

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-leselang-gui-export
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-remote-ui-action-routing
cargo test -p leselang-ui
```

## Where To Go Next

- [Leselang language contract](../leselang-language.md)
- [Leselang UI IR contract](../leselang-ui.md)
- [GUI function-chain closure](../leserpent-gui-function-chains.md)
- [Leselang documentation module](../modules/leselang.md)
