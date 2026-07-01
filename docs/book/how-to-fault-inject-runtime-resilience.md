# How To Fault-Inject Runtime Resilience

Use this page when the question is:

- did the new `0.16.x` resilience controls actually stop a hang or retry storm?
- how do I verify external-analysis circuit breaking on purpose?
- how do I verify socket-session backoff on purpose?

Do not use this page as:

- the full runtime config reference
- the security deployment checklist
- the release checklist

For those, use:

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
- [docs/release-checklist.md](docs/release-checklist.md)

## Goal

This guide exists to prove one narrow claim:

`gewyvern` should degrade visibly under repeated failure instead of hanging
indefinitely or spinning in a hot retry loop.

The two current resilience loops are:

1. external analysis repeated-failure circuit breaking
2. socket service repeated-failure backoff

This page gives a repeatable manual drill for both.

## Before You Start

Prepare:

1. a writable runtime config file
2. a writable runtime log path
3. one terminal for `gewyvern`
4. one terminal for the helper client or failure script

If you want a prebuilt set of helper scripts and a ready-made runbook, use:

```sh
bash scripts/validation/runtime_resilience_roundtrip.sh
```

That helper prepares:

- a timeout external-engine helper
- a hard-fail external-engine helper
- a healthy recovery helper
- a small config snippet
- a text runbook with the expected log signals

If you are using the standard config path, start from:

- [docs/fixtures/gewyvern.toml.example](docs/fixtures/gewyvern.toml.example)

The resilience keys we care about are:

```toml
[resilience]
external_failure_circuit_threshold = 3
external_failure_circuit_cooldown_seconds = 30
socket_failure_backoff_base_ms = 100
socket_failure_backoff_cap_ms = 2000
```

For drills, it is often easier to lower the threshold and cooldown:

```toml
[resilience]
external_failure_circuit_threshold = 2
external_failure_circuit_cooldown_seconds = 10
socket_failure_backoff_base_ms = 100
socket_failure_backoff_cap_ms = 800
```

## Drill A: External Analysis Circuit Breaking

### Why

This drill proves that a misbehaving external engine does not keep making the
main runtime wait for repeated timeouts forever.

### Setup

Point `[external_engine].bin` to a helper that always times out or exits
non-zero.

You can generate one with:

```sh
bash scripts/validation/runtime_resilience_fault_injection.sh \
  emit-external-engine timeout \
  /tmp/gewyvern-fault-timeout-engine.sh
```

Example timeout script:

```sh
#!/bin/sh
sleep 5
printf 'late\n'
```

Example failure script:

```sh
#!/bin/sh
printf 'broken\n' >&2
exit 1
```

Then set:

```toml
[external_engine]
bin = "target/validation/gewyvern-fault-failing-engine.sh"
```

For a quick recovery drill, you can also emit a healthy helper:

```sh
bash scripts/validation/runtime_resilience_fault_injection.sh \
  emit-external-engine healthy \
  /tmp/gewyvern-fault-healthy-engine.sh
```

### Run

Start a focused command that exercises analysis output:

```sh
cargo run -- --diagnostics --summary
```

Run it enough times to cross the configured failure threshold.

### Expected Signals

The runtime log should show:

1. `event=external_analysis_failed`
2. `event=external_analysis_circuit_open`

After the circuit opens, subsequent attempts within the cooldown window should
skip expensive external retries and fall back quickly.

### Recovery Check

Now replace the failing helper with a healthy one, or restore the real engine
path, then run the same command again after the cooldown window.

The runtime log should show:

1. `event=external_analysis_recovered`

If you still only see repeated `external_analysis_failed` without recovery,
the helper or config path is still broken.

## Drill B: Socket Session Failure Backoff

### Why

This drill proves that repeated socket-session failures do not turn the serve
loop into a high-frequency error storm.

### Setup

Use serve mode with a socket target:

```sh
cargo run -- --serve --tcp-socket 127.0.0.1:9909 --max-sessions 20
```

Then repeatedly connect with intentionally bad or incomplete input. The goal is
not to crash the listener. The goal is to trigger repeated session failure.

Examples:

- connect and send malformed fact JSON
- connect and send oversized lines
- connect and disconnect without a valid fact stream

### Run

Drive repeated failures from another terminal. For example, repeatedly connect
and send one invalid line.

The exact client tool does not matter; `nc`, `socat`, or a tiny script are all
fine.

If `nc` is available, a helper is now included:

```sh
bash scripts/validation/runtime_resilience_fault_injection.sh \
  drive-socket-bad-json 127.0.0.1 9909 6
```

### Expected Signals

The runtime log should show session failures with structured counters:

1. `event=socket_session_collect_failed` or `event=socket_session_run_failed`
2. `consecutive_failures=...`
3. `total_failures=...`
4. `backoff_ms=...`

As failures continue, `backoff_ms` should grow until it reaches the configured
cap.

This is the key proof that the loop is not hot-spinning.

### Recovery Check

Once the bad input stops, send one valid session payload.

The runtime log should then show:

1. `event=socket_service_recovered`

That confirms the loop does not stay permanently throttled after one healthy
session succeeds.

## API Surface For Control Planes

If a fleet view such as `leserpent` wants one structured checkpoint instead of
replaying logs, use:

```text
GET /v1/runtime/resilience.json
```

That surface now exposes:

- `degraded`
- `status`
- `severity`
- `summary`
- `recommended_actions`
- `external_analysis`
- `socket_service`

The intent is simple:

- the control plane should not need to reconstruct warning state from raw
  counters alone
- the runtime should publish enough structure that a nearby dashboard can show
  posture, advice, and per-component summaries directly

Example interpretation:

- `status = "healthy"` means no bounded-fallback loop is active
- `status = "degraded"` usually means socket-session failures are currently
  triggering backoff
- `status = "circuit_open"` means the external analysis circuit is open and the
  runtime is intentionally serving with fallback

The `/health` endpoint also exposes:

- `resilience_degraded`

Use that flag for very small probes.
Use `/v1/runtime/resilience.json` when the UI needs operator-facing detail.

## Fast Review Matrix

Use this matrix to decide whether the resilience layer behaved correctly.

| Scenario | Expected outcome |
| --- | --- |
| External engine fails once | `external_analysis_failed` only |
| External engine keeps failing | `external_analysis_circuit_open` appears after threshold |
| External engine becomes healthy again | `external_analysis_recovered` appears |
| One socket session fails | failure event, usually no backoff yet |
| Many socket sessions fail in a row | failure event plus growing `backoff_ms` |
| One healthy socket session after failures | `socket_service_recovered` appears |

## Common Misreads

### “I still see failure logs, so the circuit is broken.”

Not necessarily.
The circuit is meant to reduce repeated expensive work, not hide failure.
If the engine is still unhealthy, failure should remain visible.

### “The backoff is too small to notice.”

That may just mean the configured cap is conservative.
Lower the threshold for testing or raise the cap briefly in a non-production
drill.

### “Nothing changed after editing the config file.”

Check precedence:

1. CLI flags override config defaults
2. resilience environment variables override config-level resilience keys

If an environment variable is already set, the file value will not replace it.

## Suggested `0.16.x` Evidence

For release or review notes, keep:

1. one log excerpt showing `external_analysis_circuit_open`
2. one log excerpt showing `external_analysis_recovered`
3. one log excerpt showing socket failures with `backoff_ms`
4. one log excerpt showing `socket_service_recovered`

That is enough evidence to support the current claim that repeated failure
degrades into bounded, visible fallback instead of silent hanging.

If you already have one runtime log file or one log directory, a helper can
extract the resilience evidence and summarize the event counts:

```sh
bash scripts/validation/runtime_resilience_log_evidence.sh \
  target/validation/runtime.log \
  /tmp/gewyvern-resilience-evidence
```

That helper writes:

- `resilience-events.log`
- `resilience-summary.txt`

If you want one output bundle that combines:

- helper scripts
- config snippet
- runbook
- resilience evidence summary

use:

```sh
bash scripts/validation/runtime_resilience_validation.sh \
  127.0.0.1:9910 \
  target/validation/runtime.log \
  /tmp/gewyvern-resilience-validation
```

That wrapper does not replace the drill itself.
It packages the current roundtrip helper and the current evidence extractor
into one reviewable directory that is easier to archive with `0.16.x`
validation notes.
