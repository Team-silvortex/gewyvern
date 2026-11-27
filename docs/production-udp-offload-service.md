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
