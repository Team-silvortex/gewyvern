# How-To: Validate The Current Runtime Surface

Use this guide when the question is:

- can the current `gewyvern` runtime still be trusted today?
- which validation commands should I run first?
- how do I distinguish compiler drift from runtime drift?

This page is task-first. It is not trying to explain every subsystem.

For the broader validation philosophy, see
[docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md).
For the current running record of what has already passed, see
[docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md).

## When To Use This Guide

Use this guide when you are:

- checking whether a checkout is still healthy
- preparing a `v0.13.0` judgment call
- validating a branch after runtime, report, or DSL changes
- trying to narrow "what broke?" before reading code

## The Short Validation Ladder

Run the checks in this order:

1. workspace tests
2. compiler-facing `gewyc` surface
3. focused runtime smoke
4. registry/package sweep
5. high-frequency protocol validation
6. packaged/container validation when release confidence matters

That order matters because it helps you isolate where drift entered.

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
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
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

## Step 4: Run The Registry Shelf, Not Just One Target

Now ask whether the scanned built-in package shelf still holds together:

```bash
cargo run -- --list-protocols
cargo run -- --scan-all --json --summary-only
bash /Users/Shared/chroot/dev/gewyvern/scripts/registry_validation.sh
```

Why all three matter:

- `--list-protocols` confirms the registry is still discoverable
- `--scan-all` confirms broad runtime target enumeration still works
- `registry_validation.sh` tells you which exact package drifted and whether it
  is a parse, validation, diagnostics, or JSON-shape failure

This is usually the fastest way to answer:

- did a protocol package drift?
- did a DSL/package boundary drift?
- did the registry scanner break?

## Step 5: Exercise The High-Frequency Shelf

For the current pre-`1.0` line, the most valuable operator surface is the
high-frequency protocol shelf.

Run:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/high_frequency_validation.sh
```

This is where we keep pressure on:

- `HTTP / HTTPS / TLS`
- `DNS`
- `SSH`
- `SOCKS5 / proxy`
- `MySQL / PostgreSQL`
- `QUIC / HTTP/3`

If this fails while the broad registry sweep still passes, the problem is
probably not "the whole runtime is broken". It is more likely:

- one important protocol path drifted
- a mixed-flow expectation changed
- a built-in guidance expectation moved

## Step 6: Use Container Checks When Confidence Really Matters

When you are judging release confidence or cross-environment behavior, use the
container line as well:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_container_check.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/three_module_stack_smoke.sh
```

These answer different questions:

- `release_container_check.sh`
  asks whether packaged Linux install/runtime/protocol/operator-path behavior is
  still healthy
- `three_module_stack_smoke.sh`
  asks whether the current `gewyvern + etragon + leserpent` topology still
  works in Docker

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

Treat this as protocol shelf drift until proven otherwise.

### If `high_frequency_validation.sh` fails

Look first at:

- the exact protocol path
- mixed-flow expectations
- `operator_guidance_action`
- any recent diagnosis/report policy change

### If container checks fail but local checks pass

Look first at:

- packaged asset resolution
- runtime bind/exposure assumptions
- service/API path behavior

Treat this as environment or packaging drift, not necessarily a core diagnosis
failure.

## Step 7: Validate The Serve/API And External-Engine Bridge

When the question is not only "does the runtime compile?" but also "can other
local tools safely consume it?", validate the serve/API chain directly.

For a local socket ingest plus API surface:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/targets
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
```

If you also want to smoke the generic external-engine bridge end to end:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json
```

To target one specific route, pass a path segment as the sixth argument:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json socket_session
```

Use this when you need confidence in:

- `summary.json` versus `analysis.json`
- target route discovery through `/v1/latest/targets`
- local sidecar/enrich chains rather than just CLI rendering

## What “Healthy Enough For v0.13.0” Means

For the current line, the runtime surface is in a good state when:

- the workspace tests are green
- `gewyc frontend` and `gewyc explain` still work on built-in DSL files
- focused runtime JSON still exposes the diagnosis spine coherently
- registry validation still passes
- the high-frequency shelf still passes
- release/container checks still pass when you need stronger confidence

That is enough to say:

- the project is not finished forever
- but it is already usable on purpose

For the release posture around that judgment, see
[docs/v0.13-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.13-posture.md).
