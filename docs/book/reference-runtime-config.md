# Runtime Config Reference

This page records the first formal runtime config file contract for
`gewyvern` in the `0.15.x` line.

Use it when the question is:

- where does the runtime config file live?
- which keys are supported today?
- what is the precedence between CLI, config, environment, and legacy paths?
- how should an older local setup be carried forward safely?

Do not use this page as:

- the full runtime layout reference
- the packaging build guide
- the protocol surface contract page

For those, use:

- [docs/book/reference-runtime-layout.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-layout.md)
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)

## Role In The Shelf

Treat this page as the exact lookup page for:

- config file name
- config search order
- supported sections and keys
- override precedence

It is intentionally narrow.

## Config File Search Order

At startup, `gewyvern` looks for a runtime config file in this order:

1. `GEWY_CONFIG_FILE`
2. standard config root:
   - Linux/XDG:
     `$XDG_CONFIG_HOME/gewyvern/gewyvern.toml` or
     `~/.config/gewyvern/gewyvern.toml`
   - macOS:
     `~/Library/Application Support/gewyvern/config/gewyvern.toml`
   - Windows:
     `%APPDATA%\\gewyvern\\config\\gewyvern.toml`
3. legacy fallback:
   - `~/.gewyvern/config.toml`
   - `~/.gewyvern/gewyvern.toml`

If no file exists, startup continues with built-in defaults.

Before selecting the file, `0.15.x` startup now prepares the standard runtime
roots and performs a conservative config copy-forward:

- if the standard `gewyvern.toml` is missing
- and a legacy `~/.gewyvern/config.toml` exists
- the legacy file is copied into the standard config root
- the legacy source file is left untouched

## Current Config Shape

The current config format is a small TOML-style file with these sections:

- `[runtime]`
- `[external_engine]`
- `[paths]`

Unknown sections or keys are rejected.

## Supported Keys

### `[runtime]`

- `serve = true|false`
- `socket = "unix:/path.sock"` or `socket = "tcp:127.0.0.1:9000"`
- `api_socket = "127.0.0.1:9910"`
- `allow_remote_api = true|false`
- `ingest_mode = "local-advisory"` or `"remote-advisory"`
- `max_sessions = 32`
- `history_retention = 32`

### `[external_engine]`

- `bin = "/path/to/engine"`
- `worker = "/path/to/worker.py"`
- `python_bin = "/usr/bin/python3"`

### `[paths]`

- `protocol_registry_root = "/path/to/protocols"`
- `share_root = "/path/to/share"`

These path keys are configuration-level equivalents of:

- `GEWY_PROTOCOL_REGISTRY_ROOT`
- `GEWY_SHARE_ROOT`

The runtime history key also has an environment-level override:

- `GEWY_HISTORY_RETENTION`

## Example

```toml
[runtime]
serve = true
socket = "unix:/tmp/gewyvern.sock"
api_socket = "127.0.0.1:9910"
allow_remote_api = false
ingest_mode = "local-advisory"
max_sessions = 64
history_retention = 48

[external_engine]
bin = "/opt/etragon/bin/etragon"
worker = "/opt/etragon/bin/worker.py"
python_bin = "/usr/bin/python3"

[paths]
protocol_registry_root = "/srv/gewyvern/protocols"
share_root = "/srv/gewyvern/share"
```

## Precedence Rules

The current precedence is:

1. explicit CLI arguments
2. runtime config file
3. explicit environment variables for path roots
4. standard runtime layout defaults
5. legacy compatibility roots
6. packaged read-only share root
7. repo-local development root when running from source

More precisely:

- service and external-engine defaults come from the config file, but CLI flags
  override them
- runtime history retention comes from the config file unless
  `GEWY_HISTORY_RETENTION` is already set
- config-level `paths.*` values only apply when the corresponding environment
  variable is not already set

That keeps environment overrides stronger than file-level path hints.

## Legacy Upgrade Rule

If an older local setup only has:

- `~/.gewyvern/config.toml`

the `0.15.x` runtime will still read it, and startup may copy it forward into
the standard config root if that newer path does not exist yet.

Recommended upgrade path:

1. leave the old file intact
2. create the new standard config root for the host OS
3. let startup copy the file forward, or copy it manually to
   `gewyvern.toml` under the standard root
4. validate startup from the new path
5. only remove the legacy file after the new path is confirmed

## Current Scope Limit

The current config file is intentionally conservative.

It is for:

- service bring-up defaults
- external engine wiring
- protocol/share root overrides

It is not yet the home for every CLI mode or every future persistence feature.

That broader surface can grow later on top of the now-explicit path and config
contract.

The current `0.15.x` startup behavior now pairs this config contract with a
standard state root, so `--serve` can mirror the latest API snapshot to disk
without inventing ad-hoc paths.
