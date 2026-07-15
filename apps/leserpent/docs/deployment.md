# Leserpent Native AOT deployment

## Publish on the target platform

Native AOT output is platform-specific. Build Linux x64 releases on a compatible Linux x64 builder:

```bash
dotnet publish apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -r linux-x64 \
  -o artifacts/leserpent/linux-x64
```

The publish target also builds the Rust compatibility bridge with Cargo. The
output contains both native executables, the SQLite native library, dashboard
assets, and a `deploy` directory. Linux bundles must be built on Linux so the
bridge architecture matches the selected RID.

## Install or upgrade

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

Configuration is preserved across upgrades. Review it after first install:

```bash
sudoedit /etc/leserpent/leserpent.env
sudo systemctl restart leserpent
```

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

Use `--keep-releases N` to change release retention. Failed upgrades automatically restore the previous `current` link and restart it.

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
