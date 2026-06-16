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
- `[logging]`

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

### `[logging]`

- `level = "error" | "warn" | "info" | "debug"`
- `stderr = true|false`
- `file = "/path/to/gewyvern.log"`
- `max_bytes = 1048576`
- `max_files = 4`

These keys control the unified runtime logger used by startup and serve-time
operational messages.

Runtime log records in `0.15.x` now follow a light structured text form:

- `target=...` identifies the subsystem
- `event=...` identifies the operation or failure shape
- extra fields such as `path=...`, `socket=...`, or `error=...` may appear
  before `msg=...`
- event names are intentionally stable, lower-snake-case identifiers such as
  `runtime_config_loaded`, `socket_listener_bind_failed`, or `write_failed`
- interactive and one-shot flows use the same pattern for DSL and diagnostics
  failures, for example `dsl_compile_failed`, `diagnostics_compile_failed`, or
  `scan_target_resolve_failed`

If `logging.file` is omitted, the runtime now falls back to the standard state
root log path:

- Linux/XDG:
  `"$XDG_STATE_HOME/gewyvern/logs/runtime.log"` or
  `~/.local/state/gewyvern/logs/runtime.log`
- macOS:
  `~/Library/Application Support/gewyvern/state/logs/runtime.log`
- Windows:
  `%LOCALAPPDATA%\\gewyvern\\state\\logs\\runtime.log`

When `logging.file` is active, `0.15.x` now uses a built-in light rotation
policy:

- `runtime.log` is the active file
- `runtime.log.1` is the newest rotated archive
- `runtime.log.2` and higher are older retained archives
- once the active file would exceed `max_bytes`, it rotates before the next
  record is written
- `max_files = 0` keeps no archive copies and simply truncates by reopening a
  fresh active file

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

[logging]
level = "info"
stderr = true
file = "/srv/gewyvern/state/logs/runtime.log"
max_bytes = 1048576
max_files = 4
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
- logging defaults come from the config file, but CLI flags such as
  `--log-level`, `--log-file`, `--log-stderr`, and `--no-log-stderr` override
  them
- log rotation size and archive retention currently come from the config file
  or built-in defaults
- if no log file is configured explicitly, startup falls back to the standard
  state-root log path
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
