# Leserpent v2.0.x

<p align="center">
  <a href="README.md">Gewyvern</a> ·
  <strong>Leserpent</strong> ·
  <a href="docs/leselang-language.md">Leselang</a> ·
  <a href="docs/index.md">Documentation</a> ·
  <a href="https://github.com/Team-silvortex/gewyvern/releases/latest">Download</a>
</p>

<p align="center">
  <img src="assets/branding/leserpent-icon.png" alt="Leserpent feathered serpent icon" width="220">
</p>

<p align="center">
  Cross-platform Orchestra, deployment, and automation control plane for
  Gewyvern fleets.
</p>

Leserpent turns independent Gewyvern debuggers into one manageable system. It
provides a native desktop Hub, self-hosted Web console, Rust CLI, daemon
authority, remote deployment workflow, and Leselang automation surface without
moving network-debugging authority out of Gewyvern.

All capabilities shipped in the 2.0 open core are MIT-licensed and usable
without a Team Silvortex account. Credentials protect infrastructure authority;
they are not a subscription gate.

## System Model

```text
Avalonia desktop / Web / Rust CLI / Leselang
                    |
                    | typed command, query, UI, and effect contracts
                    v
          one or more leserpentd authorities
                    |
                    | each authority manages many runtimes
                    v
       Gewyvern service per kernel or container instance
                    |
                    v
          Linux network-debugging and eBPF evidence
```

- One kernel or container instance maps to one Gewyvern debugging service.
- One `leserpentd` manages multiple Gewyvern services and owns one authenticated
  control and Web endpoint.
- One Leserpent client can manage multiple independent `leserpentd` authorities.
- Avalonia, Web, CLI, and Leselang use the same protocol operations instead of
  maintaining separate control semantics.
- Reverse deployment first installs or connects a `leserpentd`, then uses that
  authority to provision, inspect, or retire Gewyvern runtimes.

## Start Here

1. Download the current community build from
   [GitHub Releases](https://github.com/Team-silvortex/gewyvern/releases/latest).
2. Open the desktop Hub and choose Local Orchestra, connect an existing daemon,
   or deploy a new daemon.
3. Follow the [first desktop session](docs/book/tutorial-leserpent-desktop.md),
   or use the [remote deployment lab](docs/book/tutorial-remote-deployment-lab.md)
   for the complete daemon-to-runtime lifecycle.

The Hub is the normal entry point. An account is optional, and the application
does not require a remote connection before local Orchestra can be used.

### Choose An Artifact

| Goal | Community artifact |
| --- | --- |
| Operate from an Apple Silicon Mac | `Leserpent-2.0.0-macos-arm64-adhoc.zip` |
| Host the Linux control plane and Web console | `leserpent-control-2.0.0-linux-x86_64.tar.gz` |
| Install Gewyvern on Debian or Ubuntu | `gewyvern_2.0.0-1_amd64.deb` |
| Install Gewyvern on an RPM-based x86-64 host | `gewyvern-2.0.0-1.x86_64.rpm` |

The macOS community archive is ad-hoc signed and not Apple-notarized. Verify all
downloads with the `SHA256SUMS` file attached to the same release.

## Build From Source

```bash
cargo dev doctor
cargo dev version check

# macOS NativeAOT desktop application
cargo dev package desktop

# Linux x86-64 NativeAOT control bundle
cargo dev package control

# Native Rust command and daemon surfaces
cargo run --locked -p leserpent-cli -- --help
cargo run --locked -p leserpentd -- --help
```

On macOS, Linux-heavy packaging and validation can use the configured physical
Linux host. See [packaging](docs/packaging.md) and
[remote execution](docs/remote-docker.md) for the reproducible paths.

## What 2.0 Ships

- Hub-first Avalonia desktop experience with Local Orchestra and multiple
  daemon workspaces.
- Independent child workspaces for the Gewyvern runtimes managed by an
  authority.
- Authenticated local IPC and remote HTTPS control through `leserpentd`.
- Credential-handle-based daemon bootstrap and Gewyvern provision, inspect,
  logs, upgrade, rollback, and retirement lifecycles.
- Self-hosted Web console and native Rust CLI over the same authority model.
- Leselang parser, HIR, VM, UI protocol, effect journal, deterministic re-entry,
  and canonical GUI-operation export.
- Persistent Orchestra history, typed recovery, idempotency, audit, and
  fail-closed mutation behavior.
- Official locale catalog and installable language-pack support.

## Platform Posture

- Gewyvern's eBPF data plane remains Linux-only.
- Leserpent itself does not depend on eBPF and is designed as a portable control
  plane.
- macOS and Linux are the primary native 2.0 operator platforms.
- Windows can use the Web console; native Windows packaging remains post-2.0
  work.
- Mobile sources and conformance contracts are present, while broader physical
  mobile release parity remains a later validation track.

## Read Next

- [Leserpent 2.0 architecture](docs/leserpent-2-architecture.md)
- [Leserpent delivery record](docs/leserpent-2-roadmap.md)
- [Native CLI reference](docs/leserpent-cli.md)
- [GUI function chains](docs/leserpent-gui-function-chains.md)
- [Leselang language](docs/leselang-language.md)
- [2.0.0 release notes](docs/history/v2.0.0-release-notes.md)
- [Implementation and compatibility-bridge notes](apps/leserpent/README.md)
