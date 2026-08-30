# Leserpent Native AOT deployment

## Package on the target platform

Native AOT output is platform-specific. Build Linux x64 releases on a compatible Linux x64 builder:

```bash
cargo dev package control
```

This is the recommended release-shaped entrypoint. It restores the locked
NativeAOT graph and builds the Rust compatibility bridge plus `leserpentd` in
parallel, publishes the managed host without rebuilding those Rust payloads,
then atomically replaces `artifacts/leserpent/linux-x64` only after the whole
pending bundle passes validation.

The bundle carries `bundle-manifest.toml` and a sorted `SHA256SUMS`. Their
contract binds the exact symlink-free file inventory, payload byte count,
Linux x86-64 executable format, product version, dashboard assets, deployment
files, managed host, bridge, and daemon. An installer verifies the source and
the copied immutable release before switching `current`; any unlisted,
missing, or modified file fails before activation.

The lower-level publish path remains available for diagnostics:

```bash
dotnet restore apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -p:PublishAot=true \
  -p:RuntimeIdentifier=linux-x64 \
  --locked-mode
dotnet publish apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -p:PublishAot=true \
  -r linux-x64 \
  --no-restore \
  -o artifacts/leserpent/linux-x64
```

The lower-level publish target builds both Rust payloads in one Cargo
invocation and copies the Linux deployment files. It does not write the strict
content-addressed metadata owned by `cargo dev package control`. Linux bundles
must be built on Linux so every native payload matches the selected RID.

Release builds run the native `leserpent-frontend-package` Rust coordinator
before static-web-asset discovery. Its content-addressed fast path does not
start Node. MSBuild compiles the coordinator only when its Rust source or locked
dependency graph changes, then executes the native binary directly on every
Release build so asset freshness remains content-verified without paying a
`cargo run` startup cost. Only a stale source tree invokes the locked TypeScript
toolchain; that rebuild also verifies every official language pack and
atomically refreshes `frontend-package-manifest.json`. The published dashboard
assets are exposed through .NET `MapStaticAssets`, so JavaScript,
CSS, HTML, SVG, and JSON use build-time Brotli/Gzip negotiation and content
ETags instead of spending runtime CPU on first-request compression.

Validate the complete bundle without changing the host installation:

```bash
scripts/validation/leserpent_linux_bundle_smoke.sh artifacts/leserpent/linux-x64
```

The smoke uses a temporary `DESTDIR`, installs and upgrades two immutable
releases, explicitly rolls back through the `current`/`previous` links, proves
configuration and state preservation, executes a real bridge request, starts
the rolled-back Native AOT host on loopback, and retains a machine-readable
result below `target/validation/leserpent-linux-bundle-smoke/`.

## Install or upgrade

```bash
cargo dev deploy control
```

The native command builds a fresh verified bundle and invokes its installer.
Use `cargo dev deploy control --reuse` to skip compilation only when the
existing default bundle still passes full manifest and checksum validation.
Pass `--no-start` or `--keep-releases N` through the same entrypoint.
The lower-level installer remains available as:

```bash
sudo artifacts/leserpent/linux-x64/deploy/install.sh
```

The installer:

- creates the system user and group `leserpent`
- installs immutable releases below `/opt/leserpent/releases`
- atomically points `/opt/leserpent/current` at the new release
- writes the systemd unit to `/etc/systemd/system/leserpent.service`
- creates `/etc/leserpent/leserpent.env` once and generates a 256-bit admin token
- enables the bundled `leserpent-compat-bridge` through an absolute release path
- stores mutable state and SQLite data below `/var/lib/leserpent`
- starts the service, waits for `/health`, and rolls back the release link if health fails
- retains the three newest healthy releases by default
- serializes real host mutation with a bounded installation lock
- removes an uncommitted release and restores the stable link after any failed upgrade stage
- updates the systemd unit in the same transaction as the release link, including rollback

Configuration is preserved across upgrades. Review it after first install:

```bash
sudoedit /etc/leserpent/leserpent.env
sudo systemctl restart leserpent
```

Explicitly switch back to the retained previous release:

```bash
sudo /opt/leserpent/current/deploy/install.sh --rollback
```

Installed releases retain their verified `deploy` directory, so rollback does
not require a separate source checkout. The command validates its bundle,
atomically switches the installed `current` link, restarts the service, waits
for health, and restores the original links if the rollback target is
unhealthy. A successful rollback keeps the displaced release as `previous`,
allowing an operator to switch back.

The secure default listens on `127.0.0.1:5210`. Keep that binding when a TLS reverse proxy runs on the same host. If clients reach Leserpent through a proxy, provide the generated admin token through the UI security control or the `X-Leserpent-Admin-Token` API header.

## Operations

```bash
systemctl status leserpent
journalctl -u leserpent -f
curl --fail http://127.0.0.1:5210/health
```

Install files without starting systemd:

```bash
sudo artifacts/leserpent/linux-x64/deploy/install.sh --no-start
```

Stage the filesystem layout for packaging or validation without changing the host:

```bash
DESTDIR=/tmp/leserpent-root artifacts/leserpent/linux-x64/deploy/install.sh
```

Use `--keep-releases N` with an integer from 2 through 64 to change release
retention. Failed upgrades automatically restore the previous `current` link,
restart it when service activation had begun, and remove the uncommitted
release.
The selected release and `/etc/systemd/system/leserpent.service` therefore stay
version-aligned after upgrades, explicit rollback, and failed health checks.

## Remove a host installation

Export any required state before removal, then stop and remove the service and its files:

```bash
sudo systemctl disable --now leserpent
sudo rm -f /etc/systemd/system/leserpent.service
sudo systemctl daemon-reload
sudo rm -rf /opt/leserpent /etc/leserpent /var/lib/leserpent
sudo userdel leserpent
sudo groupdel leserpent
```
