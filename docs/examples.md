# Examples Guide

This guide is the shortest path from "I have the repo" to "I can use
`gewyvern` to answer a real debugging question".

The examples below focus on the operator-facing runtime, not the compiler.

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
cargo run -- --scan-all --pid 4242 --tcp-socket 127.0.0.1:9000 --summary-only --report-format html --out /tmp/process-scan.html
```

Pay attention to:

- `ingest_trust_mode`
- `primary_failure_confidence`
- `primary_failure_basis`

That combination tells you both what the runtime thinks and how directly the
evidence supports that conclusion.

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
