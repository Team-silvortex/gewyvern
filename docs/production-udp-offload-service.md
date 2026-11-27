# UDP Offload Service Reliability

The [production UDP collector](live-udp-capture.md) retains whole normalized
GSO/GRO groups across consumption batches and service windows. This check uses
real dual-stack traffic and the formal CLI to verify both successful completion
and budget rejection, then reuses the same sockets across SIGKILL and restart.
Further checks cover idle expiry and repeated aggregate-ID history rollover.
It extends the [bounded offload proof](production-udp-offload.md), not the protocol
surface or the supported offload types.

## Window Boundary

Eight rounds each send two 16-segment requests, one per IP family, with ordinary
replies. Native server reads must return actual GRO metadata and byte-identical
aggregates; the 17-byte final segment is checked independently. These produce
544 hook records in the paused collector's ring.

The first 512-record consume ends after two members of group 31, the IPv6 client
egress group in round 7. Groups 1..30 and their ordinary replies account for
510 publishable records. The two tested capacities distinguish the outcomes:

| Per-window capacity | First window | Second window | Recovery window | Terminal retained | Userspace dropped |
| --- | --- | --- | --- | --- | --- |
| 528 | 510 | 34 | 20 | 564 | 0 |
| 512 | 510 | 18 | 20 | 548 | 16 |

At capacity 528, the pending group's 16-slot reservation survives publication.
Its two received records remain pending until the next consume completes the
group. At capacity 512, there is no room for that reservation: both initial
members and all 14 later members count as budget drops. Freeing the first
window does not resurrect its tail. Group 32 and a later 4-segment recovery
round are accepted in either case.

During the first window, kernel `matched=544` but userspace `received=512`:
32 records still wait in the ring. The larger capacity also has two received
pending records. Neither running snapshot is `final_accounted`; these temporary
differences are not guessed losses. After orderly shutdown both equations
reconcile. Only the capacity-limited run exits 3 and remains incomplete.

The controller first observes an empty service publication and pauses only
after confirming the stopped main thread is in its idle sleep. An initial
`running` status proves BPF attachment, not that the window timer has started.
Pausing immediately on startup status was a test race and is not used as proof.

## Process Boundary

After orderly shutdown of the window case, a new collector captures another
4-segment round using the same peer sockets and is killed with SIGKILL.
Its hooks detach, but its last persisted status remains `running` with
`final_accounted=false`. One more round completes while no collector is present.

A fresh collector starts in the same cgroup and output directory. Its new
`run_id` is required before the next round is admitted as evidence. The restarted
run retains exactly 20 observations from that new round, not the downtime round
or the old export. Its aggregate IDs restart at 1..4, which is valid only because
they are scoped to the new run. Do not join different runs by aggregate ID alone;
see the [group import rules](production-udp-offload.md#group-admission-and-replay).

The SIGKILL step does not prove flushing or recovery of unpublished records.
`--out` still holds only the latest independent window, not a durable queue.
Consumers must distinguish fresh run identity, process liveness and terminal
accounting; a stale `running` file is not successful recovery.

## Evidence And Reproduction

The window/restart proof below predates the idle-expiry extension described
after it; each artifact binds its own executed sources and binaries.

The [dated proof](fixtures/gewyvern_product_udp_offload_service_linux_x86_64_20261120.json)
contains detailed native/packet oracles for both capacities and hashes plus
terminal counters for three repetitions each. Every case checks exact per-peer
packet multisets, no cross-window duplicate/partial groups, import/replay,
same-socket recovery, fragment counters and owned-resource cleanup. All six
final cases pass. Raw per-run window exports and full oracles remain on Linux;
18 local/remote build inputs and both executed binaries are hash-bound.

Build without root, then use a new output directory for each invocation:

```bash
cargo test --locked -p gewyvern --all-features --test production_capture_linux_tdd --no-run
sudo env GEWY_OFFLOAD_SERVICE_LIMIT=528 GEWY_OFFLOAD_SERVICE_OUT=/tmp/gewy-offload-service-528 \
  target/debug/deps/production_capture_linux_tdd-<build-hash> --ignored --exact \
  network::offload::service::production_capture_offload_service_windows_and_restart --nocapture
```

Repeat with capacity `512` and a different output directory. The test owns two
MTU-1500 network namespaces and one cgroup; it changes no host-interface offload
settings. Root, cgroup v2, clang/libbpf, bpftool and iproute2 are required.
The fixture and rejection controls are checked by
`tests/native_validation_harness_tdd/udp_offload_service.rs` without privileges.

This is a bounded 12-round, single-host veth test, not a soak/performance result,
physical-NIC or older-kernel qualification, durable archival, or supervisor
restart proof. No production code change or status-maturity promotion was needed.

## Idle Expiry And Capacity Recovery

The [idle-expiry proof](fixtures/gewyvern_product_udp_offload_expiry_linux_x86_64_20261120.json)
adds a running-service case for IPv4 and IPv6, each repeated three times. Only
the client cgroup is captured; the native server is in a separate owned cgroup.
A paused reader, 4 KiB ring and 24-fact budget retain eight ordinary baseline
records followed by nine members of a 16-segment group. The remaining seven
members and its reply are counted as eight ring drops.

| Stage | Received | Retained | Budget drops | Decode rejects | Pending members |
| --- | --- | --- | --- | --- | --- |
| Partial group | 17 | 8 | 0 | 0 | 9 |
| Probe before expiry | 34 | 9 | 16 | 0 | 9 |
| Idle expiry | 34 | 9 | 16 | 9 | 0 |
| Full group after expiry | 51 | 26 | 16 | 9 | 0 |
| Short recovery group | 56 | 31 | 16 | 9 | 0 |

The partial group's reservation survives the first publication, so the next
16-segment probe is budget-rejected. With no further traffic or stop signal,
idle polling expires the original group and releases its reservation. Only its
nine received members become decode rejects; unseen members remain kernel loss.
Subsequent complete 16- and 4-segment groups are accepted using the same socket.
The two-second deadline is fixed at first receipt, not renewed by empty windows.
The observed expiry publication occurred 2.16..2.24 seconds after the partial
publication in these runs, including the 200 ms publication cadence.

Every final snapshot reconciles 64 matched = 56 received + 8 ring drops and
56 received = 31 retained + 16 budget drops + 9 decode rejects. The process exits
3, not 0, because recovery does not erase earlier loss. All retained packet
prefixes/lengths and native full-payload hashes are checked; only new group IDs
3 and 4 appear in exports. This does not claim late missing kernel segments can
be recovered. Twenty local/remote build inputs and both binaries are hash-bound.

Run the same built test executable with a new output directory:

```bash
sudo env GEWY_OFFLOAD_EXPIRY_FAMILY=4 GEWY_OFFLOAD_EXPIRY_OUT=/tmp/gewy-offload-expiry-v4 \
  target/debug/deps/production_capture_linux_tdd-<build-hash> --ignored --exact \
  network::offload::service::expiry::production_capture_offload_idle_expiry_and_capacity_recovery --nocapture
```

Repeat with family `6` and another new directory. Offline verification and
negative controls live in `tests/native_validation_harness_tdd/udp_offload_expiry.rs`.

## Empty Window Allocation

`CaptureFactBuffer::take_facts()` now preserves the ready buffer when empty and
returns an unallocated empty vector, instead of allocating another full-capacity
buffer at every idle publication. Nonempty publications retain the existing
bounded ownership transfer. Pending reservations and deadlines are untouched.
Two regressions fail before this change and pass after it: 1,000 empty takes at
capacities 1, 24 and 65,536 retain the same allocation, and a pending group's
reservation still expires and admits a later complete group. This eliminates
ready-buffer churn only; analysis/serialization still allocate, and no whole-
service allocation-free or measured CPU/RSS improvement is claimed.

## Aggregate History Rollover

The [history proof](fixtures/gewyvern_product_udp_offload_history_linux_x86_64_20261120.json)
keeps one formal CLI process and the same IPv4/IPv6 sockets alive across 129
nonempty service publications. The first 128 blocks each contain 80 real GSO/GRO
groups and 400 hook observations. A final block uses two 16-segment requests,
including 17-byte tails, to test recovery after both 4,096 and 8,192 group-ID
boundaries. Each run retains exactly 51,268 facts from 10,244 groups with no
kernel loss, userspace drop or decode rejection, then exits 0 with final accounting.

Every block uses distinct payload identities, checks full native request/reply
bytes and actual GRO metadata, then independently checks captured packet
multisets, group membership, run-local ID continuity and replay. The controller
pauses only at the main loop's idle syscall, queues a bounded block in the ring,
and resumes for publication. Empty windows are allowed between blocks; this is
not uninterrupted traffic or a throughput benchmark. The recovery block must
publish 68 facts, not partial members or old groups.

A separate in-memory regression advances through 12,289 IDs at one fixed time,
so timeout cannot hide history eviction. Three pending groups retire in separate
history cycles, freeing reservations and deadline entries. Their late tails
remain rejected. The map never exceeds 4,096 IDs, and only received partial
members are charged as decode rejects. This checks the internal bound directly;
the native run checks externally visible traffic, not private map size.

The fixture retains one full compact run and hashes/terminal counters for three
fresh repetitions. Offline checks reconstruct all native payload hashes and
per-window packet hashes, including both boundary checkpoints. Negative controls
reject missing windows, reused group ranges, false finality, corrupted native
hashes, truncated recovery and invented rejection counters. Raw exports stay on
Linux, rather than being copied into the repository.

Each block also samples collector RSS/high-water memory and user/system CPU
ticks from `/proc`. CPU accounting covers that process, not all BPF hook work in
the traffic-generating processes. The three unoptimized dev/test runs observed:

| Run | Last sample elapsed | Sampled RSS range | Sampled HWM max | Process user + system CPU at last sample |
| --- | --- | --- | --- | --- |
| 1 | 58.05 s | 46,412..48,732 KiB | 52,368 KiB | 15.06 + 3.11 s |
| 2 | 61.51 s | 46,568..48,404 KiB | 51,620 KiB | 15.67 + 3.37 s |
| 3 | 58.13 s | 46,384..51,048 KiB | 52,380 KiB | 15.05 + 3.08 s |

Elapsed time starts after readiness; cumulative process CPU also includes startup.
Samples end before final shutdown. No monotonic RSS growth was observed in these
short runs. The 512 MiB RSS guard is a test safety ceiling,
not a memory budget or proof of leak freedom. Retained allocator memory, transient
serialization allocations and long-running memory growth require separate study.
No physical-NIC, older-kernel, peak-performance or durable-archive claim follows.

```bash
sudo env GEWY_OFFLOAD_HISTORY_OUT=/tmp/gewy-offload-history \
  target/debug/deps/production_capture_linux_tdd-<build-hash> --ignored --exact \
  network::offload::service::history::production_capture_offload_history_rollover --nocapture
```

Use a new directory per invocation. The same root/cgroup/BPF prerequisites and
owned-network cleanup rules apply. Source and binary hashes bind the executed
build; no production behavior or status-maturity promotion is introduced here.

## Multi-CPU Interleaving

The concurrent test uses four independent private veth pairs and eight persistent
workers pinned to distinct CPUs. Each pair has connected IPv4/IPv6 sockets. Native
clients share timed release points; their observed execution intervals must
overlap. Both request and reply egress CPU IDs must match worker affinity, not
just the configured mask. Actual server GRO metadata and full native payloads
remain required. No host route or interface setting is changed.

Sixteen bursts each produce 544 hook records from 32 sixteen-segment groups plus
ordinary replies. A paused reader and elapsed service clock force a publication
after consuming 512 records. Complete groups are published, while partial groups
remain reserved until the following window; the exact split varies with CPU
scheduling. Each export must contain complete groups, and independently rebuilt
packet multisets, every fact ID, group ownership and replay must match across
both windows. A final four-segment recovery burst brings the total to 8,784 facts
and 528 groups, with no losses/rejections and a complete exit-0 snapshot.

The test counts observed producer interleaving from non-contiguous member fact
IDs and counts group-ID descents in first-arrival order. These are observations,
not a promise to force a particular kernel schedule. A separate fixed-time
regression feeds tail-first members from 32 out-of-order groups across 160 rounds,
publishes between completions and crosses the history boundary. All 10,240
members survive with bounded reservations/history and no rejection. This models
adversarial bounded order independently of what the native scheduler happens to
produce; it is not evidence of a kernel emitting tail-first group records.

Initial probing exposed a controller race: expensive replay of one window could
allow the next nonempty export to be overwritten by an empty publication. Kernel
and userspace counters still reconciled, but the controller had missed the
window. The concurrent test now freezes at the idle-loop boundary immediately
after reading a nonempty export, before replay, and uses a 600 ms publication
cadence. It explicitly rejects skipped or overwritten publications. Earlier
probes remain separate from accepted evidence. This does not turn `--out` into
a durable archive or a lossless history subscription.

The [dated concurrent proof](fixtures/gewyvern_product_udp_offload_concurrent_linux_x86_64_20261120.json)
contains one detailed accepted run, including member fact IDs and publication
ownership, plus raw-summary hashes and terminal counters for three fresh runs:

| Accepted run | Groups with interleaved members | First-arrival group-ID descents | Windows with pending members | Maximum pending members |
| --- | --- | --- | --- | --- |
| 1 | 358 | 22 | 16 | 40 |
| 2 | 258 | 13 | 16 | 40 |
| 3 | 188 | 12 | 16 | 8 |

All three finish at 8,784 matched = received = retained, with every loss/rejection
counter zero. Twenty-two matching local/remote inputs and both executed binaries
are hash-bound. Offline verification reconstructs all native full-payload and
packet hashes, every member/ordinary fact ID, exact window ownership, pending
counts and final accounting. Negative controls reject duplicate IDs, moved
groups, false finality, absent recovery GRO metadata, missing facts and invented
decode rejection. Raw probes/exports stay on Linux. No production defect or
status-maturity promotion is claimed by this test-controller fix.

```bash
sudo env GEWY_OFFLOAD_CONCURRENT_CPUS=0,2,4,6,8,10,12,14 \
  GEWY_OFFLOAD_CONCURRENT_OUT=/tmp/gewy-offload-concurrent \
  target/debug/deps/production_capture_linux_tdd-<build-hash> --ignored --exact \
  network::offload::service::concurrent::production_capture_offload_multi_cpu_windows --nocapture
```

Choose eight distinct CPUs allowed on the host; this list is the tested host's
configuration, not a portable default. Use a fresh directory per run. This is
a paced single-host correctness test, not peak throughput, exhaustive CAS
contention/exhaustion, a long soak, physical-NIC or multi-host qualification.

## Multi-CPU Ring Loss And Recovery

The [small-ring proof](fixtures/gewyvern_product_udp_multicpu_pressure_linux_x86_64_20261120.json)
reuses four pinned dual-stack peer pairs, but limits the ring to 4 KiB while
leaving the userspace budget at 4,096 facts. Eight ordinary hook records first
occupy the paused reader's ring. Four overlapping clients then generate 544
observations from 16-segment requests and ordinary replies. With the current
232-byte private record plus ring framing, only 17 records fit: at most nine
slots remain after the ordinary prelude, so no complete 16-segment group fits.

The collector resumes, then idles with no new traffic or stop signal until its
fixed partial-group deadline clears the received incomplete members. Recovery
uses the same sockets, admitting each lane separately: two-segment requests on
IPv4/IPv6 produce 12 complete observations per lane, below ring capacity. This
isolates ring loss from userspace-budget refusal and from native UDP delivery.

All three accepted runs execute three cycles in one CLI process. Each cycle
has the following accounting; the table shows increments for that cycle:

| Stage | Matched | Ring drops | Received | Retained | Decode rejects | Pending members |
| --- | --- | --- | --- | --- | --- | --- |
| Prelude plus pressure | 552 | 535 | 17 | 8 | 0 | 9 |
| Idle expiry | 552 | 535 | 17 | 8 | 9 | 0 |
| Four recovered lanes | 600 | 535 | 65 | 56 | 9 | 0 |

Each full run finishes at 1,800 matched = 1,605 ring-dropped + 195 received and
195 received = 168 retained + 27 decode-rejected, with zero userspace drops,
unsupported observations, malformed records or missing cookies. The exit code
remains **3** with `incomplete=true`: successful recovery does not erase loss.
Here decode rejection means received incomplete-group members, not malformed
native payloads. Expiry publication occurs about 2.55..2.57 seconds after the
resume point, including the 600 ms service cadence, not exactly at the deadline.

All native request/reply bytes and GRO metadata are verified. The 822 logical
request datagrams and 78 ordinary replies per run succeed at the socket layer;
the injected loss is in observation, not application delivery. No pressure-group
facts are published. Each recovery window contains four complete two-segment
groups and ordinary replies, with original socket and namespace identity.

Offline verification rebuilds native hashes and every retained packet, checks
unique IDs, group/peer ownership, incomplete running snapshots, timeout accounting
and terminal loss. Six negative controls reject partial-group publication, false
finality, erased ring loss, missing recovery records/GRO metadata and exit-0
claims. The proof binds 23 matching local/remote inputs and both actual binaries.
An initial probe exposed a single-namespace assumption in the test packet helper;
verification now partitions by peer before whole-run reconciliation. That probe
is excluded and retained on Linux, not presented as a production failure.

```bash
sudo env GEWY_OFFLOAD_CONCURRENT_CPUS=0,2,4,6,8,10,12,14 \
  GEWY_OFFLOAD_PRESSURE_OUT=/tmp/gewy-offload-multicpu-pressure \
  target/debug/deps/production_capture_linux_tdd-<build-hash> --ignored --exact \
  network::offload::service::concurrent::ring_pressure::production_capture_multi_cpu_ring_loss_and_recovery --nocapture
```

Use eight allowed CPUs and a fresh output directory. The induced drop ratio is
not a throughput result or a production loss-rate estimate. These short private
veth runs do not qualify other kernels/physical NICs or recover lost kernel
tails, and latest-window output remains non-durable. No production behavior or
status maturity is promoted by this evidence.

## Mixed Ring And Budget Pressure

The [mixed-pressure proof](fixtures/gewyvern_product_udp_mixed_pressure_linux_x86_64_20261127.json)
uses the same 4 KiB ring with a limit of **24 facts**. The eight ordinary
prelude records leave room for one 16-member reservation. Before its partial
members expire, a separate dual-stack probe contributes 68 matched observations:
51 never enter the ring and the 17 delivered to userspace are refused for lack
of budget. The probe publishes no facts and does not change the pending count.
Without more traffic, expiry then releases the reservation; each of the four
lanes captures its next complete two-segment groups on the same sockets.

| Run (three cycles each) | Ring drops | Userspace budget drops | Expired members | Retained |
| --- | ---: | ---: | ---: | ---: |
| 1 | 1,758 | 57 | 21 | 168 |
| 2 | 1,758 | 63 | 15 | 168 |
| 3 | 1,758 | 55 | 23 | 168 |

In each run, 2,004 matched = 1,758 ring drops + 246 received. The final
snapshot accounts for all 246 received and exits 3 with loss still marked.
Differences between runs reflect which group members reach the reserved slots.
Neither kernel-dropped records nor budget refusals are counted as expired
members. The late-tail regression in the fact buffer checks rejection even
after capacity is freed; the native run does not re-inject missing kernel tails.

To reproduce, use the small-ring command above with `GEWY_OFFLOAD_MIXED_OUT`
set to a fresh directory instead of `GEWY_OFFLOAD_PRESSURE_OUT`, and select
`network::offload::service::concurrent::ring_pressure::production_capture_multi_cpu_mixed_limits_and_recovery`.
For offline verification, run `cargo test -p gewyvern --all-features --test native_validation_harness_tdd udp_offload_ring_pressure`.
These single-host private-veth runs on Linux 7.0.0-44-generic verify accounting
and recovery, not throughput or a production loss rate.

## Graceful Shutdown With An Occupied Ring

The [shutdown fixture](fixtures/gewyvern_product_udp_offload_shutdown_linux_x86_64_20261127.json)
records three IPv4 and three IPv6 runs on Linux 7.0.0-44-generic. Stop the
reader after it publishes eight ordinary records with nine members of a
16-segment group still pending. While the reader is paused, the same native UDP
peers exchange a new 16-segment request and an ordinary reply, exactly filling
the client-side ring. Queue SIGTERM before SIGCONT to exercise the final drain.

All six processes exited before the two-second partial-group deadline (119..123 ms
from the first resume to exit). Each final window contained just the new reply;
none of the prior eight facts or incomplete segments appeared. The stopped run
accounted for 42 matched = 8 ring drops + 34 received, and 34 received =
9 retained + 16 refused for budget + 9 incomplete members rejected. Exit 3
preserved that loss. With both peers still alive, a new CLI run on the same
cgroup and sockets captured complete 16- and 4-segment groups and both replies
(22 retained, no loss, exit 0). No host route was changed.

Reproduce with the ignored native test
`network::offload::service::expiry::shutdown::production_capture_offload_shutdown_drain_and_restart`.
Set `GEWY_OFFLOAD_EXPIRY_FAMILY=4` or `6` and `GEWY_OFFLOAD_SHUTDOWN_OUT` to
a fresh directory. The timing establishes pre-expiry shutdown on this host,
not a shutdown latency guarantee; SIGKILL cannot promise a final drain.

## Forced Stop With An Occupied Ring

The [crash fixture](fixtures/gewyvern_product_udp_offload_pending_crash_linux_x86_64_20261127.json)
uses the same partial-group prelude, then kills the paused reader with SIGKILL
after the peers have completed another 16-segment request and its reply. Unlike
SIGTERM, this cannot produce a terminal snapshot. In three runs per address
family the last publication remained byte-identical: 25 matched, 8 ring drops,
17 received, 8 retained and 9 pending, with `state=running` and
`final_accounted=false`. These are **last published** counters, not final
counts and not evidence that the process is still alive.

After verifying that the hooks detached, the peers exchanged another request
and reply before restart. The CLI reused the same cgroup, status and export
paths, but readiness required a new run ID and an empty publication. Only the
next four-segment request and reply appeared: 5 retained, group ID 1, no loss,
exit 0. Peer payload hashes cover all three exchanges on the original sockets.
Queued observations were not recovered and downtime traffic was not backfilled.

To reproduce, use the same family setting as the graceful-stop test, set
`GEWY_OFFLOAD_CRASH_OUT` to a new directory, and select
`network::offload::service::expiry::shutdown::production_capture_offload_kill_pending_and_restart_in_place`.

## Status Write Failure And Repair

The [Linux results](fixtures/gewyvern_product_udp_status_repair_linux_x86_64_20261127.json)
cover IPv4 and IPv6, with and without a loopback API, during a running window
and during SIGTERM. After an ordinary window and an admitted partial group, the
test replaces only its own status file with a directory, leaving the export
writable. Before the fix the process exited 1, but its file-only export still
said running or stopped; with the API enabled, failure cleanup overwrote the
last reply with an empty window.

All eight repaired cases return `capture_status_write_failed` and exit 1.
The remaining export is failed, incomplete and reconciled (42 matched, 8 ring
drops, 34 received = 9 retained + 16 budget drops + 9 rejected partial
members). Its single reply survives replay. Where enabled, both the persisted
API latest export and the sole terminal history entry match it. This is a
publication failure, not an extra packet-drop category.

The test repairs the destination, restores the old running status as a stale
file and restarts in the same directory on the original sockets. A new run ID
starts empty, retains only the five post-restart observations and exits 0.

Reproduce with the ignored Linux test
`network::offload::service::expiry::shutdown::status_failure::production_capture_status_write_failure_and_repair`,
`GEWY_OFFLOAD_EXPIRY_FAMILY=4|6`, a fresh `GEWY_STATUS_OUT`, and
`GEWY_STATUS_CASE` set to `running`, `terminal`, `running-api` or `terminal-api`.

## Both Outputs Read-Only

The [eight native Linux runs](fixtures/gewyvern_product_udp_readonly_outputs_linux_x86_64_20261127.json)
repeat that matrix after a successful running window. In a private mount
namespace, the test makes the directory containing both CLI files read-only
and verifies an independent write fails with `EROFS`. Stderr and optional
API state stay on a writable filesystem.

The process exits 1 and releases its BPF attachments. In both periodic and
SIGTERM publication, the JSON error on stderr retains the primary
`output_write_failed`; its message also identifies the final status path and
`[capture_status_write_failed]`. Before the fix this second failure was silent.
Both files remain byte-identical to the last running window, not a final count.
In file-only mode there is no terminal snapshot. With writable API state, the
persisted latest and single terminal history entry agree: failed, incomplete,
42 matched = 8 ring drops + 34 received. Those 34 comprise 9 retained records,
16 userspace drops and 9 decode rejections. These are not extra losses caused by the read-only mount.

Removing the read-only mount alone does not restart the collector or change
either file. Restarting the same CLI on the same paths and cgroup produces a
new run ID and an empty window; the original sockets then contribute exactly
four request segments and one reply. No downtime traffic is backfilled, and
the new run exits 0. This is same-path restart, not in-process retry or proof
that API history survives when its own storage is read-only.

Reproduce with the ignored Linux test
`network::offload::service::expiry::shutdown::status_failure::read_only_outputs::production_capture_read_only_outputs_and_repair`.
Use a fresh `GEWY_READONLY_OUT`, `GEWY_OFFLOAD_EXPIRY_FAMILY=4|6` and the
same four `GEWY_STATUS_CASE` values as above. It requires BPF and mount
privileges; do not remount the host filesystem to reproduce it.

## API State And Both CLI Outputs Read-Only

The [four native runs](fixtures/gewyvern_product_udp_readonly_archive_linux_x86_64_20261127.json)
seed a real, clean terminal entry, start a different capture and wait for its
first nonempty running window. Running windows do not replace the persisted API
latest. Two private bind mounts then make the CLI directory and the entire API
state directory read-only; independent write probes on each return `EROFS`.
IPv4 and IPv6 were each exercised at periodic publication and at SIGTERM.

The final JSON on stderr preserves primary `output_write_failed`, exit 1 and
`retryable=true`. Its message identifies both `[snapshot_persist_failed]`
(API latest write) and `[capture_status_write_failed]` (CLI status). Before
the fix the last JSON omitted the API failure. Neither CLI file advances.
The directory names and file contents of persisted latest and history retain
the same fingerprint, including the history index: the seed is the only entry,
not a terminal record for the failed run. There is no durable final accounting
for that run.

After repair, restarting on the original paths/cgroup creates a third distinct
run ID. Its first window is empty. The original sockets contribute only four
new request segments and one reply; exit is 0. The index and on-disk archives
then list exactly the seed and the repaired run, with latest matching the latter.
This tests storage that is already read-only, not an interruption partway through
a writable multi-file archive.

Use the same privileges, family selector and fresh `GEWY_READONLY_OUT` as
above, set `GEWY_STATUS_CASE=running-api` or `terminal-api`, and select
`network::offload::service::expiry::shutdown::status_failure::read_only_outputs::production_capture_read_only_api_and_outputs`.

## Interrupted History Publication

The [three native Linux runs](fixtures/gewyvern_product_udp_interrupted_history_linux_x86_64_20261127.json)
watch the actual history writer after SIGTERM and send SIGKILL once its new
`meta.json` exists. Each run verifies that the per-target tree is still missing
at death. The CLI capture and persisted API latest export already contain the
interrupted run's stopped, reconciled snapshot (four loopback UDP observations,
IPv4 and IPv6). This does **not** make it a committed history entry or prove
that every file under latest advanced together.

Before the fix, the text history reader discovered two numeric directories
and reported removals from a half-written protocol catalog. The JSON reader
correctly listed one. Writers now serialize publication, build the archive
and its delta files in an owned nonnumeric slot, rename the complete tree,
atomically replace the index, and only then prune retired entries. Both
`--list-history` views use the published index when present; an invalid index
does not authorize a directory-scan fallback. Legacy shelves without an index
retain their text directory view.

At SIGKILL both fixed views still list only the seed and the index bytes are
unchanged. After same-path restart, HTTP `/health` reports the new empty run's
identity. The next terminal publication reclaims the interrupted staging slot;
the index then contains exactly the seed and the new four-observation run.
A new writer does not adopt arbitrary unindexed numeric directories. A kill
*after* rename but *before* index replacement could leave such a complete
orphan; the [retention-limit check](#repeated-interruption-at-the-retention-limit)
below addresses this later window. Power-loss durability is a separate gate.

Reproduce with `GEWY_ARCHIVE_INTERRUPT_OUT` pointing at a new directory and
the ignored Linux test
`network::offload::service::expiry::shutdown::status_failure::read_only_outputs::history_interruption::production_capture_interrupted_history_publication`.

## Repeated Interruption At The Retention Limit

The [before/after Linux results](fixtures/gewyvern_product_udp_history_retention_rename_linux_x86_64_20261127.json)
use two real terminal archives and `GEWY_HISTORY_RETENTION=2`. A test-owned
inotify watch signals the collector on the move of its completed archive to a
numeric name. Each kill must leave an unchanged index and both old archive
trees byte-identical, while the unindexed export has the interrupted run ID
and four matched and retained UDP observations. Both CLI views still list the
two original entries.

| Numeric directories | After kill 1 | After kill 2 | After kill 3 | After one clean stop |
| --- | ---: | ---: | ---: | ---: |
| Before fix | 3 | 4 | 5 | 5 |
| After fix (three independent repetitions) | 3 | 3 | 3 | 2 |

Before renaming, the writer records one timestamp and random publication nonce
in both the staged archive and an atomic intent file. At the next terminal
publication, under the shelf writer lock, it consults the committed index.
An indexed generation is kept. Only an unindexed directory matching the
outstanding intent may move back to the reserved staging slot. The final
successful index retains the new run and the newer seed; only then is the
oldest seed evicted. There is no background cleanup: one complete extra
archive may remain while the writer is down.

This is not a sweep of pre-upgrade or manually copied numeric directories.
A missing index with a pending intent, a conflicting destination, or a bad
stamp stops recovery rather than guessing which data to delete. The evidence
is for SIGKILL **before** index replacement, not power loss or interruption of
the [later retention prune](#interrupted-prune-after-index-replacement). Reproduce with a fresh `GEWY_RETENTION_INTERRUPT_OUT`
and the ignored Linux test
`network::offload::service::expiry::shutdown::status_failure::read_only_outputs::history_interruption::retention_interruption::production_capture_repeated_rename_at_retention_limit`.

## Interrupted Prune After Index Replacement

The [Linux results](fixtures/gewyvern_product_udp_history_prune_linux_x86_64_20261127.json)
cover the opposite side of the index switch at retention two. An inotify
watch sends SIGKILL on deletion **inside the evicted archive**. A round only
counts if the new run and its predecessor are the two indexed, readable
terminals, the survivor's tree hash is unchanged, and the victim has strictly
fewer files than before but is not empty.

| Retired directories on disk | Kill 1 | Kill 2 | Kill 3 | After clean rotation |
| --- | ---: | ---: | ---: | ---: |
| Before fix | 1 | 2 | 3 | 3 |
| After fix (three independent repetitions) | 1 | 1 | 1 | 0 |

Before replacing the index, the writer records a bounded prune plan under
the existing writer lock. Its fingerprints identify the **ordered timestamp
lists** before and after commit. At the next terminal publication, the old
list means no eviction occurred; the new list authorizes finishing only the
specified, unindexed generations. Missing or unrelated indexes do not
authorize any deletion. Each victim retains its publication stamp (or a
timestamp-checked, hashed legacy metadata file) until its payload is gone.
An already absent victim or the final empty directory can be finished safely;
a nonempty replacement with a different identity is not swept.

One additional campaign interrupted recovery of the **same** partial victim
twice: 471 to 461 to 29 files. Neither the index bytes nor either committed
tree hash changed during these retries. The subsequent clean publication left
two numeric directories, both indexed. There is no background cleanup while
the writer is stopped, no sweep of orphaned pre-upgrade data, and no proof of
power-loss durability.

From the same Linux privileged test suite, select
`production_capture_repeated_committed_prune` with a fresh `GEWY_PRUNE_INTERRUPT_OUT`,
or `production_capture_prune_recovery_is_interruptible` with a fresh
`GEWY_PRUNE_RETRY_OUT`, using either suffix as a Cargo test filter with
`-- --ignored --nocapture`.

## Changing Retention Between Evictions

Five completed runs fill a shelf at retention five. Stopping a sixth at
retention two commits that run and the newest seed, retiring four directories.
The Linux test uses a fanotify permission event on the second victim's stamp:
it allows the pre-commit identification read, but holds the cleanup read after
the first victim is gone. Only the test collector is killed. At that point the
three remaining victims must still have their original full-tree SHA-256s.

| Restart limit | While collecting | After the next successful terminal publication |
| --- | --- | --- |
| 4 | Two indexed; three old directories still await cleanup | Three indexed directories: restart, interrupted run, newest seed. No retired run returns. |
| 1 | Same committed index and pending cleanup | One directory: the restart. The two preceding runs are evicted by the new publication. |

The test pauses the restarted writer once more, after the old plan is cleared
but before the next index switch. Both old index entries must still read back as
their original terminal snapshots, the index bytes must match, and all four old
victims must be absent. At completion the numeric directories must exactly match
the new index; neither a prune journal nor an in-flight receipt may remain.

For an indexed shelf, `--list-history` reports the **committed** retention, as
`--list-history --json` does. When different, `local configured retention`
is the setting of the CLI process, not necessarily the daemon's setting. With
no recorded limit the text view uses the local policy. Starting a collector
does not by itself run cleanup or change the committed limit.

See the [Linux field evidence](fixtures/gewyvern_product_udp_history_shrink_linux_x86_64_20261127.json).
To repeat, use a fresh absolute output directory for each run (the test creates
it), on a privileged Linux host with BPF, cgroup v2 and fanotify permission events:

```sh
GEWY_SHRINK_RAISE_OUT=/var/tmp/gewy-raise-1 cargo test --locked -p gewyvern --all-features --test production_capture_linux_tdd production_capture_raise_after_partial_batch -- --ignored --nocapture
GEWY_SHRINK_LOWER_OUT=/var/tmp/gewy-lower-1 cargo test --locked -p gewyvern --all-features --test production_capture_linux_tdd production_capture_lower_after_partial_batch -- --ignored --nocapture
```

These are process-crash tests on one filesystem and kernel, not power-cut tests.
