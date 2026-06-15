# Release Checklist

This page is the shortest practical release checklist for the current
`0.14.x` line.

Use it when the question is not "how does packaging work?" or "what does field
validation mean?", but simply:

- can we still ship this line with confidence?
- did we exercise the real packaged artifacts?
- did standalone and multi-project paths both survive?

For deeper background, see:

- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)

## Role In The Shelf

Treat this page as the shortest practical release gate.

Use it when the question is:

- can we call this line green today?
- which exact packaged checks must pass before shipping?
- what is the fastest narrowing path when one release-phase check fails?

Do not use this page as:

- the full validation philosophy for the line
- the durable statement of what `v0.14.0` is supposed to mean
- the evidence log of what already passed over time

For those, use:

- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)

## Companion Shelves

- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
  for the broader validation program and scenario bands
- [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md)
  for the short record of what has already been demonstrated
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
  for the current line's intended product and documentation posture

## Current `0.14.x` Gate

Treat the line as release-ready only when all of the following stay true:

1. current native artifacts are rebuilt from the current source tree
2. packaged `deb` and `rpm` install smoke both pass
3. packaged standalone runtime validation both pass
4. packaged protocol validation both pass
5. packaged operator-path validation both pass
6. runtime validation still proves the training dataset/export roundtrip
7. the default `deb+rpm` release wrapper passes as one routine
8. the three-module Docker stack smoke still passes

This section is intentionally binary and operational. It should stay shorter
and stricter than the broader validation note.

## Rebuild Current Artifacts

Always rebuild the native packages before calling the release path green:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages_in_container.sh --format all
```

Expected outputs:

- `/Users/Shared/chroot/dev/gewyvern/target/packages/gewyvern_0.14.0-1_arm64.deb`
- `/Users/Shared/chroot/dev/gewyvern/target/packages/rpm/gewyvern-0.14.0-1.aarch64.rpm`

Do not trust an older green run if it was using stale artifacts from another
version line.

## Fastest Release Check

The shortest one-command gate is:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh
```

That sequence rebuilds current native artifacts, runs the packaged release
validation wrapper, and then runs the three-module stack smoke.

If you want to skip one phase while narrowing a failure, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh --skip-build
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh --skip-stack
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh --rpm
```

The lower-level packaged release-minded entrypoint is:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh
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
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_protocol_validation.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_operator_path_validation.sh
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
bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh
```

That smoke should confirm:

- `etragon-status-ok`
- `etragon-output-ok`
- `summary-ok`
- `runtimes-ok`
- `three-module stack smoke: ok`

This is the highest-signal collaboration check for the current line because it
exercises:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

## If Something Fails

Use this triage order:

1. if package install smoke fails, inspect packaging/layout first
2. if runtime validation fails, inspect `--serve`, socket ingest, or packaged assets
3. if protocol validation fails, compare current JSON semantics against the scripted expectation
4. if operator-path validation fails, check whether the runtime drifted or the expected guidance drifted
5. if three-module smoke fails, inspect cross-project API contracts before changing single-project diagnosis logic

## Ship Read

For the current `0.14.x` line, a good practical ship read is:

- current artifacts rebuilt
- `release_gate.sh` green, or the equivalent build + packaged release check +
  three-module smoke sequence green
- full `release_container_check.sh` green in default mode
- `three_module_stack_smoke.sh` green
- no new drift in `field-findings` that would downgrade trust in conservative diagnosis

If all four are true, the line is in a healthy release posture.
