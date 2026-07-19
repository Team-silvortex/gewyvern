# Native Packaging

`gewyvern` can be staged and packaged as native Linux artifacts for operator
install flows that do not want to build directly from source.

The packaging path is intentionally narrow:

- it packages the standalone runtime and compiler tools
- it ships the built-in DSL and protocol registry assets
- it does not yet try to provision system users, systemd units, or fleet-level
  orchestration glue

The packaging scripts now live together under:

- [`scripts/packaging/`](scripts/packaging)

That shelf intentionally groups:

- build entrypoints
- install smoke
- packaged validation
- release wrappers

## Role In The Shelf

Treat this page as the native artifact and packaged-validation shelf.

Use it when the question is:

- how do I build `.deb` / `.rpm` artifacts?
- how do I validate installed packaged behavior in clean Linux containers?
- how do I narrow a packaging or packaged-runtime failure?

Do not use this page as:

- the shortest release gate decision page
- the global operator script router
- the contributor workflow guide

For those, use:

- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/development.md](docs/development.md)

## Companion Shelves

- [docs/script-entrypoints.md](docs/script-entrypoints.md)
  for the shortest goal-based script routing
- [docs/cli-recipes.md](docs/cli-recipes.md)
  for the broader command shelf outside packaging-specific flows
- [docs/release-checklist.md](docs/release-checklist.md)
  for the shorter ship/no-ship gate

## What Gets Installed

The native package layout is:

- `/usr/bin/gewyvern`
- `/usr/bin/gewyvern_socket_send`
- `/usr/bin/gewyc`
- `/usr/share/gewyvern/dsl`
- `/usr/share/gewyvern/protocols`
- `/usr/share/gewyvern/package-compat.toml`
- `/usr/share/gewyvern/examples/gewyvern.toml.example`
- `/usr/share/doc/gewyvern/README.md`
- `/usr/share/doc/gewyvern/LICENSE`
- `/usr/share/doc/gewyvern/docs`

That is enough for:

- standalone CLI use
- compiler and diagnostics use through `gewyc`
- local socket validation helpers
- protocol registry and DSL-driven built-in paths
- a machine-readable package compatibility marker
- a packaged example config that operators can copy into the standard config
  root

For the broader runtime layout policy beyond the packaged Linux tree,
also see:

- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)

That page explains:

- standard mutable roots on Linux, macOS, and Windows
- the role of `/usr/share/gewyvern` as the packaged read-only share root
- how `~/.gewyvern/` style older local layouts should be treated during
  upgrades

## Compatibility Manifest

Every native package now installs:

- `/usr/share/gewyvern/package-compat.toml`

Treat that file as the read-only compatibility contract for the installed
artifact. It records:

- `schema_version`
- `package_name`
- `package_version`
- `package_release`
- `release_line`
- `layout_version`
- `config_schema_version`
- `share_root`
- `protocol_registry_root`
- `dsl_root`
- `config_example`
- `legacy_compat_root`
- `upgrade_policy`

The default current values are intentionally conservative:

- `release_line = "v1.5.0"`
- `layout_version = 1`
- `config_schema_version = 1`
- `upgrade_policy = "copy-forward-without-overwrite"`

Packaged container validation never guesses an artifact from filename order.
Both local and remote checks read the unique `deb` or `rpm` entry from
`target/packages/build-manifest.txt`, reject malformed or duplicate keys, and
resolve its portable package-root-relative paths only after requiring the
referenced regular non-symlink file to remain inside the package root with the
expected extension. Remote artifact collection, package smoke,
and runtime smoke share this resolver; package-cache reuse applies the same
unique-key and non-symlink policy instead of trusting the first matching line.

Package builders can override the minor line and schema markers with:

- `GEWY_RELEASE_LINE`
- `GEWY_LAYOUT_VERSION`
- `GEWY_CONFIG_SCHEMA_VERSION`

Container install validators can also override package-manager mirrors:

- `GEWY_DEB_APT_MIRROR`
- `GEWY_RPM_DNF_MIRROR`

Validators install local artifacts before consulting a repository: DEB uses
`dpkg -i` with apt only as a missing-dependency fallback, while RPM uses
`rpm -Uvh` with the equivalent dnf fallback. Runtime HTTP readiness uses the
base image's Bash TCP support rather than downloading `curl`, so packages with
no external dependencies can be validated without repository access.

The install smoke checks this manifest for both DEB and RPM packages so the two
native package paths cannot silently drift.

## Container Runner Reliability

The packaged container validators use one shared Docker runner from:

- `scripts/packaging/container_validation_common.sh`

That runner gives each validation container a deterministic gewyvern-prefixed
name, applies a per-container timeout, and best-effort removes the container if
the command times out. Override the timeout with:

- `GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS`

The default is intentionally conservative at 900 seconds so a slow package
mirror has room to recover without leaving a hidden `docker run` behind.

RPM validation also installs the local package with local `rpm -Uvh` first and
only falls back to `dnf install` when the container needs dependency
resolution. This keeps Fedora repository metadata slowness from turning a local
package smoke into a long network-bound release gate.

## Build Entry Point

If you already know the outcome you want and only need the shortest route to
the right packaging script, start with
[docs/script-entrypoints.md](docs/script-entrypoints.md).

Use:

```bash
bash scripts/packaging/build_packages.sh --layout-only
```

That command:

- builds release binaries
- stages the Linux install tree
- renders DEB control metadata
- renders an RPM spec

Linux packages also install the inactive privileged eBPF helper at
`/usr/libexec/gewyvern-ebpf-helper`, the root-only management entry point at
`/usr/sbin/gewyvern-ebpf-provision`, and configuration examples under
`/usr/share/gewyvern/examples`. Packaging never enables the rule or chooses an
account automatically. Preview and provision a dedicated account with:

```bash
sudo /usr/sbin/gewyvern-ebpf-provision --user VALIDATION_USER --dry-run
sudo /usr/sbin/gewyvern-ebpf-provision --user VALIDATION_USER
```

The provisioner resolves the account through `getent`, rejects UID 0, verifies
that the installed helper and destination directories are root-owned and not
writable by group or world, validates generated policy with `visudo -cf`, and
atomically installs `/etc/gewyvern/ebpf-helper.conf` plus the mode-`0440`
`sudoers` rule. The provisioner itself is not included in that rule. The
validation account must not receive unrestricted passwordless `sudo`; the
helper's strict argument parser remains the privilege boundary.

When `--format all` is used for a real package build, the script now emits the
`deb` and `rpm` packages in parallel against the same staged layout so Linux
release-style runs spend less wall-clock time waiting on serial package
assembly.
It also normalizes staged file mtimes with `SOURCE_DATE_EPOCH` or, by default,
the latest Git commit timestamp so repeated unchanged builds do not drift only
because packaging ran at a different wall-clock second.
When the release binaries and packaging inputs are unchanged, the script also
reuses the existing native package artifacts instead of restaging and
reassembling them again.

Package generation is serialized per output directory with a bounded lock.
DEB/RPM payloads, the build manifest, and the cache key are published through
same-directory atomic replacement, so readers observe either the previous
complete package set or the new complete set. `--layout-only` bypasses package
cache reuse and never removes or replaces the published package manifest.

It does not require `dpkg-deb` or `rpmbuild`.

When `--layout-only` is used, the staged tree is kept under:

- `target/packages/layout.*`

## Building A DEB

If the host has `dpkg-deb`:

```bash
bash scripts/packaging/build_packages.sh --format deb
```

Artifacts are written under:

- `target/packages`

## Building An RPM

If the host has `rpmbuild`:

```bash
bash scripts/packaging/build_packages.sh --format rpm
```

RPM artifacts are written under:

- `target/packages/rpm`

## Building Inside The Bundled Linux Container

If the host itself does not provide `dpkg-deb` or `rpmbuild`, use:

```bash
bash scripts/packaging/build_packages_in_container.sh --format all
```

That path uses:

- `docker/linux-dev/Dockerfile`
- `scripts/packaging/build_packages.sh`

and writes artifacts back into:

- `target/packages`

## Container Install Smoke

After building native artifacts, verify that they install cleanly in fresh
Linux containers:

```bash
cargo run --quiet --bin gewyvern_validate -- package-install-smoke
```

Successful runs retain `deb.json`, `rpm.json`, `summary.json`, and
`evidence-index.json` under `target/validation/package-install-smoke/`. The
per-family records bind the exact package filename and SHA-256 to the clean
container image that installed it; stale success records are removed before a
new run starts.

The runtime, protocol, and operator-path validators use the same four-file
contract under their own `target/validation/<command>/` directories. Composite
`container-validation-summary` and `release-container-check` directories each
retain their own `summary.json` and `evidence-index.json`, with portable
relative references to every child evidence shelf. A reported evidence path is
therefore always an actual command-owned directory rather than the shared
validation root.

That smoke path:

- installs the latest local `.deb` in a clean Ubuntu container
- installs the latest local `.rpm` in a clean Fedora container
- runs:
  - `gewyvern --list-protocols`
  - `gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json`
- checks that packaged DSL and protocol assets exist under
  `/usr/share/gewyvern`
- checks that `/usr/share/gewyvern/package-compat.toml` exists and matches the
  expected release line
- checks that the packaged example config and license are installed
- checks that `gewyvern`, `gewyc`, and `gewyvern_socket_send` are present on
  `PATH`

If you only want one package family, use:

```bash
cargo run --quiet --bin gewyvern_validate -- package-install-smoke --deb
cargo run --quiet --bin gewyvern_validate -- package-install-smoke --rpm
```

If a wrapper or CI job needs one final machine-readable result instead of text
summaries, prefix the command with the global JSON flag:

```bash
cargo run --quiet --bin gewyvern_validate -- --json package-install-smoke
```

## Container Runtime Validation

To go beyond install smoke and validate the packaged standalone runtime itself,
use:

```bash
cargo run --quiet --bin gewyvern_validate -- container-runtime-validation
```

That path installs the latest package into a clean Linux container and then:

- starts packaged `gewyvern` in `--serve` mode
- feeds repeated TCP and UDP sessions with packaged `gewyvern_socket_send`
- injects a malformed line and confirms the service stays alive
- checks `/health`, `/v1/latest/summary.json`, `/v1/latest/analysis.json`, and
  `/v1/latest/export.json`

If you only want one package family, use:

```bash
cargo run --quiet --bin gewyvern_validate -- container-runtime-validation --deb
cargo run --quiet --bin gewyvern_validate -- container-runtime-validation --rpm
```

## Container Protocol Validation

To validate packaged protocol support on a clean Linux install, use:

```bash
bash scripts/packaging/container_protocol_validation.sh
```

That path installs the latest package and then verifies:

- packaged `--list-protocols`
- grouped high-frequency packaged protocol summaries for:
  - resolution, web, and secure transport:
    `DNS`, `HTTP`, `TLS`, `HTTP/3`, `QUIC`
  - remote access and proxy:
    `SSH`, `SOCKS5`
  - database, messaging, and directory:
    `MySQL`, `PostgreSQL`, `SMTP`, `LDAP`
  - cache, broker, auth, management, and signaling:
    `Redis`, `MQTT`, `AMQP`, `RADIUS`, `SNMP`, `FTP`, `IMAP`, `POP3`,
    `Kerberos`, `RTSP`
- packaged `--scan-all --json --summary-only`

If you only want one package family, use:

```bash
bash scripts/packaging/container_protocol_validation.sh --deb
bash scripts/packaging/container_protocol_validation.sh --rpm
```

## Container Validation Summary

To run the packaged Linux container validation suite through one summary
entrypoint, use:

```bash
cargo run --quiet --bin gewyvern_validate -- container-validation-summary
```

That wrapper runs, in order:

- [container_protocol_validation.sh](scripts/packaging/container_protocol_validation.sh)
- [container_operator_path_validation.sh](scripts/packaging/container_operator_path_validation.sh)

Naming note for the packaging scripts:

- `smoke` means install/bring-up confidence
- `validation` means one grouped packaged-behavior check
- `summary` means a wrapper over several packaged validations

If you only want one package family, use:

```bash
cargo run --quiet --bin gewyvern_validate -- container-validation-summary --deb
cargo run --quiet --bin gewyvern_validate -- container-validation-summary --rpm
```

## Release Container Check

To run the current release-oriented packaged Linux validation suite through one
entrypoint, use:

```bash
cargo run --quiet --bin gewyvern_validate -- release-container-check
```

That wrapper runs, in order:

- [package_install_smoke.sh](scripts/packaging/package_install_smoke.sh)
- [container_runtime_validation.sh](scripts/packaging/container_runtime_validation.sh)
- [container_validation_summary.sh](scripts/packaging/container_validation_summary.sh)

If you only want one package family, use:

```bash
cargo run --quiet --bin gewyvern_validate -- release-container-check --deb
cargo run --quiet --bin gewyvern_validate -- release-container-check --rpm
```

For the shorter release-minded decision shelf that also includes the
three-module integration gate, see:

- [docs/release-checklist.md](docs/release-checklist.md)

If you want one orchestration command that rebuilds artifacts, runs the
packaged release check, and then runs the three-module stack smoke, use:

```bash
cargo run --quiet --bin gewyvern_validate -- release-gate
```

That ship gate belongs conceptually to
[docs/release-checklist.md](docs/release-checklist.md).
This page stays focused on the packaging and packaged-runtime mechanics behind
that gate.

## Container Operator-Path Validation

To validate more realistic packaged operator-path chains after install, use:

```bash
bash scripts/packaging/container_operator_path_validation.sh
```

That path installs the latest package and then verifies these chained protocol
families in clean Linux environments:

- Advisory resolution and application path:
  `DNS -> QUIC -> HTTP/3`
- Secure transport and tunnel paths:
  `DNS -> TLS -> HTTPS CONNECT`,
  `DNS -> SOCKS5 -> HTTPS CONNECT`
- Secure database and mail paths:
  `DNS -> TLS -> Postgres`,
  `DNS -> TLS -> MySQL`,
  `DNS -> TLS -> SMTP auth`,
  `DNS -> SMTP`
- Conservative negative-path guard:
  `SOCKS5 auth denied`

The check keeps the same machine-facing expectations as the standalone runtime,
grouped into the same buckets:

- Advisory resolution/application:
  DNS remains a conservative advisory path,
  HTTP/3 remains a healthy-but-advisory application-layer path,
  and QUIC remains a `missing_transition` path that recommends collecting more
  runtime evidence.
- Secure transport/tunnel:
  TLS and HTTPS CONNECT remain healthy-but-advisory secure transport paths,
  while SOCKS5 auth remains a `missing_transition` proxy-auth path that
  recommends collecting more runtime evidence before strong automation.
- Secure database/mail:
  PostgreSQL query, MySQL session, SMTP auth, and SMTP session remain
  `missing_transition` paths that recommend collecting more runtime evidence
  before strong automation.
- Negative-path guard:
  the current packaged `SOCKS5 auth denied` demo still stays in a conservative
  posture instead of over-collapsing to a stronger denial claim.

If you only want one package family, use:

```bash
bash scripts/packaging/container_operator_path_validation.sh --deb
bash scripts/packaging/container_operator_path_validation.sh --rpm
```

## Notes

- packaging is Linux-oriented even if the staging script is run from another
  development host
- `--layout-only` is the safest local verification path when package manager
  tools are not installed
- the container path requires a working local Docker daemon
- current packages are intended for standalone host installs, not multi-host
  orchestration
