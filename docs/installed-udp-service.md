# Installed UDP Service

An installed `gewyvern-service-v1` process can run the same bounded collector
as the formal CLI and publish its real facts through authenticated TLS.
Capture is opt-in, with no separate shell worker or second collector loop.
The ordinary API-only installation remains unchanged when no plan is supplied.

## Installation Contract

The strict `gewyvern-install-v1` / `gewyvern-activate-v1` request accepts this
optional field in addition to its existing identity, artifact and credential
fields:

```json
{
  "capture": {
    "schema_version": 1,
    "cgroup": "/sys/fs/cgroup/system.slice/example.service",
    "udp_port": 53,
    "window_ms": 1000,
    "ring_kib": 1024,
    "max_facts": 4096
  }
}
```

This is a request fragment, not a complete installer request. Rust callers use
`gewyvern_install_contract::capture::UdpCapturePlan` and set `request.capture`.
All plan fields are required; unknown fields, invalid schemas, root/dot/relative
scopes, zero ports and out-of-range budgets are rejected. Existing requests
omit the field and retain their original wire shape. An older installer that
does not understand the new field must reject it, not silently ignore capture.

Capture plans are Linux-only. `user` installations cannot request capture;
`system` requires the operator's existing administrative installation authority.
The `test` profile is for isolated field validation, not privilege elevation.
No sudo policy, capability grant or workload cgroup membership is changed.
The workload cgroup must exist at service start. Kernel, clang/libbpf and UDP
limits are the same as [live UDP capture](live-udp-capture.md).

The plan is bound into both the private install manifest and service plan.
Startup rejects disagreement. Replaying the same artifact/runtime with a changed
plan is a generation conflict, not an in-place scope change. This round does
not introduce hot reconfiguration: choose a new runtime identity or a new
artifact generation through the normal reviewed deployment workflow.

## Runtime And Readiness

The generated Linux unit supplies separate `GEWY_CONFIG_HOME`, `GEWY_DATA_HOME`,
`GEWY_STATE_HOME` and `GEWY_CACHE_HOME` directories under that runtime's install
root. The process verifies those exact paths and refuses certificate-directory
overrides. Invoking the installed binary manually without its generated runtime
environment is intentionally rejected; do not fall back to the administrator's
shared home directory. State paths are private, not arbitrary request outputs:

```text
<runtime-root>/state/capture/export.json
<runtime-root>/state/capture/status.json
```

Installed capture uses the built-in UDP debug template. Custom DSL/protocol
selection is not part of this plan version. The explicit cgroup/port and actual
packet evidence are retained; unavailable lineage and other fragments remain
unavailable. It is not a claim of full protocol/UI capability coverage.

The collector attaches before the TLS listener starts. `/health`, protected by
the existing CA and admin-token checks, includes compact `capture` metadata.
Activation health verification requires the selected cgroup and port to match,
with a live, non-loss-marked capture run. An API-only response is not sufficient
for a capture-enabled request. `/v1/latest/export.json` carries the real facts
and the same cumulative accounting. Normal windows use the existing bounded
latest-output policy; complete API/history trees are only saved at termination.

The generated unit keeps `NoNewPrivileges`, `PrivateTmp`, read-only system paths
and a runtime-root write allowance. It does not grant new capabilities.
`SuccessExitStatus=3` keeps an accounted-loss exit from becoming a crash/restart
loop; operators must still inspect `incomplete` rather than calling it loss-free.
Linux units explicitly bound repeated startup failure to five starts per minute;
see [systemd recovery](systemd-udp-recovery.md) for retry and operator reset steps.
SIGTERM drains and releases hooks. SIGKILL cannot flush facts or counters, but
process-owned links detach and a fresh service process can start. TLS startup
failure also drains/detaches and marks the status failed, rather than leaving
a live collector behind an unavailable API.

## Linux Evidence

The opt-in installed-service test runs the actual installed generation executable:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_INSTALLED_CAPTURE_OUT=/tmp/gewy-installed-new-run \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact installed::installed_capture_service_tls_lifecycle --nocapture
```

It checks real IPv4/IPv6 UDP traffic over pinned-CA TLS, token rejection,
wrong-CA rejection, immutable-plan replay, state-root confinement, TLS and
capture startup failure, graceful/forced exit, output failure and preserved
loss exit codes. Use the test executable printed by Cargo and a new directory.
It uses native loopback peers and the isolated `test` install profile.

The [installed-process proof](fixtures/gewyvern_installed_udp_linux_x86_64_20261113.json)
launches the installed process directly and performs explicit restart.
The separate [systemd host test](systemd-udp-recovery.md) executes the generated
unit and checks actual automatic restart and bounded fault recovery. Neither
certifies native activation/boot enablement, reboot, least-privilege capability
deployment, other kernels or hour-scale throughput.
Leserpent UI/remote-deployment selection of this plan remains follow-up work;
the native request and service implementation are ready for that adapter.
