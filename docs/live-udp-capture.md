# Live Linux UDP Capture

The formal `gewyvern` CLI can start its own bounded eBPF UDP producer. This is
not demo input, socket ingest or the validation collector. `--serve` repeatedly
consumes it and publishes independent analysis windows through the existing
runtime API. Capture is explicit, never enabled by installing the program.

## Prerequisites And Scope

- Linux with cgroup v2, BPF ring buffers, cgroup BPF links and the required
  socket/netns-cookie helpers. The dated proofs cover x86_64 kernels 7.0.0-42
  and 7.0.0-43 on the same physical host; they do not establish the minimum
  supported kernel or another host.
- `/usr/bin/clang` with the BPF target, Linux/libbpf development headers and
  `libbpf.so.1` providing `ring_buffer__consume_n`. The embedded product source
  is compiled at startup in a private temporary directory, with a 30-second
  deadline. No repository checkout, shell, Python, Node or Docker is needed
  by the collector. This is not yet a precompiled portable BPF-object package.
- Root or appropriately provisioned BPF/cgroup privileges. The field test uses
  root; a least-privilege capability profile has not been certified. Do not
  grant an arbitrary CLI unrestricted passwordless sudo.
- An existing canonical, non-root directory below `/sys/fs/cgroup` and one UDP
  port. Use the workload's cgroup, not the collector's. Workload sockets must
  belong to the selected hierarchy; moving a process after socket creation
  does not prove coverage. Capture does not move processes or create scopes.
- Existing trusted parent directories for absolute output/status paths. Do
  not share output or runtime-state paths between separate collectors.

Only direct UDP is accepted: IPv4 with a valid header length or IPv6 immediately
after its base header. The observer always allows the original packet. It does
not intercept, modify or block traffic. Source or destination UDP port may
match; both endpoints inside the scope can produce two hook observations for
the same datagram. Counters count observations, not deduplicated wire packets.
Real duplicate deliveries remain separate observations. The [bounded netem check](production-udp-network.md#delay-duplication-and-reordering)
verifies multiplicity and per-socket directional sequences under delay and
reordering; it does not establish a global wire order or transaction de-duplication.

The [private veth check](production-udp-network.md) verifies IPv4/IPv6 datagrams
that the kernel fragments and reassembles around these hooks. This does not
change the direct-header parser restriction or add product-owned reassembly;
raw fragments and arbitrary extension chains remain unsupported.

## Start And Consume

Replace the example cgroup with the workload's actual existing cgroup, and use
a private output directory you have already created:

```bash
sudo gewyvern --capture-cgroup /sys/fs/cgroup/system.slice/example.service \
  --capture-udp-port 53 --capture-seconds 30 --protocol dns --entry udp \
  --json --out /var/lib/gewyvern/capture/export.json \
  --capture-status /var/lib/gewyvern/capture/status.json
```

Continuous service mode, with a loopback runtime API:

```bash
sudo gewyvern --serve \
  --capture-cgroup /sys/fs/cgroup/system.slice/example.service \
  --capture-udp-port 53 --capture-window-ms 1000 \
  --capture-max-facts 4096 --capture-ring-kib 1024 \
  --protocol dns --entry udp --json \
  --out /var/lib/gewyvern/capture/export.json \
  --capture-status /var/lib/gewyvern/capture/status.json \
  --api-socket 127.0.0.1:9100
```

The API's `/v1/latest/export.json` includes the same `capture` metadata.
Running windows update the in-memory API and atomic `--out`/status files;
the full persisted API tree and history are saved only at terminal publication.
This avoids copying the entire protocol catalog and report tree every window.
After SIGKILL, the persisted API tree may describe an older run; the explicit
output/status files and process liveness are the recovery inputs, not an assumed
complete historical archive. Ordinary external-fact socket sessions keep their
existing persistence policy.
Existing remote-API authentication restrictions still apply. A process manager
can supervise this explicit `--serve` command; the separately provisioned
`gewyvern-service-v1` TLS API launcher does not implicitly enable capture or
invent a workload scope. It now supports an explicit installation-bound capture
plan as described below. The Leserpent deployment UI/control-plane selection
surface remains a separate integration boundary.

Without `--protocol` or `--dsl`, capture uses the UDP debug template. Non-UDP
templates and conflicting socket/demo/scan modes are rejected. Only the UDP
packet fragment is supplied; route, lineage, QUIC and other fragments remain
explicitly unavailable rather than silently advertised as attached.

`--out` is atomically replaced with the **latest independent window**, not an
append-only archive. The final window can be empty. `--serve --max-sessions N`
includes the final window in its limit; `--capture-seconds` can also bound a
service run. Without `--serve`, facts accumulate within the selected capacity
until duration or a graceful stop. Bounds:

| Option | Default | Accepted range |
| --- | --- | --- |
| `--capture-window-ms` | 1000 | 10..60000 |
| `--capture-max-facts` | 4096 | 1..65536 per window |
| `--capture-ring-kib` | 1024 | power of two, 4..16384 |
| `--capture-seconds` | required for one-shot | 1..86400 |

There is no cross-window transaction continuation or historical packet archive.
Consume each window externally if history is required. Capture stores at most
128 actually observed payload bytes, full IP length, address family, ports,
socket cookie and a run-local namespace ID. Full IP endpoint addresses and
kernel namespace-cookie mappings are not retained in packet facts. No PID or
authenticated process lineage is inferred; exports remain `unverified-local`.

## Loss And Recovery

For the installed TLS service, see [installed UDP service](installed-udp-service.md).

`capture.schema_version = 1`; exact 64-bit counters are decimal strings. They
are cumulative within a `run_id`, while export facts belong to one window:

- `kernel.matched`: admitted observations before ring reservation.
- `kernel.ring_dropped`: ring reservation failures or discarded failed loads.
- `kernel.unsupported`, `malformed`, `no_cookie`: rejected observations before
  usable facts. Unsupported headers can prevent port inspection, so these are
  conservative scope-level diagnostics, not necessarily selected-port loss.
  A post-reservation load failure also increments `ring_dropped`; categories
  must not be blindly added together.
- `received`: ring records consumed; `retained`: facts admitted to bounded
  analysis windows, **not proof of durable archival**.
- `userspace_dropped`: admitted record exceeded the current fact budget;
  `decode_rejected`: unusable ABI/framing/identity record.

Running snapshots are not globally atomic. On graceful shutdown, links are
detached first, the remaining ring is drained within two seconds, and
`final_accounted` requires both exact equations:

```text
received + kernel.ring_dropped = kernel.matched
retained + userspace_dropped + decode_rejected = received
```

`incomplete = true` on loss or failure. A reconciled run can still be incomplete.
Conversely, a complete capture does not prove delivery: packets may be dropped
after an egress hook or before reaching a selected ingress hook. Application
timeouts are not collector-drop counters. The [isolated network-loss check](production-udp-network.md#network-loss-and-recovery)
verifies both sides of this distinction without inventing remote reception or
claiming automatic root-cause classification.
Successful complete stops return 0; accounted but incomplete stops return 3;
runtime/startup failures return 1 and invalid CLI input returns 2. Monitor the
capture status/export, not only protocol findings. Use a supervisor policy that
does not silently interpret exit 3 as complete evidence.

SIGINT/SIGTERM detach, drain, publish and update status. Removing/replacing the
scope or losing the output path fails closed and releases owned attachments.
An advisory inode lock excludes a second cooperating collector on the exact
same cgroup, not overlapping ancestor/descendant scopes or unrelated BPF tools.
Links are process-owned and unpinned: SIGKILL releases them, and a fresh run can
start. SIGKILL/host failure cannot flush pending facts or final counters; stale
status may still say `running`. Verify process liveness and `run_id`; never
claim the previous run is complete or resumed. Startup compiler termination
under SIGKILL and automatic supervisor restart have not been proven here.

## Reproduce The Linux Check

For empty/short/large payloads, unconnected client/server sockets and descendant
cgroup coverage, see [packet and socket boundary checks](production-udp-boundaries.md).
For cross-namespace interface identity and kernel fragmentation/reassembly,
see [private MTU-1500 network checks](production-udp-network.md).

Build without root, then run only the named ignored test with privileges:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_PRODUCTION_CAPTURE_OUT=/tmp/gewy-production-new-run \
  GEWY_CAPTURE_SERVICE_COUNT=3000 \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact production_capture_cli_lifecycle_and_loss --nocapture
```

Use the executable path printed by Cargo and a new output directory. `bpftool`
is a test-only prerequisite for checking detachment. The test's native IPv4/IPv6
loopback DNS-shaped peers are byte-checked, not a full DNS server conformance
test. It exercises startup, sustained consumption, API, scope exclusion,
overflow/resume, user limits, graceful/forced exit, output failure, scope removal
and restart. TCP capture, IPv6 extensions/fragments, jumbograms, checksum
verification, pre-hook reassembly, high-throughput performance and hour-scale
production-service retention remain separate gates.
