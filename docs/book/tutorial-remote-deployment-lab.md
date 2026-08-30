# Tutorial: A Disposable Remote Deployment Lab

This advanced tutorial exercises the intended deployment topology:

```text
Leserpent Desktop or CLI
  -> authenticated leserpentd authority
  -> remote leserpentd bootstrap
  -> remote Gewyvern provisioning
```

It performs real installation and retirement. Use only disposable Linux targets
that you are authorized to modify.

## What You Will Do

By the end, you will have:

1. verified one existing deployment authority
2. bootstrapped and bound one remote `leserpentd`
3. provisioned and registered one remote Gewyvern runtime
4. observed the runtime through its owning daemon
5. retired Gewyvern before retiring the daemon

## Prerequisites

These prerequisites are hard stop conditions. Do not begin mutation until all
of them are true:

- the authority is already reachable through authenticated IPC or HTTPS
- the authority registers `host.bootstrap`, `host.retire`,
  `gewyvern.runtime.provision`, and `gewyvern.runtime.retire`
- its private bootstrap and Gewyvern provisioning configurations pass strict
  mode/ownership validation
- each target host and SHA-256 host key is allow-listed in those configurations
- the configured platform secret service already contains the required SSH
  password under an opaque key such as `vault:ssh:lab-daemon`
- target-side policy permits the selected `user` or non-interactive `system`
  install profile
- you have unique bootstrap, provisioning, retirement, daemon, and runtime IDs

There is intentionally no raw password, private-key, or sudo-password CLI
argument. Never place one in a command, JSON fixture, environment variable,
shell history, screenshot, or repository. If the opaque `vault:ssh:*` handle is
not prepared, stop here and ask the authority administrator to provision it
through the reviewed platform-secret workflow.

## Step 1: Set Non-Secret Lab Identity

Choose stable IDs once and retain them through observation and cleanup:

```bash
export AUTHORITY=https://authority.example:7443
export AUTHORITY_CA=/absolute/path/to/authority-ca.pem
export TARGET_HOST=lab-target.example
export BOOTSTRAP_ID=tutorial-bootstrap-001
export PROVISIONING_ID=tutorial-provision-001
export RUNTIME_ID=tutorial-runtime-001
export RUNTIME_RETIREMENT_ID=tutorial-runtime-retire-001
export DAEMON_RETIREMENT_ID=tutorial-daemon-retire-001
export SSH_HANDLE=vault:ssh:lab-daemon
```

`LESERPENT_REMOTE_TOKEN` supplies the endpoint-scoped bearer token to the CLI.
Set it through your protected process environment or use the Desktop platform
vault; do not paste it into the command line.

Validate the authority before mutation:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  health
```

Stop unless health is ready and the CA belongs to that exact origin.

## Step 2: Bootstrap The Remote Daemon

In Desktop, choose `Deploy daemon`, select the verified authority, enter the
target, port, bootstrap ID, and opaque SSH handle, then review the explicit
confirmation. The CLI-equivalent mutation is:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  bootstrap deploy "$BOOTSTRAP_ID" \
  --host "$TARGET_HOST" \
  --port 22 \
  --credential-handle "$SSH_HANDLE" \
  --yes
```

The first success may be non-terminal. Observe the same ID instead of creating
a second deployment:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  bootstrap inspect "$BOOTSTRAP_ID"
```

Stop on `failed`. Remediate the bounded fault and choose a new bootstrap ID;
do not reinterpret failure as an invitation to mutate the checkpoint manually.

## Step 3: Bind And Promote The Session

When inspection reaches `bootstrapped`, bind that exact handoff:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  bootstrap bind "$BOOTSTRAP_ID" \
  --yes
```

The CLI completes the server-side session binding. In Desktop, `Verify & bind
session` performs the same transition; `Add to Hub` additionally verifies the
new endpoint's TLS/token health and promotes its opaque session/trust handles to
a saved authority. Only Local Orchestra can export its app-owned trust and
session stores for automatic promotion.

**Checkpoint:** the bootstrap state is `session_bound`, and the target daemon
appears as its own authority rather than as a runtime child.

## Step 4: Provision Gewyvern Through Its Owning Authority

Choose `Provision gewyvern` in the Hub, or use the selected authority's strict
provisioning route:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  runtime provision "$RUNTIME_ID" \
  --provisioning-id "$PROVISIONING_ID" \
  --host "$TARGET_HOST" \
  --port 22 \
  --credential-handle "$SSH_HANDLE" \
  --yes \
  --wait --count 30 --interval-ms 2000
```

Provisioning is separate from debugging-pipeline `runtime deploy`. It installs
the service, proves readiness, persists endpoint-bound trust, and registers the
runtime under one daemon authority. Polling replays the same request and stable
operation ID.

Stop on terminal `failed`. Correct the target, authority policy, or artifact,
then use a new provisioning ID. Never reuse an identity for a semantically
different installation attempt.

## Step 5: Observe Before Debugging

Refresh Hub topology and open the runtime beneath the daemon that owns it. From
the CLI:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  runtime inspect "$RUNTIME_ID"

cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  runtime logs "$RUNTIME_ID"
```

Confirm identity, current revision, registration state, observed capabilities,
and bounded logs before deploying a debug pipeline. Cached or heartbeat-only
state cannot authorize a mutation.

**Checkpoint:** the runtime is registered, inspectable, and visible only below
its owning daemon.

## Step 6: Retire In Dependency Order

Retire the Gewyvern runtime first:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  runtime retire "$RUNTIME_ID" \
  --retirement-id "$RUNTIME_RETIREMENT_ID" \
  --provisioning-id "$PROVISIONING_ID" \
  --host "$TARGET_HOST" \
  --port 22 \
  --credential-handle "$SSH_HANDLE" \
  --yes \
  --wait --count 30 --interval-ms 2000
```

A successful terminal state proves service retirement before atomic runtime
unregistration. A failed state deliberately leaves the runtime registered.

Only after the runtime is unregistered, retire the bootstrapped daemon:

```bash
cargo run -p leserpent-cli --quiet -- \
  --remote "$AUTHORITY" \
  --remote-ca "$AUTHORITY_CA" \
  bootstrap retire "$BOOTSTRAP_ID" \
  --retirement-id "$DAEMON_RETIREMENT_ID" \
  --credential-handle "$SSH_HANDLE" \
  --yes \
  --wait --count 30 --interval-ms 2000
```

Daemon retirement derives host, daemon ID, generation, and install profile from
the bound checkpoint. Do not supply or override them. Successful retirement
does not silently delete a saved Desktop profile or credential; remove those
only through the separate connection-management action after verifying the
service is gone.

## Completion Checkpoint

The lab is complete only when:

- Gewyvern reached `runtime_unregistered`
- the daemon reached `service_retired`
- Hub refresh no longer presents the retired runtime as live
- any saved authority/profile cleanup was explicit
- no raw credential appeared in terminal output or retained evidence

## Failure And Interruption Rules

- A timeout means “not observed terminal”, not “operation did not happen”.
- After a lost response, inspect the same stable ID before retrying.
- Closing a Desktop lifecycle window cancels observation and drops late UI
  completion; it does not roll back an authority mutation already accepted.
- Never create a new ID merely to hide an unknown outcome. Resolve the old
  checkpoint first.
- Never delete runtime registration before retirement proves the service is
  stopped.

## Where To Go Next

- [Leserpent CLI reference](../leserpent-cli.md)
- [Protocol compatibility and deployment boundaries](../../crates/leserpent-protocol/COMPATIBILITY.md)
- [Desktop session tutorial](tutorial-leserpent-desktop.md)
- [Deployment and recovery evidence](../leserpent-2-roadmap.md)
