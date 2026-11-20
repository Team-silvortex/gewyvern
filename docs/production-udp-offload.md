# Production UDP Offload Boundary

The product's ABI v1 represents one UDP datagram. A GSO/GRO aggregate is not
one datagram, even when its IP/UDP lengths are internally consistent. The
collector now rejects selected-port aggregates using `gso_size` or `gso_segs`
before admitting a normal packet fact. It always allows the original traffic.

## Prepared Segment Contract

The Rust capture adapter, fact model, export/import codec and runtime now share
an explicit **record-level** contract for future bounded normalization. This is
not enabled in the production BPF producer: ABI v1 still rejects aggregates as
described below, and its existing Linux before/after evidence remains historical.

`PacketMetaFact.udp_offload` is `None` for ordinary packets. Those packets keep
the existing `packet_meta` JSON shape, without a new null field. A normalized
segment uses the distinct wire tag `packet_meta_udp_segment_v1` and a required
`udp_offload` object:

| Field | Meaning |
| --- | --- |
| `aggregate_id` | Nonzero u64, canonical decimal string; one hook observation within a capture run, not a cross-host or wire ID |
| `aggregate_payload_len` | Original aggregate UDP payload length, excluding IP/UDP headers |
| `segment_size` | Nominal UDP payload size of each segment, excluding headers |
| `segment_index` | Zero-based segment position |
| `segment_count` | Ceiling of aggregate length divided by segment size, bounded to 2..16 |
| `ip_header_len` | IPv4 header length 20..60, aligned to four bytes, or direct IPv6 header length 40 |

`tot_len` is the **derived segment IP length**, not the aggregate length. The
last segment may be short. `ts_ms` is the aggregate hook-observation timestamp
shared by its segments, not separately measured wire-segment timestamps. The
adapter requires the complete bounded prefix, `min(segment_payload_len, 128)`,
and checks every cached byte/prefix alias against that contiguous byte array.
The original aggregate must fit ordinary IP/UDP length limits; fragments,
extensions and jumbograms are not introduced by this contract.

Use `CapturedUdpDatagram::into_offload_segment_fact` only after a producer has
established actual segmentation metadata and normalized the segment. It does
not infer segmentation from a large payload. Imports reject missing/unknown
provenance fields, out-of-range numbers, inconsistent lengths/prefixes, duplicate
payload offsets, and noncanonical v1 field shapes. Attaching provenance to an
ordinary wire tag is also rejected. Legacy readers reject the unknown v1 segment
tag instead of silently treating derived observations as ordinary packets. A
caller deleting both the tag and provenance is not detectable without trusted
producer authentication; this is validation, not a signature or trust upgrade.

Direct Rust runtime ingestion rejects malformed segment facts with
`invalid_udp_offload` and marks the export degraded. Direct Rust bundle replay
also validates before restoring stored summaries, so bypassing JSON cannot hide
a new segment rejection behind an old complete summary. Valid segment facts retain
their logical `PacketMeta` kind for existing protocol rules, and retain all
provenance through export/import and reanalysis. Record validation alone does
not prove that a complete group was received; the group layer below is separate.
Neither layer proves application delivery, absence of kernel loss, authenticity
or individual wire timing. Producer ABI integration and bounded kernel splitting
must still be verified before production normalization can be enabled.

The contract regressions are in `tests/udp_offload_provenance_tdd.rs`:

```bash
cargo test --locked -p gewyvern --test udp_offload_provenance_tdd
```

## Group Admission And Replay

`CaptureFactBuffer` is now the production collector's bounded staging buffer.
The BPF producer still emits ordinary records only; normalized group scenarios
are currently exercised through deterministic adapter/buffer tests, not claimed
as live GSO/GRO normalization.

- The first segment reserves the **whole group's** slots within `max_facts`.
  Ready ordinary records and reservations share this limit. A refused group
  cannot resume partway through after a service window frees capacity.
- Incomplete groups survive `take_facts()` and are never published piecemeal.
  A complete group moves to one analysis window atomically, even when segments
  arrive out of order or across separate consume calls.
- Repeated segment indexes, reused fact IDs within a pending group, and inconsistent hook identity or
  origin metadata reject the pending group. A complete, rejected or expired
  group's ID cannot reopen in a later window.
- Capture IDs are monotonically allocated within one run, allowing reordering
  within the most recent **4096 ID values**. Older arrivals are rejected, not
  trusted after cache eviction. The history and pending deadline index are
  bounded; this is not an unbounded lifetime deduplication set.
- A partial group has a fixed **two-second** deadline from its first arrival.
  Later arrivals do not renew it. Timeout, ID-window eviction and final shutdown
  release reservations and count only received-but-rejected records; they do
  not invent counts for unseen segments. `finish()` is idempotent and seals the
  buffer against further admission.

For each buffer input, exactly one of retained, userspace-budget-dropped,
decode-rejected or pending applies. Pending records are not called retained
before completion. After finish there are no pending records, so the existing
collector accounting equations remain valid. While running, pending records
may explain a temporary difference between received and classified counts;
`final_accounted` must remain false. This does not change the snapshot schema.
Kernel ring loss remains the producer's separate counter, not a guessed number
of missing group members.

Batch audits key groups by `(session, aggregate_id)` and require every index
exactly once, globally unambiguous fact IDs, matching timestamps/CPU/interface/fragment,
socket identity, direction, ports and origin metadata. Imported bundles and
direct Rust replay reject invalid groups. Changing the stored summary to
`degraded=true` does not waive this validation. Replay also rejects a stored
configuration that would filter away normalized segments before restoring old
summaries. Independent runs merged into one bundle need distinct session IDs.
Normalized fact IDs must not alias either another group's facts or ordinary
facts in that bundle; existing ordinary-only duplicate-ID behavior is unchanged.

Runtime analysis/export excludes every observed member of an incomplete,
duplicate or conflicting group and reports `incomplete_udp_offload`,
`duplicate_udp_offload` or `conflicting_udp_offload`. Existing per-record
rejection reasons remain visible. Nonconsuming runtime snapshots do not erase
the original pending observations: later completion can produce a healthy
snapshot, whereas the collector's actual loss counters remain cumulative.
Ordinary runtime sessions skip the group audit path.

```bash
cargo test --locked -p gewyvern --lib live_capture::fact_buffer::tests
cargo test --locked -p gewyvern --test udp_offload_provenance_tdd
```

## Reproduce

Use the [production prerequisites](live-udp-capture.md), iproute2 and bpftool.
Build without root, then run each mode with its own new output directory:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_OFFLOAD_GRO=false GEWY_OFFLOAD_OUT=/tmp/gewy-gso-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::offload::production_capture_udp_offload_boundary --nocapture
sudo env GEWY_OFFLOAD_GRO=true GEWY_OFFLOAD_OUT=/tmp/gewy-gso-gro-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::offload::production_capture_udp_offload_boundary --nocapture
```

Workers use persistent connected IPv4/IPv6 sockets in two private MTU-1500
network namespaces and one selected cgroup. Only these sockets enable
`UDP_SEGMENT` and `UDP_GRO`; no host-interface offload settings or sysctls change.
The native interfaces are described by Linux [udp(7)](https://man7.org/linux/man-pages/man7/udp.7.html).

## Acceptance

Four phases exercise ordinary traffic, selected-port offload, same-cgroup
unselected-port offload and ordinary same-socket recovery. One native send
contains three 1024-byte payloads and a distinct 17-byte tail. With GRO disabled,
the server must actually read four separately byte-checked datagrams. With GRO
enabled, it must actually read the joined 3089-byte buffer with a 1024-byte
`UDP_GRO` ancillary segment size; merely setting the option is insufficient.
Replies remain ordinary 32-byte datagrams. Fragmentation counters must stay zero.

Selected GSO egress cannot be exported as one giant UDP fact. When GRO is off,
the separately delivered server ingress datagrams remain valid observations.
When GRO is on, the aggregate ingress must also be rejected. Baseline, replies
and recovery retain exact socket/namespace/interface identity, ports, lengths
and bounded payloads. Unselected-port aggregates must contribute neither facts
nor unsupported counts. Replay and cleanup must preserve these boundaries.

## Defect And Repair

The [dated before/after proof](fixtures/gewyvern_product_udp_offload_linux_x86_64_20261120.json)
reproduces a real false-evidence defect. Before the repair, both modes exited 0
with `incomplete=false`. GSO-only exported two fictitious aggregate datagrams;
GSO+GRO exported four, with full IP lengths of 3117/3137 instead of distinct
UDP datagrams. Native peer I/O independently proved the segmentation/merging.

The fix filters the selected port, rejects offload metadata as `unsupported`,
then validates/adopts normal datagrams. Six final runs, three per mode, retain
144 valid observations and reject 18 aggregate hook observations. All six exit
3 with truthful incomplete capture, exact accounting and no ring, userspace or
decode drops. Same-scope offload on other ports is excluded. Native traffic
still completes 120 request datagrams and 48 replies across all phases/ports.
Fresh real fragmentation, receive-pressure and collector-lifecycle regressions
also pass. Failed reproductions and initial repaired passes are not final runs.

`unsupported` counts rejected aggregate observations, not missing wire packets
or segments. Accepted facts still have exact accounting; the run remains
incomplete after ordinary traffic resumes because its counters are cumulative.
A new ordinary-only run can be complete. Full selected-traffic evidence requires
ordinary per-datagram I/O until explicit offload normalization is implemented.

This repairs truthful rejection, not GSO/GRO normalization, hardware-NIC support,
wire-segment timestamps, full-payload capture or multi-host certification. Raw
exports/oracles stay in the recorded Linux evidence directory; compact fixtures
bind the accepted per-peer packet arrays by reproducible digests. No ABI schema,
protocol count, release artifact or status maturity is promoted.
