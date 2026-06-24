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
- [docs/book/reference-runtime-certificate-policy.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-certificate-policy.md)
- [docs/book/reference-runtime-certificate-state.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-certificate-state.md)
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

Before selecting the file, active `0.17.x` startup prepares the standard runtime
roots and performs a conservative config copy-forward:

- if the standard `gewyvern.toml` is missing
- and a legacy `~/.gewyvern/config.toml` exists
- the legacy file is copied into the standard config root
- the legacy source file is left untouched

The same conservative copy-forward behavior now applies to legacy operator
certificate assets:

- legacy `~/.gewyvern/certificates/` contents are copied into the standard
  certificate root when the destination files are missing
- legacy `~/.gewyvern/state/certificates/` contents are copied into the
  standard certificate state root when the destination files are missing
- existing files in the standard roots are never overwritten during migration

## Current Config Shape

The current config format is a small TOML-style file with these sections:

- top-level `schema_version = 1`

- `[runtime]`
- `[external_engine]`
- `[paths]`
- `[certificates]`
- `[logging]`
- `[resilience]`

Unknown sections or keys are rejected.

Unknown top-level keys are also rejected. The only supported top-level key is:

- `schema_version = 1`

## Schema Version Rules

The current config contract version is:

- `schema_version = 1`

Recommended posture:

- new config files should include `schema_version = 1` at the top of the file
- unversioned older files are still accepted as a compatibility path in the
  active `0.17.x` line
- if a file declares a higher version than the runtime understands, startup
  fails instead of guessing

This means `gewyvern` treats config upgrades conservatively:

- explicit future schema -> reject
- explicit current schema -> load normally
- missing schema version -> load as `legacy_unversioned` compatibility input

The runtime logger now records both:

- `schema_version`
- `schema_status`

where `schema_status` is currently either:

- `current`
- `legacy_unversioned`

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

### `[certificates]`

- `root = "/path/to/certificates"`
- `trust_root = "/path/to/certificates/trust"`
- `authority_root = "/path/to/certificates/authorities"`
- `identity_root = "/path/to/certificates/identities"`
- `state_root = "/path/to/state/certificates"`
- `require_explicit_remote_trust = true|false`

These keys establish the operator-facing certificate shelf used by the runtime
for future TLS identity, trust-anchor, and local authority management.

The corresponding environment overrides are:

- `GEWY_CERTIFICATE_ROOT`
- `GEWY_TRUST_ROOT`
- `GEWY_AUTHORITY_ROOT`
- `GEWY_IDENTITY_ROOT`
- `GEWY_CERTIFICATE_STATE_ROOT`
- `GEWY_REQUIRE_EXPLICIT_REMOTE_TRUST`

The runtime also publishes the current discovered certificate inventory at:

- `/v1/runtime/certificates.json`

And it now publishes a policy interpretation layer at:

- `/v1/runtime/certificate-policy.json`

And it now publishes the runtime-managed certificate state shelf at:

- `/v1/runtime/certificate-state.json`

This policy surface is intentionally conservative. In the active `0.17.x`
line it highlights:

- explicit remote trust without any trust anchors
- private keys mistakenly stored in the trust shelf
- identity keys without matching certificate material
- identity certificates present without matching private keys
- empty authority shelves
- missing certificate state roots
- parsed certificate material that is already expired
- parsed certificate material that is approaching expiry

For the stable reason-code contract and status meanings, use:

- [docs/book/reference-runtime-certificate-policy.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-certificate-policy.md)

### `[resilience]`

- `external_failure_circuit_threshold = 3`
- `external_failure_circuit_cooldown_seconds = 30`
- `socket_failure_backoff_base_ms = 100`
- `socket_failure_backoff_cap_ms = 2000`

These keys currently tune the two runtime fault-tolerance loops introduced at
the end of the `0.15.x` line and carried into `0.16.x`:

- external analysis repeated-failure circuit breaking
- socket service repeated-failure backoff

They are intentionally narrow. They do not yet define a full retry-policy
language.

The corresponding environment overrides are:

- `GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD`
- `GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS`
- `GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS`
- `GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS`

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

[certificates]
root = "/srv/gewyvern/certificates"
trust_root = "/srv/gewyvern/certificates/trust"
authority_root = "/srv/gewyvern/certificates/authorities"
identity_root = "/srv/gewyvern/certificates/identities"
state_root = "/srv/gewyvern/state/certificates"
require_explicit_remote_trust = true

[logging]
level = "info"
stderr = true
file = "/srv/gewyvern/state/logs/runtime.log"
max_bytes = 1048576
max_files = 4

[resilience]
external_failure_circuit_threshold = 3
external_failure_circuit_cooldown_seconds = 30
socket_failure_backoff_base_ms = 100
socket_failure_backoff_cap_ms = 2000
```

A copyable sample file also lives at:

- [docs/fixtures/gewyvern.toml.example](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyvern.toml.example)

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
- config-level `resilience.*` values only apply when the corresponding
  resilience environment variable is not already set

That keeps environment overrides stronger than file-level path hints.

## Fault-Injection Notes

For `0.16.x`, the resilience keys are meant to support simple fault-injection
and recovery drills without recompiling:

1. point `[external_engine].bin` at a helper that times out or exits non-zero
2. lower `external_failure_circuit_threshold` to `2` or `3`
3. confirm logs emit:
   - `external_analysis_failed`
   - `external_analysis_circuit_open`
   - `external_analysis_recovered`
4. drive repeated socket session failures and confirm serve-time logs include:
   - `consecutive_failures=...`
   - `total_failures=...`
   - `backoff_ms=...`
5. restore a healthy peer and confirm `socket_service_recovered`

This is not yet a full chaos-test harness, but it gives operators and
maintainers a stable manual path for verifying that repeated failure does not
degrade into indefinite hangs or hot-loop retry storms.

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

The current `0.17.x` startup behavior now pairs this config contract with a
standard state root, so `--serve` can mirror the latest API snapshot to disk
without inventing ad-hoc paths.
