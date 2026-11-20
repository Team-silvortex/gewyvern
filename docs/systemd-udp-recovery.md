# Systemd UDP Recovery

This is the Linux supervisor acceptance layer for
[installed UDP capture](installed-udp-service.md). It runs the generated unit
unchanged under the host's real systemd manager, not inside a Docker systemd
simulation. It is separate from boot enablement and installer activation.

## Recovery Policy

Generated Linux services use `Restart=on-failure`, `RestartSec=2`,
`StartLimitIntervalSec=60` and `StartLimitBurst=5`. The explicit limit prevents a
persistent fast failure from retrying indefinitely under a host's shorter default
interval. With two-second retries, the tested host's default five starts per ten
seconds did not stop a persistent invalid-certificate failure: ten retries were
observed before the regression test stopped the unit.

Transient failures can recover automatically after their cause is removed:

- SIGKILL releases process-owned BPF links; the next process attaches a fresh run.
- Invalid TLS certificates fail closed before serving clients. Replacing the
  certificate with the original valid private file permits the scheduled retry.
- Output failure releases the collector; repairing the output path permits retry.
- Removing a target cgroup stops capture. Recreating the same canonical path lets
  a new process attach, without reusing the old inode or run identity.

These recoveries do not preserve pending packets or final counters from SIGKILL.
They are fresh capture runs, not resumed transactions. The cgroup path is the
selected identity in this plan version; recreating it intentionally selects the
new workload at that path.

After the start limit is reached, first correct the underlying problem, then
reset and start only the affected service:

```bash
sudo systemctl status gewyvern-<runtime-id>.service
sudo journalctl -u gewyvern-<runtime-id>.service -n 100 --no-pager
sudo systemctl reset-failed gewyvern-<runtime-id>.service
sudo systemctl start gewyvern-<runtime-id>.service
```

`systemctl start` is not capture readiness: verify authenticated `/health` has
the requested scope/port and a running, non-loss-marked capture snapshot. An
API-only response is insufficient. Do not reset limits globally or remove
filesystem/TLS protections to get a failing service started.

`systemctl stop` drains the collector and does not schedule a restart. Accounted
loss returns exit code 3; the capture-enabled unit's `SuccessExitStatus=3`
prevents a retry loop, but **does not make incomplete evidence loss-free**.
Check `status.json` and the export's capture counters even when systemd reports
`Result=success`.

## Repeatable Host Check

Build on Linux, then use the exact test executable printed by Cargo:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_SYSTEMD_CAPTURE_OUT=/tmp/gewy-systemd-new-run \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact installed::systemd::generated_unit_restart_and_failure_recovery --nocapture
```

The evidence directory must not already exist. The test needs root, host systemd,
cgroup v2, libsystemd, clang, libbpf and bpftool. It creates uniquely named
`gewyvern-field-systemd-*` units only in `/run/systemd/system`, never enables them,
and removes its units and cgroup on completion. Both success and assertion-failure
paths attempt owned-service cleanup. Do not run it on a host where this bounded
fault injection is inappropriate.

The native driver keeps a D-Bus `RefUnit` reference during observation. Otherwise
systemd can garbage-collect a successfully stopped unit and a later property read
can show default exit metadata. This changes neither the unit's restart policy
nor its executable. For rate-limit acceptance it checks the failed state, stable
restart count and manager journal together: some versions retain `Result=exit-code`
for the original failure even when the journal confirms start-limit refusal.

The [dated proof](fixtures/gewyvern_systemd_udp_linux_x86_64_20261113.json) retains
actual TLS dual-stack exports, final counters, invocation identities, supervisor
properties and the reproduced pre-fix restart behavior. It does not certify
`gewyvern-activate-v1`, persistent boot enablement, reboot recovery, least-privilege
provisioning, another physical host/kernel, or sustained production throughput.

Policy references: [systemd service restart behavior](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html)
and [systemd manager D-Bus interface](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.systemd1.html).
