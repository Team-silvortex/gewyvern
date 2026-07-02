# Release Checklist

This page is the shortest practical release checklist for the active
`0.19.x` line.

Use it when the question is not "how does packaging work?" or "what does field
validation mean?", but simply:

- can we still ship this line with confidence?
- did we exercise the real packaged artifacts?
- did standalone and multi-project paths both survive?

For deeper background, see:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/field-findings.md](docs/field-findings.md)
- [docs/packaging.md](docs/packaging.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)

## Role In The Shelf

Treat this page as the shortest practical release gate.

Use it when the question is:

- can we call this line green today?
- which exact packaged checks must pass before shipping?
- what is the fastest narrowing path when one release-phase check fails?

Do not use this page as:

- the full validation philosophy for the line
- the durable statement of what the current minor line is supposed to mean
- the evidence log of what already passed over time

For those, use:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/field-findings.md](docs/field-findings.md)

## Companion Shelves

- [docs/field-validation.md](docs/field-validation.md)
  for the broader validation program and scenario bands
- [docs/field-findings.md](docs/field-findings.md)
  for the short record of what has already been demonstrated
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
  for the current line's intended debugger-integration posture
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)
  for the protocol-breadth and physical-host validation baseline that this line
  inherits

## Current `0.19.x` Gate

Treat the line as release-ready only when all of the following stay true:

1. current native artifacts are rebuilt from the current source tree
2. packaged `deb` and `rpm` install smoke both pass
3. packaged standalone runtime validation both pass
4. packaged protocol validation both pass
5. packaged operator-path validation both pass
6. runtime validation still proves the training dataset/export roundtrip
7. lifecycle validation proves startup, stop, log evidence, recovery, and cleanup
8. the default `deb+rpm` release wrapper passes as one routine
9. the three-module Docker stack smoke still passes
10. pathological container/runtime-ingest validation still proves bad clients do
    not wedge the runtime
11. debugger cross-validation still compares runtime summary JSON, debugger
    console JSON, and `gewyc` envelope JSON without overclaiming negative cases
12. security dependency checks stay clean for Rust, .NET, and frontend package
    manifests

This section is intentionally binary and operational. It should stay shorter
and stricter than the broader validation note.

## Rebuild Current Artifacts

Always rebuild the native packages before calling the release path green:

```bash
bash scripts/packaging/build_packages_in_container.sh --format all
```

Expected outputs:

- `target/packages/gewyvern_<version>-1_<deb-arch>.deb`
- `target/packages/rpm/gewyvern-<version>-1.<rpm-arch>.rpm`

The `<version>` value is read from the root `gewyvern` package metadata in
`Cargo.toml`. For the current tree, that resolves to `0.19.0`, so the
concrete artifact names should look like `gewyvern_0.19.0-1_<deb-arch>.deb`
and `gewyvern-0.19.0-1.<rpm-arch>.rpm`.

The release-line posture can move ahead of that metadata, but the package
smoke must always verify the artifacts that the tree actually builds today.

Do not trust an older green run if it was using stale artifacts from another
version line.

## Fastest Release Check

The shortest one-command gate is:

```bash
bash scripts/packaging/release_gate.sh
```

That sequence rebuilds current native artifacts, runs the packaged release
validation wrapper, runs the three-module stack smoke, and then runs the
pathological container/runtime-ingest validation.

If you want to skip one phase while narrowing a failure, use:

```bash
bash scripts/packaging/release_gate.sh --skip-build
bash scripts/packaging/release_gate.sh --skip-stack
bash scripts/packaging/release_gate.sh --skip-pathology
bash scripts/packaging/release_gate.sh --deb
bash scripts/packaging/release_gate.sh --rpm
```

The lower-level packaged release-minded entrypoint is:

```bash
bash scripts/packaging/release_container_check.sh
```

This must pass in default `deb+rpm` mode.

It covers:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

The packaged runtime validation now also confirms the machine-facing training
surface stays internally consistent:

- `/v1/latest/training-dataset.json` remains fetchable
- each sample row points to a usable `training-example.json`
- manifest `sample_id` values match the fetched sample payloads
- the default split policy remains `name_bucket_mod_10`

If you are narrowing a failure, these subchecks may be run independently:

```bash
bash scripts/packaging/package_install_smoke.sh
bash scripts/packaging/container_runtime_validation.sh
bash scripts/packaging/container_protocol_validation.sh
bash scripts/packaging/container_operator_path_validation.sh
cargo run --quiet --bin gewyvern_validate -- debugger-cross
cargo audit
```

## Expected Packaged Semantics

The release check is not only checking process exit codes. It is also asserting
current behavior that should remain stable enough for this line:

- `http request` stays `manual_review`
- `tls client` stays `manual_review`
- `quic initial` stays `collect_more_runtime_evidence`
- `http3 request` stays
  `operator_guidance_action = "safe_to_escalate_protocol_signal"`
- packaged malformed ingest does not kill the `--serve` loop
- packaged training dataset roundtrip still verifies stable sample identity

If one of these changes, treat it as a deliberate semantics review, not just a
test refresh chore.

## Multi-Project Integration Gate

After the single-project packaged path is green, run:

```bash
bash scripts/validation/three_module_stack_smoke.sh
```

On physical validation hosts with an already-built stack image, this equivalent
form avoids rebuilding the Docker toolchain while still refreshing leserpent's
NuGet packages before using `--no-restore`:

```bash
IMAGE_TAG=gewyvern-stack-dev-physical \
  SKIP_DOCKER_BUILD=true \
  LESERPENT_DOTNET_RESTORE_FIRST=true \
  LESERPENT_DOTNET_IGNORE_FAILED_SOURCES=true \
  LESERPENT_DOTNET_NO_RESTORE=true \
  bash scripts/validation/three_module_stack_smoke.sh
```

That smoke should confirm:

- `etragon-status-ok`
- `etragon-output-ok`
- `summary-ok`
- `runtimes-ok`
- `three-module stack smoke: ok`
- one `resilience_summary=...` artifact path worth archiving with the current
  line's review notes when collaboration posture is relevant

This is the highest-signal collaboration check for the current line because it
exercises:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

## Pathological Container Runtime Gate

After normal stack confidence is green, run:

```bash
bash scripts/validation/pathological_container_validation.sh
```

That gate drives intentionally bad clients against the runtime ingest surface:

- truncated JSON
- empty disconnects
- slow-drip incomplete JSON
- oversized fact lines

The expected result is not "nothing bad happened"; it is more precise:

- the runtime stays reachable after bad input
- health and resilience surfaces report degraded/backing-off state
- post-fault analysis still returns a coherent runtime payload
- log evidence records the ingest failures without turning them into process
  death

## If Something Fails

Use this triage order:

1. if package install smoke fails, inspect packaging/layout first
2. if runtime validation fails, inspect `--serve`, socket ingest, API lifecycle, or packaged assets
3. if protocol validation fails, compare current JSON semantics against the scripted expectation
4. if operator-path validation fails, check whether the runtime drifted or the expected guidance drifted
5. if three-module smoke fails, inspect cross-project API contracts before changing single-project diagnosis logic
6. if pathological validation fails, inspect socket ingest resilience and bad-client log evidence before changing protocol diagnosis logic

## Ship Read

For the active `0.19.x` line, a good practical ship read is:

- current artifacts rebuilt
- `release_gate.sh` green, or the equivalent build + packaged release check +
  three-module smoke sequence green
- full `release_container_check.sh` green in default mode
- `three_module_stack_smoke.sh` green
- `pathological_container_validation.sh` green on a Docker-capable host
- `gewyvern_validate -- debugger-cross` green
- Rust/.NET/frontend dependency vulnerability checks green
- no new drift in `field-findings` that would downgrade trust in conservative diagnosis

If all of these are true, the line is in a healthy release posture.
