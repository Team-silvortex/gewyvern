# How-To: Wire etragon As A Nearby Sidecar

Use this guide when the question is:

- how do I connect `etragon` to one local `gewyvern` runtime?
- how do I prove the sidecar bridge is really working?
- how do I distinguish API/runtime drift from sidecar drift?

This guide is task-first.
It assumes you already understand the broad stack shape.

For the collaboration boundary itself, see:

- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)

## Book Path

This chapter belongs to the Collaborate And Package band of the how-to volume.

Read it when the task is no longer “what is a sidecar?” but:

- how do I attach one nearby?
- what commands prove it is alive?
- what outputs should I inspect first?

Then continue with:

- [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
- [docs/book/explanation-stack-topology.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-stack-topology.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

## What “Nearby Sidecar” Means Here

The intended relationship is:

```text
etragon <-> one nearby gewyvern
leserpent -> many gewyvern instances
```

`etragon` is not the base diagnosis owner.
It is an additive nearby engine that can return:

- `augmentations`
- `evidence_chain_enrichment`
- `diagnostic_opinion`

`gewyvern` remains authoritative for the built-in diagnosis spine.

## Choose The Right Validation Path

There are two practical ways to validate the bridge.

### Path A: Inline Engine Hook

Use this when you want to prove:

- `gewyvern` can invoke `etragon`
- sidecar output is merged back into local JSON surfaces
- the serve/API path stays healthy with the sidecar attached

### Path B: Roundtrip Or Full Stack Script

Use this when you want to prove:

- the API payload is consumable by a sibling engine
- the returned engine output looks sane
- the broader `gewyvern + etragon + leserpent` topology still works

## Path A: Attach etragon Through `--external-engine-bin`

Start one local runtime with API enabled:

```bash
cargo run -- \
  --scan-all \
  --tcp-socket 127.0.0.1:9000 \
  --ingest-mode local-advisory \
  --serve \
  --api-socket 127.0.0.1:9100 \
  --json \
  --summary-only \
  --external-engine-bin /Users/Shared/chroot/dev/etragon/target/debug/etragon
```

If you want the Python-backed worker path instead of the engine's default Rust
pass:

```bash
cargo run -- \
  --scan-all \
  --tcp-socket 127.0.0.1:9000 \
  --ingest-mode local-advisory \
  --serve \
  --api-socket 127.0.0.1:9100 \
  --json \
  --summary-only \
  --external-engine-bin /Users/Shared/chroot/dev/etragon/target/debug/etragon \
  --external-engine-worker /Users/Shared/chroot/dev/etragon/scripts/python_baseline_worker.py
```

## Step 1: Confirm The API Is Alive First

Before blaming the sidecar, confirm the runtime itself is healthy:

```bash
curl http://127.0.0.1:9100/health
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/targets
```

If these fail, stop there first.
That is runtime/API drift, not sidecar drift.

## Step 2: Inspect The Latest Sidecar-Aware Surfaces

These are the highest-value routes to read first:

```bash
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
curl http://127.0.0.1:9100/v1/latest/findings.json
```

Look for:

- `external_sidecar_context`
- `has_external_sidecar_context`
- `has_external_evidence_chain_enrichment`
- `has_external_diagnostic_opinion`
- `external_sidecar_trust_level`
- `external_sidecar_consumption_mode`

These fields are the fastest way to see whether the sidecar is present only as
append-only context, or whether the bridge has enough declared capability to be
treated as richer nearby guidance context.

## Step 3: Check The Base Diagnosis Did Not Move Illegitimately

The most important sanity check is negative:

- `primary_failure_*` should still be owned by `gewyvern`
- `operator_guidance_*` should not be silently rewritten by the sidecar

If sidecar presence appears to replace the core diagnosis spine instead of
augmenting it, treat that as a real bug.

## Path B: Use The External-Engine Roundtrip Demo

When you want one direct end-to-end proof that:

- `gewyvern` publishes analysis JSON
- `etragon` can consume it
- engine output is emitted back as JSON

run:

```bash
bash scripts/external_engine_roundtrip_demo.sh \
  127.0.0.1:9900 \
  127.0.0.1:9910 \
  udp \
  /tmp/gewyvern-analysis.json \
  /tmp/external-engine-augmentations.json
```

By default this looks for a sibling `../etragon` repo and runs:

```bash
cargo run -- analyze-url
```

inside that engine root.

If you need to point at a different engine checkout or command:

```bash
ENGINE_ROOT=/path/to/external-engine
EXTERNAL_ENGINE_CMD='cargo run -- analyze-url'
```

## What Success Looks Like

A healthy roundtrip gives you:

- one saved analysis snapshot
- one saved engine output payload
- a returned JSON object from the engine on stdout

Start by opening:

- `/tmp/gewyvern-analysis.json`
- `/tmp/external-engine-augmentations.json`

The first proves the runtime published a usable machine-facing snapshot.
The second proves the engine returned append-only augmentation content.

## Use Target-Specific Analysis When Needed

If you need one concrete target instead of the latest top-level analysis
snapshot, pass the URL-safe target path segment as the sixth argument:

```bash
bash scripts/external_engine_roundtrip_demo.sh \
  127.0.0.1:9900 \
  127.0.0.1:9910 \
  udp \
  /tmp/gewyvern-analysis.json \
  /tmp/external-engine-augmentations.json \
  socket_session
```

This is useful when one scan exposes many targets and you want to validate the
bridge against a specific one.

## Full Stack Validation

When you want stronger evidence that the whole collaboration topology still
works, run:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/three_module_stack_smoke.sh
```

This is the high-signal check for:

- one nearby `etragon` sidecar
- multiple `gewyvern` runtimes
- one `leserpent` control plane

This is heavier than the local roundtrip demo, but it is the right check when
you care about stack-level confidence instead of only one local bridge.

## How To Triage A Failure

Use this split:

### If `/health` and `/v1/capabilities` fail

Treat this as base runtime/API drift.

Look first at:

- `--serve`
- `--api-socket`
- local bind/exposure assumptions

### If the API is healthy but no `external_sidecar_context` appears

Treat this as bridge or engine-hook drift.

Look first at:

- `--external-engine-bin`
- `--external-engine-worker`
- engine process startup
- engine stdout JSON validity

### If sidecar fields appear, but `external_sidecar_trust_level` is low

Treat this as capability-handshake or compatibility drift.

Look first at:

- `<external-engine-bin> protocol-capabilities`
- capability profile content
- published context declarations
- version compatibility rules

### If sidecar output seems to overwrite built-in diagnosis

Treat this as a merge-posture bug.

The intended model is additive-only.

### If roundtrip works but `three_module_stack_smoke.sh` fails

Treat this as broader orchestration or Docker-stack drift, not a narrow local
bridge failure.

## Practical Success Checklist

Before calling the nearby sidecar path healthy, confirm:

1. base API health passes
2. `analysis.json` is published
3. sidecar-aware fields appear when the engine is enabled
4. core `primary_failure_*` fields remain owned by `gewyvern`
5. engine output stays append-only
6. stack smoke passes when release confidence matters

## Where To Go Next

- For the exact process-boundary and payload contract:
  [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
- For the additive-only merge posture:
  [docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md)
- For the broader three-layer system view:
  [docs/book/explanation-stack-topology.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-stack-topology.md)
