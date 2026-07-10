# Runtime Event Reference

This page is the exact lookup shelf for the runtime event names that are meant
to stay recognizable in the active `0.20.x` line, while preserving the earlier
event-contract groundwork.

Use it when you need to answer:

- which event names are intentionally stable enough to key alerts or parsers on?
- what event families exist today in runtime, API, DSL, diagnostics, and output?
- what does a structured `gewyvern` log line look like?

Do not use this page as:

- the full logging configuration reference
- the machine JSON contract
- a troubleshooting playbook

For those, use:

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/service-behavior.md](docs/service-behavior.md)

## Contract Posture

The event names on this page are the current named spine for operator-facing log
streams in `0.17.x`.

Minor fields may grow over time, but the intent is:

- the `event=` token remains machine-readable
- the value remains `snake_case`
- one event name describes one failure or transition class
- runtime, serve, API, DSL, diagnostics, and output paths do not invent casual
  one-off naming for the same condition

If a future line needs a different event meaning, prefer adding a new event
name over silently mutating the old one.

## Structured Log Shape

A structured record currently follows this shape:

```text
ts=2026-06-16T08:12:44.102Z level=INFO target=runtime event=runtime_config_loaded msg="loaded runtime config" config_path="config/runtime.toml"
```

Contract-relevant pieces are:

- `ts=` RFC 3339 timestamp with millisecond precision
- `level=` uppercase log level
- `target=` subsystem such as `runtime`, `serve`, `api`, or `dsl`
- `event=` stable event token from this page
- `msg=` human-readable summary
- trailing `key=value` fields for machine-friendly context

Sensitive values should not be copied into fields casually. Paths, sockets,
status labels, and counts are expected. Tokens, secrets, and raw credentials
are not.

## Stable Event Names

### Runtime And Config

- `runtime_config_loaded`
- `runtime_roots_prepared`
- `legacy_config_copied`
- `legacy_entries_migrated`
- `history_render_failed`

### Serve And Socket Runtime

- `unix_service_start`
- `tcp_service_start`
- `socket_stale_cleanup_failed`
- `socket_listener_bind_failed`
- `socket_session_collect_failed`
- `socket_session_run_failed`
- `socket_listener_cleanup_failed`
- `snapshot_persist_failed`

### API Surface

- `api_service_start`
- `api_listener_bind_failed`
- `api_client_accept_failed`
- `api_client_overload_rejected`

### DSL And Diagnostics

- `dsl_compile_failed`
- `diagnostics_requires_dsl`
- `diagnostics_compile_failed`
- `scan_target_resolve_failed`

### Output And Persistence

- `append_failed`
- `write_failed`

## Naming Rule

When adding a new event, keep the shape narrow:

1. prefer `subject_action_outcome` style in `snake_case`
2. name the class of transition, not a one-off sentence
3. keep the event stable even if the human `msg=` wording changes
4. add contextual fields instead of encoding detail into the event token

Good pattern examples:

- `socket_session_run_failed`
- `api_client_overload_rejected`
- `runtime_config_loaded`

Less desirable patterns:

- `socket_problem`
- `api_request_bad_thing_happened`
- `config_loaded_from_new_path_but_with_warning`

## Scope Note

This page does not mean `0.16.x` has already opened, and it does not promise
that every future internal trace point becomes part of the stable operator
contract.

It does name the current event spine that should remain understandable and safe
to reference in:

- release notes
- operator docs
- alert routing
- lightweight log parsing
