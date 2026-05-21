# Examples Guide

This guide is the shortest path from "I have the repo" to "I can use
`gewyvern` to answer a real debugging question".

The examples below focus on the operator-facing runtime, not the compiler.

If you need a short contract note for which CLI flags and JSON fields are
are the current machine-facing contract candidates, read
[docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md)
alongside this guide.

## When To Use What

Use `--protocol` when:

- you already know the protocol family you care about
- you want a focused single-path report
- you want to inspect one module deeply

Use `--scan-all` when:

- you do not yet know which protocol path matters
- one process may participate in multiple network modules
- you want a report that ranks likely problem areas

Use `--pid` when:

- multiple processes are active on the host
- you want one process image, not host-wide traffic
- you are diagnosing one concrete tool such as `apt`, `curl`, `ffmpeg`,
  `mysqldump`, or a proxy process

## Example 1: Single Protocol Summary

Inspect a PostgreSQL client session:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
```

This is the fastest way to answer:

- did the path match?
- what module kind did it look like?
- where did it stop?
- how confident is that conclusion?

Look first at:

- `kind`
- `name`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`

Interpretation:

- `primary_failure_confidence=high`
  means the result is backed by a protocol-direct signal such as a denied or
  explicit error response.
- `primary_failure_confidence=medium`
  usually means the result is based on a missing transition such as
  `send_query->receive_ok`.
- `primary_failure_confidence=low`
  means the runtime is deliberately being conservative and compressing weaker
  evidence into a summary conclusion.

## Example 2: Focused HTML Report

Render a single-target visual report:

```bash
cargo run -- --protocol http3 --entry request --report-format html --out /tmp/http3-request.html
```

This is useful when you want:

- the target-level conclusion card
- process-level network profiles
- per-flow failure mode and failure basis

The HTML report is a good fit for sharing a debugging snapshot with someone
else who does not want to read raw JSON.

## Example 3: Full Built-In Sweep

Run the built-in registry sweep:

```bash
cargo run -- --scan-all --json --summary-only
```

This is the fastest way to answer:

- which registered protocol paths matched at all
- which ones are healthy
- which ones are attention-worthy
- which module families dominate the result

Useful fields:

- top-level `kind`
- top-level `target_count`
- top-level target counts
- `targets[*].primary_module_kind`
- `targets[*].primary_failure_mode`
- `targets[*].primary_failure_confidence`
- `targets[*].process_network_profiles`
- `targets[*].protocol_flows`

## Example 4: PID-Scoped Sweep

Limit the full scan to one process:

```bash
cargo run -- --scan-all --pid 4242 --json --summary-only
```

This is the most practical command for questions like:

- "where is this one process getting stuck?"
- "is this process blocked in DNS, connect, auth, relay, or request/response?"
- "which protocol-shaped module best explains the current evidence?"

The most useful block is usually:

- `process_network_profiles`

That view compresses multiple matched protocol flows into one process-oriented
network picture.

## Example 5: Socket Ingest With HTML

If facts are coming from a socket producer:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --summary-only --report-format html --out /tmp/process-scan.html
```

If another local service needs to consume the latest serve-session result over
HTTP:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/targets
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
curl http://127.0.0.1:9100/v1/latest/targets/scan:http:request/report.json
```

When you need a target-specific route, prefer discovering it from
`/v1/latest/targets` and use the returned `target_refs[].path_segment` field
instead of guessing how to encode the name yourself. The API uses
percent-encoded path segments for any characters outside the direct-safe set
`A-Z a-z 0-9 . _ ~ :`.

Pay attention to:

- `ingest_mode`
- `ingest_mode_note`
- `ingest_trust_mode`
- `pid_attribution_status`
- `pid_attribution_note`
- `primary_failure_confidence`
- `primary_failure_basis`

That combination tells you both what the runtime thinks and how directly the
evidence supports that conclusion.

If another service wants a machine-friendly intermediate result instead of a
rendered report, prefer:

- `/v1/latest/analysis.json`
- `/v1/latest/targets/<path-segment>/analysis.json`

Those surfaces expose the analysis snapshot directly, including:

- `protocol_flows`
- `process_network_profiles`
- `primary_module_kind`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `ambiguous`
- `augmentations`

`augmentations` is intentionally the extension slot for future enrich/rerank
passes. It already carries both built-in advisory items and any external
augmentations that `gewyvern` merges back from a sibling engine such as
`etragon`, without forcing other services to parse the human-oriented report
surfaces. The built-in chain already emits lightweight advisory items such as:

- `unverified_ingest_lineage`
- `competing_hypotheses`
- `automation_recommendation`

If you want `gewyvern` to call an external engine binary itself and merge those
augmentations back into its own outputs, add:

```bash
--external-engine-bin /Users/Shared/chroot/dev/etragon/target/debug/etragon
```

If you want that hook to use a Python-backed worker path instead of the
engine's default Rust pass, add:

```bash
--external-engine-worker /Users/Shared/chroot/dev/etragon/scripts/python_baseline_worker.py
```

`etragon` is the sibling engine we currently use in examples, but `gewyvern`
only assumes a generic external-engine protocol.

If another service is deciding between:

- `summary.json`
- `analysis.json`
- `report.json`

prefer:

- `summary.json` for one conservative rendered conclusion per target
- `analysis.json` for machine-facing enrich/rerank/sidecar integration
- `report.json` only when you intentionally want the richer rendered report
  payload, not the narrowest long-lived automation contract

`automation_recommendation` is the first built-in rerank/enrich style pass. It
does not replace the core conclusion; it gives downstream automation a
conservative next-action hint such as:

- `avoid_pid_strong_actions`
- `keep_multiple_hypotheses`
- `safe_to_escalate_protocol_signal`
- `collect_more_runtime_evidence`
- `competing_hypotheses`

Important:

- `--pid` is intentionally rejected with socket ingest

If you want to test the generic external-engine bridge end to end, run:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json
```

By default that script looks for a sibling `/../etragon` repo and runs:

```bash
cargo run -- analyze-url
```

inside that engine root. To point it at a different implementation, set:

```bash
ENGINE_ROOT=/path/to/external-engine
EXTERNAL_ENGINE_CMD='cargo run -- analyze-url'
```

To make the bridge consume a target-specific route, pass the target path segment
as the sixth argument:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json socket_session
```

That gives you:

- a real `gewyvern` `/v1/latest/analysis.json`
- a real external augmentation payload
- a concrete example of the open analysis chain in action
- direct `analyze-url` consumption of the live `gewyvern` API
- lineage arriving over socket ingest is treated as unverified
- `local-advisory` and `remote-advisory` are operator-facing run modes, not proof of trust
- process-scoped conclusions in this mode should be read as advisory

If you are implementing your own engine instead of using the sibling `etragon`
repo, see:

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)

## Reading Failure Semantics

The runtime now separates coarse failure mode from finer failure detail.

Typical pairs:

- `server_denied + access_denied`
- `server_denied + auth_required`
- `semantic_error + protocol_error`
- `semantic_error + protocol_constraint_violation`
- `no_response + request_sent_no_reply`
- `setup_incomplete + handshake_incomplete`
- `not_sent + followup_not_sent`

This distinction matters:

- `failure_mode` is the coarse operational category
- `failure_detail` is the more specific diagnosis
- `failure_basis` explains whether that diagnosis came from direct protocol
  evidence, a missing transition, or a weaker phase-level inference

## Interpreting Confidence Safely

Treat the confidence values this way:

- `high`
  safe to read as a strong protocol-backed result
- `medium`
  usually reliable enough for operator guidance, but still inferred from
  missing transitions
- `low`
  useful as a lead, but should be treated as hypothesis-level guidance rather
  than a final verdict

If multiple module kinds or multiple missing transitions compete inside one
process profile, the runtime will intentionally reduce confidence instead of
pretending certainty.

The report will now also say this explicitly:

- `ambiguous=true`
- `competing_hypotheses=[...]`

That is the runtime telling you there is more than one plausible network-module
story for the same process.

## Suggested Learning Order

1. Run one single-protocol `--summary-only --json` command.
2. Run the same target with `--report-format html`.
3. Run `--scan-all --pid ...`.
4. Compare `protocol_flows` with `process_network_profiles`.
5. Only then move down into the underlying `.gewy` path files under
   [dsl](/Users/Shared/chroot/dev/gewyvern/dsl).

If your main goal is process-oriented diagnosis rather than command syntax, the
next best companion guide is
[docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md).

If your main goal is to understand local-advisory vs remote-advisory ingest,
trust labels, and why PID attribution is deliberately downgraded for socket
inputs, the best companion guide is
[docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md).

If your main goal is to understand what report language like
`server_denied`, `request_sent_no_reply`, or `followup_not_sent` means across
different protocol clusters, the best companion guide is
[docs/failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/failure-semantics.md).
