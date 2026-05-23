# Native Packaging

`gewyvern` can be staged and packaged as native Linux artifacts for operator
install flows that do not want to build directly from source.

The packaging path is intentionally narrow:

- it packages the standalone runtime and compiler tools
- it ships the built-in DSL and protocol registry assets
- it does not yet try to provision system users, systemd units, or fleet-level
  orchestration glue

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

## Build Entry Point

Use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages.sh --layout-only
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
bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages.sh --format deb
```

Artifacts are written under:

- `/Users/Shared/chroot/dev/gewyvern/target/packages`

## Building An RPM

If the host has `rpmbuild`:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages.sh --format rpm
```

RPM artifacts are written under:

- `/Users/Shared/chroot/dev/gewyvern/target/packages/rpm`

## Building Inside The Bundled Linux Container

If the host itself does not provide `dpkg-deb` or `rpmbuild`, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages_in_container.sh --format all
```

That path uses:

- `/Users/Shared/chroot/dev/gewyvern/docker/linux-dev/Dockerfile`
- `/Users/Shared/chroot/dev/gewyvern/scripts/build_packages.sh`

and writes artifacts back into:

- `/Users/Shared/chroot/dev/gewyvern/target/packages`

## Notes

- packaging is Linux-oriented even if the staging script is run from another
  development host
- `--layout-only` is the safest local verification path when package manager
  tools are not installed
- the container path requires a working local Docker daemon
- current packages are intended for standalone host installs, not multi-host
  orchestration
