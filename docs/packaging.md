# Native Packaging

`gewyvern` can be staged and packaged as native Linux artifacts for operator
install flows that do not want to build directly from source.

The packaging path is intentionally narrow:

- it packages the standalone runtime and compiler tools
- it ships the built-in DSL and protocol registry assets
- it does not yet try to provision system users, systemd units, or fleet-level
  orchestration glue

The packaging scripts now live together under:

- [`scripts/packaging/`](/Users/Shared/chroot/dev/gewyvern/scripts/packaging)

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

- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

## Companion Shelves

- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
  for the shortest goal-based script routing
- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
  for the broader command shelf outside packaging-specific flows
- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)
  for the shorter ship/no-ship gate

## What Gets Installed

The native package layout is:

- `/usr/bin/gewyvern`
- `/usr/bin/gewyvern_socket_send`
- `/usr/bin/gewyc`
- `/usr/share/gewyvern/dsl`
- `/usr/share/gewyvern/protocols`
- `/usr/share/doc/gewyvern/README.md`
- `/usr/share/doc/gewyvern/docs`

That is enough for:

- standalone CLI use
- compiler and diagnostics use through `gewyc`
- local socket validation helpers
- protocol registry and DSL-driven built-in paths

For the broader `0.15.x` runtime layout policy beyond the packaged Linux tree,
also see:

- [docs/book/reference-runtime-layout.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-layout.md)

That page explains:

- standard mutable roots on Linux, macOS, and Windows
- the role of `/usr/share/gewyvern` as the packaged read-only share root
- how `~/.gewyvern/` style older local layouts should be treated during
  upgrades

## Build Entry Point

If you already know the outcome you want and only need the shortest route to
the right packaging script, start with
[docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md).

Use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages.sh --layout-only
```

That command:

- builds release binaries
- stages the Linux install tree
- renders DEB control metadata
- renders an RPM spec

It does not require `dpkg-deb` or `rpmbuild`.

When `--layout-only` is used, the staged tree is kept under:

- `/Users/Shared/chroot/dev/gewyvern/target/packages/layout.*`

## Building A DEB

If the host has `dpkg-deb`:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages.sh --format deb
```

Artifacts are written under:

- `/Users/Shared/chroot/dev/gewyvern/target/packages`

## Building An RPM

If the host has `rpmbuild`:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages.sh --format rpm
```

RPM artifacts are written under:

- `/Users/Shared/chroot/dev/gewyvern/target/packages/rpm`

## Building Inside The Bundled Linux Container

If the host itself does not provide `dpkg-deb` or `rpmbuild`, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages_in_container.sh --format all
```

That path uses:

- `/Users/Shared/chroot/dev/gewyvern/docker/linux-dev/Dockerfile`
- `/Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages.sh`

and writes artifacts back into:

- `/Users/Shared/chroot/dev/gewyvern/target/packages`

## Container Install Smoke

After building native artifacts, verify that they install cleanly in fresh
Linux containers:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh
```

That smoke path:

- installs the latest local `.deb` in a clean Ubuntu container
- installs the latest local `.rpm` in a clean Fedora container
- runs:
  - `gewyvern --list-protocols`
  - `gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json`
- checks that packaged DSL and protocol assets exist under
  `/usr/share/gewyvern`
- checks that `gewyvern`, `gewyc`, and `gewyvern_socket_send` are present on
  `PATH`

If you only want one package family, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh --rpm
```

## Container Runtime Validation

To go beyond install smoke and validate the packaged standalone runtime itself,
use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh
```

That path installs the latest package into a clean Linux container and then:

- starts packaged `gewyvern` in `--serve` mode
- feeds repeated TCP and UDP sessions with packaged `gewyvern_socket_send`
- injects a malformed line and confirms the service stays alive
- checks `/health`, `/v1/latest/summary.json`, `/v1/latest/analysis.json`, and
  `/v1/latest/export.json`

If you only want one package family, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh --rpm
```

## Container Protocol Validation

To validate packaged protocol support on a clean Linux install, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_protocol_validation.sh
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
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_protocol_validation.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_protocol_validation.sh --rpm
```

## Container Validation Summary

To run the packaged Linux container validation suite through one summary
entrypoint, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh
```

That wrapper runs, in order:

- [container_protocol_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_protocol_validation.sh)
- [container_operator_path_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_operator_path_validation.sh)

Naming note for the packaging scripts:

- `smoke` means install/bring-up confidence
- `validation` means one grouped packaged-behavior check
- `summary` means a wrapper over several packaged validations

If you only want one package family, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh --rpm
```

## Release Container Check

To run the current release-oriented packaged Linux validation suite through one
entrypoint, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh
```

That wrapper runs, in order:

- [package_install_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh)
- [container_runtime_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh)
- [container_validation_summary.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh)

If you only want one package family, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh --rpm
```

For the shorter release-minded decision shelf that also includes the
three-module integration gate, see:

- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)

If you want one orchestration command that rebuilds artifacts, runs the
packaged release check, and then runs the three-module stack smoke, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh
```

That ship gate belongs conceptually to
[docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md).
This page stays focused on the packaging and packaged-runtime mechanics behind
that gate.

## Container Operator-Path Validation

To validate more realistic packaged operator-path chains after install, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_operator_path_validation.sh
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
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_operator_path_validation.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_operator_path_validation.sh --rpm
```

## Notes

- packaging is Linux-oriented even if the staging script is run from another
  development host
- `--layout-only` is the safest local verification path when package manager
  tools are not installed
- the container path requires a working local Docker daemon
- current packages are intended for standalone host installs, not multi-host
  orchestration
