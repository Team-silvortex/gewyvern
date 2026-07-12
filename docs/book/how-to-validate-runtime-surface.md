# How-To: Validate The Current Runtime Surface

Use this guide when the question is:

- can the current `gewyvern` runtime still be trusted today?
- which validation commands should I run first?
- how do I distinguish compiler drift from runtime drift?

This page is task-first. It is not trying to explain every subsystem.

For the broader validation philosophy, see
[docs/field-validation.md](docs/field-validation.md).
For the current running record of what has already passed, see
[docs/field-findings.md](docs/field-findings.md).

## Book Path

This chapter belongs to the Validate band of the how-to volume.

Read it when you already know the system roughly and need the shortest path
to answering:

- is this checkout still healthy?
- where did drift likely enter?
- what confidence do I actually have right now?

Then continue with:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/field-findings.md](docs/field-findings.md)
- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## When To Use This Guide

Use this guide when you are:

- checking whether a checkout is still healthy
- preparing a `v1.0.0` release judgment call
- validating a branch after runtime, report, or DSL changes
- trying to narrow "what broke?" before reading code

## The Short Validation Ladder

Run the checks in this order:

1. workspace tests
2. compiler-facing `gewyc` surface
3. focused runtime smoke
4. debugger cross/negative validation
5. registry/package sweep
6. high-frequency protocol validation
7. packaged/container validation when release confidence matters

That order matters because it helps you isolate where drift entered.

The naming split in the referenced scripts is deliberate:

- `smoke` is the lightest check
- `roundtrip` is one narrow consumer path
- `validation` is one grouped stability check
- `summary` is a wrapper over multiple validations

## Step 1: Start With The Whole Workspace

```bash
cargo test --workspace --quiet
```

What this tells you:

- core runtime scenarios still pass
- compiler-facing surfaces still pass
- report/API/export regressions are caught early

What not to overreact to:

- the two local socket bind tests may remain `ignored` in restricted
  environments

If this fails, stop here first. There is no point treating packaged runtime or
protocol drift as the first suspect when the main test suite is already red.

## Step 2: Confirm The Compiler Shell Still Looks Healthy

Use `gewyc` directly on one built-in DSL file:

```bash
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- explain dsl/http_request_path.gewy --json
```

What this confirms:

- pipeline/package parsing still works
- `include(...)` and `use(...)` relationships still resolve
- diagnostics/findings surfaces still render

If these fail while the broad runtime tests still pass, the problem is likely
in `gewylang` or `gewyc`, not in the operator-facing runtime shell.

## Step 3: Run One Focused Runtime Path

Pick one stable entry and ask for the narrow diagnosis spine:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
```

Read these first:

- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `operator_guidance_action`

This is the fastest way to confirm that the runtime still produces a coherent
single-target explanation.

If you want a human-oriented surface as well:

```bash
cargo run -- --protocol http3 --entry request --report-format html --out /tmp/http3-request.html
```

## Step 4: Cross-Validate Debugger Surfaces

Before broad protocol sweeps, prove that the same scenario reads consistently
through multiple debugger surfaces:

```bash
cargo run --quiet --bin gewyvern_validate -- debugger-cross
```

This check compares:

- runtime summary JSON
- local debugger-console JSON
- local debug-session JSON, its `debugger_posture`, and local `command` hints
- `gewyc` envelope JSON

The harness also writes `evidence-index.json` next to the raw case outputs. Use
that file first when you want a compact map of which case produced which
posture, `debugger_route`, guidance action, missing transition, and
compiler-envelope status. Only open the raw JSON files after the index points
you at the suspicious case.

It also runs negative cases. The protocol negatives are valid inputs with
missing evidence, so they must stay in `attention` /
`collect_more_runtime_evidence` posture, while their `debugger_posture` must
stay in `needs_evidence` rather than pretending the next action is already
safe. The toolchain negative is invalid Gewylang input, so parse must fail
before validation or diagnostics can claim success.

The legacy
`scripts/validation/debugger_cross_validation.sh`
script remains available for older automation, but it delegates to the native
Rust harness instead of owning the assertions itself.

Use this when the question is not just "does it run?" but "can it actually
debug without overclaiming?"

When the runtime API is serving, prefer `/v1/latest/debug-session.json` as the
operator-facing starting point. It preserves the recommended focus from the
debugger console, then adds the target links, failure spine, protocol-reading
path, and next-step hints needed to continue the investigation without hunting
through several endpoints first. Its `debugger_posture` object is the compact
read: whether the target is healthy, ready to escalate, still missing evidence,
or still ambiguous enough to need hypothesis review. Its `debugger_route`
object is the UI-friendly route: the primary surface to open first, a fallback
surface, and whether escalation is currently allowed.

When you are staying in the local CLI instead of the API, `--debug-session
--json` now carries matching `command` hints on `debugger_route` and
`next_steps`, so the shell-facing next move is explicit instead of implied.
The API surface keeps the same route/step idea but uses `path` fields instead.

## Step 5: Run The Registry Shelf, Not Just One Target

Now ask whether the scanned built-in package shelf still holds together:

```bash
cargo run -- --list-protocols
cargo run -- --scan-all --json --summary-only
cargo run --quiet --bin gewyvern_validate -- registry
```

Why all three matter:

- `--list-protocols` confirms the registry is still discoverable
- `--scan-all` confirms broad runtime target enumeration still works
- `gewyvern_validate registry` tells you which exact package drifted and
  whether it is a parse, validation, diagnostics, or JSON-shape failure

This is usually the fastest way to answer:

- did a protocol package drift?
- did a DSL/package boundary drift?
- did the registry scanner break?

## Step 6: Exercise The High-Frequency Shelf

For the active `1.0.0` line, the most valuable operator surface is the
high-frequency protocol shelf plus the debugger cross-validation path.

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- high-frequency
```

This is where we keep pressure on:

- `HTTP / HTTPS / TLS`
- `DNS`
- `SSH`
- `SOCKS5 / proxy`
- `MySQL / PostgreSQL`
- `QUIC / HTTP/3`

Pair it with `debugger-cross` before release judgment so the broad shelf still
funnels into one coherent diagnosis story:

```bash
cargo run --quiet --bin gewyvern_validate -- debugger-cross
```

If this fails while the broad registry sweep still passes, the problem is
probably not "the whole runtime is broken". It is more likely:

- one important protocol path drifted
- a mixed-flow expectation changed
- a built-in guidance expectation moved

## Step 7: Use Container Checks When Confidence Really Matters

When you are judging release confidence or cross-environment behavior, use the
container line as well:

```bash
cargo run --quiet --bin gewyvern_validate -- release-container-check --deb
bash scripts/validation/three_module_stack_smoke.sh
```

These answer different questions:

- `release_container_check.sh`
- `gewyvern_validate release-container-check`
  asks whether packaged Linux install/runtime/protocol/operator-path behavior is
  still healthy
- `three_module_stack_smoke.sh`
  asks whether the current `gewyvern + etragon + leserpent` topology still
  works in Docker

When you want one stronger Linux-only target-side read without claiming that
`gewyvern` is already a web-vulnerability scanner, add:

```bash
sudo cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation
```

That practical target-lab path preserves suspicious HTTP evidence from a real
Docker target and then proves the same host can still execute attach, kprobe,
and tc smoke. Treat it as a high-signal release-confidence check for Linux/BPF
reliability, not as direct vulnerability classification.

Use them when:

- you are preparing a release judgment
- local sandbox restrictions make socket/runtime behavior ambiguous
- you need evidence stronger than "unit tests are green"

## How To Triage A Failure

Use this simple split:

### If `cargo test --workspace` fails

Look first at:

- the failing module or scenario test
- recent runtime/report/export changes

Treat this as a core correctness issue, not a packaging problem.

### If `gewyc frontend` or `gewyc explain` fails

Look first at:

- `src/dsl.rs`
- `src/dsl/`
- `src/gewyc/`

Treat this as compiler/DSL drift.

### If `registry_validation.sh` fails

Look first at:

- the specific package path it reports
- the failure class:
  - command
  - parse
  - validation
  - diagnostics
  - JSON shape

The legacy
`scripts/validation/registry_validation.sh`
script remains available for older automation, but it delegates to
`gewyvern_validate registry`.

Treat this as protocol shelf drift until proven otherwise.

### If `high_frequency_validation.sh` fails

Look first at:

- the exact protocol path
- mixed-flow expectations
- `operator_guidance_action`
- any recent diagnosis/report policy change

The legacy
`scripts/validation/high_frequency_validation.sh`
script remains available for older automation, but it delegates to
`gewyvern_validate high-frequency`.

### If `debugger_cross_validation.sh` fails

Look first at:

- whether summary JSON and debugger-console JSON disagree
- whether `gewyc envelope` no longer reports parse, validation, and diagnostics
  consistently
- whether a negative protocol case stopped producing `missing_transition`
- whether a negative case started recommending action instead of more evidence

### If container checks fail but local checks pass

Look first at:

- packaged asset resolution
- runtime bind/exposure assumptions
- service/API path behavior

Treat this as environment or packaging drift, not necessarily a core diagnosis
failure.

## Step 8: Validate The Serve/API And External-Engine Bridge

When the question is not only "does the runtime compile?" but also "can other
local tools safely consume it?", validate the serve/API chain directly.

For controlled lifecycle coverage:

```bash
cargo run --quiet --bin gewyvern_validate -- field-smoke --socket --scan-all
cargo run --quiet --bin gewyvern_validate -- socket-roundtrip
cargo run --quiet --bin gewyvern_validate -- runtime-operator
cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle
cargo run --quiet --bin gewyvern_validate -- resilience-roundtrip
cargo run --quiet --bin gewyvern_validate -- resilience-log-evidence --log-source target/validation/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-bundle --api-addr 127.0.0.1:9910 --log-source target/validation/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-emit-helper --mode fail --output /tmp/gewyvern-external-fail.sh
cargo run --quiet --bin gewyvern_validate -- resilience-drive-bad-json --host 127.0.0.1 --port 9909 --count 6
```

The native field smoke checks demo summary, DSL summary, `gewyc explain`, Unix
socket roundtrip, and `--scan-all`. The native runtime operator check exercises
TCP and UDP serve sessions, latest summary/export/analysis API readability,
malformed ingest recovery, and training dataset sample roundtrip. The native
lifecycle check starts local runtime processes, verifies bounded shutdown,
confirms malformed socket input degrades and then recovers, checks log evidence,
and proves API/socket reachability is gone after explicit stop. The native
resilience commands prepare the recovery runbook and extract log evidence
without requiring `rg`, `grep`, `curl`, or `python3`. The native fault-injection
helpers generate external-engine probes and drive malformed socket payloads
without requiring `nc`. The legacy
`scripts/validation/field_validation_smoke.sh` and
`scripts/validation/runtime_operator_validation.sh` and
`scripts/validation/runtime_lifecycle_validation.sh` entrypoints remain available
for older automation.

For a local socket ingest plus API surface:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/targets
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
curl http://127.0.0.1:9100/v1/latest/training-dataset.json
```

If you also want to smoke the external-engine bridge roundtrip end to end:

```bash
cargo run --quiet --bin gewyvern_validate -- external-engine-roundtrip --ingest-addr 127.0.0.1:9900 --api-addr 127.0.0.1:9910 --template udp --analysis-out /tmp/gewyvern-analysis.json --engine-out /tmp/external-engine-augmentations.json
```

If you want to confirm the training-dataset consumer roundtrip the way a
sibling engine would actually use it, run:

```bash
cargo run --quiet --bin gewyvern_validate -- training-roundtrip --api-addr 127.0.0.1:9100 --out-dir /tmp/gewyvern-training-roundtrip
```

That consumer roundtrip checks three things that are easy to miss in narrower
API smoke:

- the manifest itself is available
- each manifest sample row resolves to a real training example payload
- `sample_id` stays identical between the manifest row and the fetched sample

To target one specific route, pass a path segment as the sixth argument:

```bash
cargo run --quiet --bin gewyvern_validate -- external-engine-roundtrip --ingest-addr 127.0.0.1:9900 --api-addr 127.0.0.1:9910 --template udp --analysis-out /tmp/gewyvern-analysis.json --engine-out /tmp/external-engine-augmentations.json --target-path-segment socket_session
```

Use this when you need confidence in:

- `summary.json` versus `analysis.json`
- target route discovery through `/v1/latest/targets`
- local sidecar/enrich chains rather than just CLI rendering
- training manifests versus fetched sample payloads

## What “Healthy Enough For v1.0.0” Means

For the current line, the runtime surface is in a good state when:

- the workspace tests are green
- `gewyc frontend` and `gewyc explain` still work on built-in DSL files
- focused runtime JSON still exposes the diagnosis spine coherently
- registry validation still passes
- the high-frequency shelf still passes
- debugger cross-validation still proves runtime, console, and compiler
  envelopes agree without overclaiming negative cases
- runtime lifecycle validation still proves start, stop, recovery, and cleanup
- release/container checks still pass when you need stronger confidence
- the practical Linux target-lab shelf preserves suspicious target evidence
  without losing same-host eBPF attach proof
- Rust/.NET/frontend dependency checks stay clean when preparing a release

That is enough to say:

- the project is not finished forever
- but it is already usable on purpose

For the release posture around that judgment, see
[docs/history/v0.20.x.md](docs/history/v0.20.x.md) and
[docs/release-checklist.md](docs/release-checklist.md).
