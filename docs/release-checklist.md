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
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)

## Current `0.14.x` Gate

Treat the line as release-ready only when all of the following stay true:

1. current native artifacts are rebuilt from the current source tree
2. packaged `deb` and `rpm` install smoke both pass
3. packaged standalone runtime validation both pass
4. packaged protocol validation both pass
5. packaged operator-path validation both pass
6. the default `deb+rpm` release wrapper passes as one routine
7. the three-module Docker stack smoke still passes

## Rebuild Current Artifacts

Always rebuild the native packages before calling the release path green:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages_in_container.sh --format all
```

Expected outputs:

- `/Users/Shared/chroot/dev/gewyvern/target/packages/gewyvern_0.14.0-1_arm64.deb`
- `/Users/Shared/chroot/dev/gewyvern/target/packages/rpm/gewyvern-0.14.0-1.aarch64.rpm`

Do not trust an older green run if it was using stale artifacts from another
version line.

## Fastest Release Check

The shortest one-command gate is:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_gate.sh
```

That sequence rebuilds current native artifacts, runs the packaged release
validation wrapper, and then runs the three-module stack smoke.

If you want to skip one phase while narrowing a failure, use:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_gate.sh --skip-build
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_gate.sh --skip-stack
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_gate.sh --deb
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_gate.sh --rpm
```

The lower-level packaged release-minded entrypoint is:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/release_container_check.sh
```

This must pass in default `deb+rpm` mode.

It covers:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

If you are narrowing a failure, these subchecks may be run independently:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/package_install_smoke.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/container_runtime_validation.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/container_protocol_validation.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/container_operator_path_validation.sh
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

If one of these changes, treat it as a deliberate semantics review, not just a
test refresh chore.

## Multi-Project Integration Gate

After the single-project packaged path is green, run:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/three_module_stack_smoke.sh
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
