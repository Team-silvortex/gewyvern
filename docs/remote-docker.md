# Remote Docker Execution

This how-to defines the supported path for moving container-heavy development
off a workstation and onto the trusted Linux validation host.

## Default Policy

Container shell entrypoints use `GEWY_DOCKER_EXECUTION=auto`:

- macOS dispatches to `GEWY_REMOTE_HOST`, defaulting to `kyuubiki-lab`
- Linux and CI execute against their local Docker daemon
- `GEWY_DOCKER_EXECUTION=remote` forces SSH execution
- `GEWY_DOCKER_EXECUTION=local` explicitly opts back into local Docker

The dispatcher does not use a remote Docker socket or expose a Docker TCP API.
It incrementally rsyncs the repository to
`~/.cache/gewyvern/docker-workspace`, executes there over SSH, reuses
`~/.cache/gewyvern/docker-target`, and copies validation and package artifacts
back into the local `target/` tree. The remote command phase is serialized with
a bounded `flock` wait so two container suites do not execute against one
workspace at the same time.

## Supported Entrypoints

Use the existing scripts; no extra flag is required on macOS:

```bash
bash scripts/packaging/build_packages_in_container.sh --format all
bash scripts/packaging/release_container_check.sh
bash scripts/packaging/container_validation_summary.sh
bash scripts/validation/pathological_container_validation.sh
bash scripts/validation/juice_shop_container_validation.sh
bash scripts/validation/ftp_denied_container_validation.sh
bash scripts/validation/ldap_bind_denied_container_validation.sh
```

Run an arbitrary repository command on the same remote shelf with:

```bash
scripts/remote/run_on_linux_host.sh -- cargo test --workspace
```

Manage the persistent privileged headless Linux compose environment with:

```bash
scripts/remote/headless_linux.sh build
scripts/remote/headless_linux.sh up
scripts/remote/headless_linux.sh shell
scripts/remote/headless_linux.sh down
```

## Configuration

Set `GEWY_REMOTE_HOST` to an SSH config alias or a validated host name. SSH
keys, agent state, host verification, and optional administrator elevation stay
outside the repository. Passwords are never accepted by the dispatcher.

`GEWY_REMOTE_DOCKER_WORKSPACE` may override the remote path, but it must remain
under the remote user's `~/.cache/gewyvern/` directory. This prevents a typo or
untrusted environment value from turning rsync `--delete` into a destructive
operation elsewhere on the host.

The sync excludes `.git`, local build output, dependency caches, Leserpent
runtime state, and persisted control-plane state. A small allowlist forwards
container image, mirror, timeout, and offline-build settings; arbitrary local
environment variables and secrets are not copied.

## Privilege Boundary

Membership in the remote `docker` group is sufficient for package and ordinary
container validation. Linux eBPF attach checks still require their documented
BPF privileges. The dispatcher does not silently grant `sudo`, weaken the
Docker socket, or turn skipped attach evidence into a successful result.

For a one-off local run:

```bash
GEWY_DOCKER_EXECUTION=local bash scripts/packaging/release_container_check.sh
```

For debugging the remote routing itself, run the generic dispatcher with
`--host <ssh-host>`. Generated evidence remains under `target/validation/`.
