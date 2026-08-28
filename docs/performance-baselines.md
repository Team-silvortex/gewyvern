# Performance Baselines

These are lightweight local baselines for the current ignored benchmark tests.
They now serve two purposes:

- a practical comparison point for day-to-day optimization work
- the comparison baseline established during `0.19.x` and retained for
  `1.0.0` runtime and report performance

They are still not a promise that every machine will produce identical
numbers.

They are the agreed local reference point for deciding whether a change keeps
`gewyvern` within its currently accepted operational envelope.

## Native Build Workflow Baseline

The `2026-08-28` macOS arm64 calibration used an 8-logical-CPU, 16 GiB host,
Rust `1.94.1`, locked dependencies, fresh target directories, and
`CARGO_INCREMENTAL=0`. This isolates compiler work from a preheated incremental
cache; day-to-day rebuilds should be faster.

| Rust path | Wall time | User CPU | Target allocation |
| --- | ---: | ---: | ---: |
| Full-debug workspace build | `35.89 s` | `138.45 s` | `1.2 GiB` |
| Line-table workspace build | `35.08 s` | `108.92 s` | `942 MiB` |
| Line-table workspace check | `15.59 s` | `44.93 s` | not comparable to linked output |

The lighter profile reduced compiler CPU by `21.3%` and physical target usage
by roughly one fifth; parallel critical-path work limited the cold wall-time
change to `2.3%`. Skipping Rust code generation and linking through
`cargo dev check` reduced cold wall time by `56.6%`. A separately cleaned .NET
comparison reduced the control stage from `5.86 s` for the solution, which also
compiled test projects, to `4.69 s` for the production control project
(`20.0%`). Full `cargo dev build`, workspace tests, and release gates remain the
required paths whenever runnable artifacts or release evidence are needed.

The complete cross-stack command took `22.619 s` when the control project first
needed a locked restore (`17.25 s` of that stage). Its immediate warm rerun
correctly selected `--no-restore`: Rust check, control, and desktop finished in
`0.184 / 0.757 / 1.102 s`, with `1.106 s` total parallel wall time.
After the runnable Rust artifacts were populated, the corresponding no-change
`cargo dev build` reused all three stacks in `1.065 s`.

## Leserpent 2 Named Benchmark Shelf

Run `gewyvern_validate leserpent-benchmark` to measure the bounded
Leserpent runtime, renderer-neutral UI IR, and release binary surfaces. The
shelf enforces broad disaster-regression budgets and retains exact measurements
for same-host-class trend comparison. Timing values from unrelated machines are
not directly comparable.

The fixed workload contains 16 fresh SQLite opens, 2,000 list queries over 256
runtimes, 10,000 effects inserted as batches of 100, and 100 iterations over a
1,539-node UI document. The UI phase measures document generation, diff,
apply, JSON encode, and JSON decode separately while retaining the combined
diff-plus-apply and encode-plus-decode metrics. A separate .NET Release probe runs
500 iterations comparing a 256-log full workspace compose with an 8-log
incremental compose-and-merge while retaining a 256-entry result. The .NET
workload uses a proof-local artifacts root, so it can run beside parity,
accessibility, or AOT without sharing project intermediates. It also builds the native
`leserpent` and `leserpentd` release binaries and applies a 32 MiB
per-binary ceiling.

The human-readable command reports five numbered phases before starting each
workload; `--json` mode suppresses those progress lines so stdout remains a
machine contract. Shared Cargo subprocesses are bounded at 30 minutes and the
small .NET workspace-log phase is bounded at 5 minutes. The AOT,
accessibility, parity/recovery, and locked .NET proof subprocesses now use the
same bounded runner: tool probes stop after 30 seconds, GUI fixtures after 5
minutes, and builds or suites after 30 minutes. Native field, demo, lifecycle,
and operator socket helpers stop after 30 seconds; the external-analysis demo
stops after 5 minutes. Captured stdout and stderr are each capped at 32 MiB. A
blocked compiler, runaway output stream, build server, or host integration
therefore fails with the named phase and limit instead of leaving a release job
waiting indefinitely or exhausting operator memory.

Current `2026-07-18` references:

| Host | Cold open p95 | List p50 | 10k enqueue | UI document p50 | UI patch p50 | UI codec p50 | CLI / daemon |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| macOS arm64 | `13.411 ms` | `0.111 ms` | `310.687 ms` / `32.2k/s` | `1.540 ms` | `17.314 ms` | `4.411 ms` | `2.51 / 5.65 MiB` |
| Linux x86_64 | `14.982 ms` | `1.078 ms` | `376.523 ms` / `26.6k/s` | `0.458 ms` | `4.649 ms` | `1.281 ms` | `1.07 / 3.85 MiB` |

Evidence lives under `target/validation/leserpent-benchmark/` and the
physical Linux copy under
`target/validation/leserpent-benchmark-linux-x64/`.

A `2026-08-11` macOS arm64 run after the bounded-subprocess hardening passed
the complete shelf: SQLite cold-open p95 was `20.190 ms`, runtime-list p50 was
`0.095 ms`, and 10,000 effects took `382.030 ms` (`26.2k/s`). The language
pipeline p50 was `0.255 ms`; UI document/patch/codec p50 values were
`1.184 / 2.510 / 3.323 ms`. Incremental workspace-log projection took
`0.212 ms`, `15.6%` of the full path, with a `9.5%` allocation ratio. The CLI
and daemon binaries were `3,792,000 / 9,312,784` bytes.

A `2026-08-24` macOS arm64 allocation pass changed successful UI document and
patch validation to borrow node identifiers instead of allocating temporary
owned identifiers for every visited node. Across seven release-process samples
of the fixed workload, document generation fell from `0.742750` to `0.576750 ms`
(`22.3%`), patch-plus-apply from `1.586750` to `0.934958 ms` (`41.1%`), and
codec from `2.075792` to `1.839459 ms` (`11.4%`). The benchmark now also emits
standalone diff, apply, encode, and decode timings so later work can identify
which half of a combined path changed.

The same date's follow-up pass removed allocation-heavy identity checks and
duplicate continuation semantic validation from the 64-branch Leselang VM
start path. Across seven release-process samples, VM start fell from `0.125125` to
`0.113417 ms` (`9.4%`) and the complete language pipeline from `0.182000` to
`0.164209 ms` (`9.8%`). The runtime effect queue now reuses cached SQLite
SELECT and INSERT statements inside each atomic enqueue batch. Ten alternating
same-host baseline/optimized pairs reduced the 10,000-effect median from
`167.844` to `115.426 ms` (`31.2%`) and raised median throughput from `59.6k/s`
to `86.6k/s`; this paired result is preferred over isolated runs because local
filesystem commit latency remains visibly noisy.

The core report pass on the same date made external-analysis snapshot JSON
lazy when no sidecar engine is configured and stopped allocating duplicate
profile strings during aggregation. Ten alternating same-host binary pairs
reduced the 200-snapshot median from `1101.081` to `862.733 ms` (`21.6%`). Scan
reports now resolve every target against one per-report protocol-registry
snapshot instead of recursively scanning and parsing the registry once per
target. A preserved pre-change binary took `39385.371 ms` for one precomputed
24-target JSON run; the optimized path has a seven-run median of `1626.549 ms`
(`95.9%` lower). The complete recomputing JSON path moved from a same-host
pre-change sample of `41389.666 ms` to a five-run median of `5737.565 ms`
(`86.1%` lower). This is request-local snapshot reuse, not a stale process-wide
cache, so registry changes remain visible to the next report. Service
publication carries the same surfaces through text, JSON, HTML, per-target
reports, and the API snapshot, and writes the selected CLI format without a
second analysis pass.

The `2026-08-24` GewyLang compiler pass reuses one immutable builtin fragment
registry, validates from the diagnostics already computed for the envelope,
and transfers parser/lowering values instead of cloning them between stages.
Ten alternating same-host baseline/optimized binary pairs reduced the binding
report median from `89.792` to `65.170 ms` (`27.4%`), explain from `44.422` to
`35.959 ms` (`19.1%`), and the complete envelope from `44.768` to `37.238 ms`
(`16.8%`). The frontend-only median remained effectively neutral at
`44.746/44.425 ms`; package lockfile generation was not changed by this pass.

The same date's runtime-ledger pass replaces the full stable sort after every
accepted fact with an append fast path and a stable binary-insertion fallback
for genuinely out-of-order IDs. Ten alternating same-host test-binary pairs on
8,192 ordered facts reduced the ingest median from `299.352` to `0.795 ms`
(`99.7%`). Duplicate IDs retain arrival order, and out-of-order input remains
sorted. Flow reconstruction now borrows repeated fragment-source identifiers
before cloning the first occurrence; across 50 reconstructions of the same
8,192-fact, 256-flow input, the paired median fell from `149.925` to
`139.078 ms` (`7.2%`).

The follow-up evidence-reconstruction pass builds one borrowed FactId index and
uses it across program-flow and reason-chain construction, so each flow visits
only its own evidence instead of rescanning the complete ledger. The index
retains duplicate IDs and original input order, including for unsorted public
API inputs, without cloning fact envelopes. Ten alternating same-host
test-binary pairs reduced 10 program-flow reconstructions over 8,192 facts and
256 flows from `2897.740` to `72.931 ms` (`97.5%`), and 10 reason-chain
reconstructions from `2791.941` to `76.548 ms` (`97.3%`). Runtime export shares
the same index between both reconstruction stages.

The next complete-export pass avoids scanning the protocol registry when the
reconstructed flows use built-in operations that cannot produce protocol IR.
Custom protocol operations still resolve against the complete registry, but now
reuse one request-local summary snapshot for matching and surface projection.
Finding rules also prepare shared attach/rejection evidence once, and module
aggregation borrows grouping keys instead of cloning them twice. Ten alternating
same-host test-binary pairs reduced 10 complete exports of 7,936 facts across 256
process flows, each missing its required route stage, from `563.778` to
`188.872 ms` (`66.5%`). Every export retained 256 program flows, findings,
module findings, and reason chains; protocol IR correctly remained empty.

The following materialized-view pass relies on the runtime session invariant
that accepted facts are already window-filtered, letting flow, reason, and
export reconstruction borrow the authoritative ledger without cloning it.
Flow aggregation uses hash lookup followed by an explicit cookie sort, and
low-cardinality fragment sources are deduplicated in place before their final
deterministic sort. Ten alternating same-host binary pairs reduced direct flow
reconstruction from `142.079` to `119.015 ms` (`16.2%`), session flow snapshots
from `205.771` to `129.556 ms` (`37.0%`), and repeated borrowed exports from
`190.847` to `184.901 ms` (`3.1%`). A consuming export API additionally moves
one-shot session state into the bundle; against a baseline that already
contained the flow changes, its paired median fell from `186.518` to
`177.098 ms` (`5.1%`). Borrowed and consuming exports are parity-tested, and
production one-shot paths use the consuming form.

The diagnosis-projection pass then combines phase collection and last-phase
selection into one traversal while retaining the finding-first terminal-check
short circuit. Failure mode, detail, basis, and confidence share one normalized
stage/module context, with confidence derived directly from the already selected
basis. Process aggregation also borrows `(pid, comm)` lookup keys and allocates
its attention state only on the first transition. Ten alternating frozen/current
binary pairs reduced 200 analyses of the 256-flow workload from `1010.249` to
`881.959 ms` (`12.7%`); three alternating pairs reduced the complete 24-target
JSON scan report from `6842.968` to `6328.286 ms` (`7.5%`). The Codex host process
was concurrently above 150% CPU during these final samples, so they establish
the paired improvement but do not replace the quieter-host absolute regression
floors in the table below.

The `2026-07-18` macOS arm64 hybrid-log reference measured the 256-entry full
compose at `2.099 ms` p50 and `321,424` allocated bytes per iteration. The
8-entry incremental compose-and-merge measured `0.299 ms` and `30,488` bytes,
for timing/allocation ratios of `0.142 / 0.095`, while preserving a 256-entry
result. These ratios are same-host signals, not cross-machine promises.

The `2026-07-28` macOS arm64 UI diff optimization adds an O(n) path for
unchanged node topology while retaining the general insert/remove/move
algorithm. On the fixed 1,539-node, two-operation workload, patch-plus-apply p50
fell from `14.536 ms` to a three-run range of `2.104-2.242 ms` (median
`2.110 ms`, about 85.5% lower). The benchmark now requires exactly two patch
operations and caps patch p50 at four times document-generation p50, preventing
the former O(n²) behavior from hiding under the broad absolute budget.

The same date adds a first-class Leselang language workload: a 5,371-byte,
1,674-token program containing the maximum 64 declared `all` branches, sampled
500 times across parsing, HIR lowering, ephemeral VM start, and the complete
pipeline. Three alternating detached-HEAD/current pairs produced medians of
`0.0778/0.0451 ms` for parse, `0.0422/0.0285 ms` for HIR,
`0.4283/0.2076 ms` for VM start, and `0.5229/0.2974 ms` end to end. The
respective reductions are about 42%, 33%, 52%, and 43%. The optimized paths
avoid per-character decoding for ordinary unescaped strings, replace
per-entry tree allocations during name deduplication, and validate ephemeral
continuation encoding size without materializing discarded JSON bytes.

Measurement notes:

- date: `2026-05-20`
- host: local developer machine
- method: `bash scripts/perf/benchmark_summary.sh 3 <benchmark-filter>`
- value to compare first: `median`

## Release-Candidate Interpretation

For the active `1.20.x` line, the intended acceptance rule is:

- compare against the `median`
- judge regressions on the same developer-class machine, not across unrelated
  hosts
- treat small single-run noise as expected
- treat sustained or clearly visible regressions in the main hot paths as RC
  blockers until explained or accepted on purpose

The hot paths that matter most are:

- `benchmark_analysis_snapshot_large_protocol_flow_export`
- `benchmark_summary_json_large_protocol_flow_export`
- `benchmark_scan_report_json_large_protocol_flow_export`
- `benchmark_scan_report_text_large_protocol_flow_export`
- `benchmark_scan_report_html_large_protocol_flow_export`
- `benchmark_runtime_ingest_ordered_8192_facts`
- `benchmark_flow_reconstruction_8192_facts`
- `benchmark_runtime_flow_snapshots_8192_facts`
- `benchmark_program_flow_reconstruction_8192_facts`
- `benchmark_reason_chain_reconstruction_8192_facts`
- `benchmark_runtime_export_missing_route_7936_facts`
- `benchmark_runtime_one_shot_export_missing_route_7936_facts`

For `gewylang` / `gewyc`, the first useful compiler-facing benchmark family is:

- `benchmark_gewyc_binding_report_udp_process_debug`
- `benchmark_gewyc_frontend_report_udp_process_debug`
- `benchmark_gewyc_explain_report_udp_process_debug`
- `benchmark_gewyc_envelope_report_udp_process_debug`
- `benchmark_gewyc_lockfile_protocol_publish_package`

The expected workflow before calling a release candidate acceptable is:

1. run the targeted benchmark with `scripts/perf/benchmark_summary.sh`
2. compare the new `median` against this table
3. if the result is materially worse, either fix it or explicitly accept the
   regression with intent
4. only then refresh this file

Current baselines:

| Benchmark | Median (ms) | Notes |
| --- | ---: | --- |
| `benchmark_analysis_snapshot_large_protocol_flow_export` | `862.733` | 200 iterations, 256 flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_analysis_snapshot_json_large_protocol_flow_export` | `139.603` | 200 iterations, precomputed snapshot |
| `benchmark_summary_json_large_protocol_flow_export` | `1009.468` | 200 iterations, single-target JSON surface, seven samples |
| `benchmark_findings_json_large_protocol_flow_export` | `77.081` | 200 iterations, single-target findings JSON |
| `benchmark_scan_report_json_large_protocol_flow_export` | `5737.565` | 40 iterations, 24 targets, 256 flows each, five samples |
| `benchmark_scan_report_json_precomputed_analysis_large_protocol_flow_export` | `1626.549` | 40 iterations, 24 targets, shared analysis and registry snapshots, seven samples |
| `benchmark_scan_report_text_large_protocol_flow_export` | `5684.507` | 40 iterations, 24 targets, 256 flows each, five samples |
| `benchmark_scan_report_html_large_protocol_flow_export` | `939.069` | 10 iterations, 12 targets, 256 flows each, seven samples |
| `benchmark_http_transactions_json_large_view` | `155.222` | 200 iterations, 256 synthetic HTTP transactions |
| `benchmark_http_transactions_text_large_view` | `78.767` | 200 iterations, 256 synthetic HTTP transactions |
| `benchmark_gewyc_binding_report_udp_process_debug` | `65.170` | 200 iterations, median of 10 optimized samples from alternating binary A/B |
| `benchmark_gewyc_frontend_report_udp_process_debug` | `44.425` | 200 iterations, median of 10 optimized samples; neutral against the paired baseline |
| `benchmark_gewyc_explain_report_udp_process_debug` | `35.959` | 100 iterations, median of 10 optimized samples from alternating binary A/B |
| `benchmark_gewyc_envelope_report_udp_process_debug` | `37.238` | 100 iterations, median of 10 optimized samples from alternating binary A/B |
| `benchmark_gewyc_lockfile_protocol_publish_package` | `3.013` | 100 iterations, unchanged path retained as the current same-host observation |
| `benchmark_runtime_ingest_ordered_8192_facts` | `0.795` | 8,192 ordered facts, median of 10 optimized samples from alternating binary A/B |
| `benchmark_flow_reconstruction_8192_facts` | `119.015` | 50 reconstructions of 8,192 facts across 256 flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_runtime_flow_snapshots_8192_facts` | `129.556` | 50 session snapshots of 8,192 materialized facts across 256 flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_program_flow_reconstruction_8192_facts` | `72.931` | 10 reconstructions of 8,192 facts across 256 flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_reason_chain_reconstruction_8192_facts` | `76.548` | 10 reconstructions of 8,192 facts across 256 flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_runtime_export_missing_route_7936_facts` | `184.901` | 10 borrowed complete exports, 7,936 facts, 256 missing-route process flows, median of 10 optimized samples from alternating binary A/B |
| `benchmark_runtime_one_shot_export_missing_route_7936_facts` | `177.098` | 10 consuming one-shot exports from prebuilt sessions, 7,936 facts and 256 missing-route process flows each, median of 10 optimized samples from alternating binary A/B |

## Ubuntu Physical Host Scan-Report Check

Measurement notes:

- date: `2026-06-30`
- host: `kyuubiki-lab`, Ubuntu 24.04, Linux `6.17.0-35-generic`
- Rust/Cargo: `1.95.0`
- method: `bash scripts/perf/benchmark_summary.sh 3 <benchmark-filter>`
- value to compare first: `median`

These numbers are not interchangeable with the local developer-machine
baseline above. Use them as the current physical-host reference for scan-report
hot paths and for checking whether precomputed analysis is being reused.
They predate the `2026-08-24` per-report registry snapshot optimization and
must be refreshed on the next physical-host run before they are used as a
post-change regression floor.

| Benchmark | Median (ms) | Notes |
| --- | ---: | --- |
| `benchmark_analysis_snapshot_large_protocol_flow_export` | `1186.634` | 200 iterations, 256 flows, accumulator-local profile scoring |
| `benchmark_scan_report_html_large_protocol_flow_export` | `3790.273` | 10 iterations, 12 targets, recomputes analysis, compact scan-all flow details |
| `benchmark_scan_report_html_precomputed_analysis_large_protocol_flow_export` | `2999.616` | 10 iterations, 12 targets, reuses analysis, compact scan-all flow details |
| `benchmark_scan_report_json_large_protocol_flow_export` | `29426.387` | 40 iterations, 24 targets, recomputes analysis, compact scan-all flow details |
| `benchmark_scan_report_json_precomputed_analysis_large_protocol_flow_export` | `23378.290` | 40 iterations, 24 targets, reuses analysis, compact scan-all flow details |
| `benchmark_scan_report_text_large_protocol_flow_export` | `29153.123` | 40 iterations, 24 targets, recomputes analysis, compact scan-all flow details |
| `benchmark_scan_report_text_precomputed_analysis_large_protocol_flow_export` | `23219.876` | 40 iterations, 24 targets, reuses analysis, compact scan-all flow details |

Recommended workflow:

1. Make one focused optimization change.
2. Run the targeted benchmark with `scripts/perf/benchmark_summary.sh`.
3. Compare the new `median` against this table.
4. Only update this file when a new result is stable enough to be a useful team reference.

Suggested commands:

```bash
bash scripts/perf/benchmark_summary.sh 3 benchmark_analysis_snapshot_large_protocol_flow_export
bash scripts/perf/benchmark_summary.sh 3 benchmark_scan_report_
bash scripts/perf/benchmark_summary.sh 3 benchmark_findings_json_large_protocol_flow_export
bash scripts/perf/benchmark_summary.sh 3 benchmark_http_transactions_
bash scripts/perf/benchmark_summary.sh 3 benchmark_gewyc_
bash scripts/perf/benchmark_summary.sh 3 benchmark_runtime_ingest_
bash scripts/perf/benchmark_summary.sh 3 benchmark_flow_reconstruction_
bash scripts/perf/benchmark_summary.sh 3 benchmark_runtime_flow_snapshots_
bash scripts/perf/benchmark_summary.sh 3 benchmark_program_flow_reconstruction_
bash scripts/perf/benchmark_summary.sh 3 benchmark_reason_chain_reconstruction_
bash scripts/perf/benchmark_summary.sh 3 benchmark_runtime_export_missing_route_
bash scripts/perf/benchmark_summary.sh 3 benchmark_runtime_one_shot_export_missing_route_
```

## Ubuntu Physical Host Remote Validation Baseline

Measurement notes:

- date: `2026-07-11`
- host: `kyuubiki-lab`, Ubuntu 24.04, Linux `6.17.0-35-generic`
- method: `cargo run --quiet --bin gewyvern_validate -- remote-linux-host-validation`
- admin-assisted eBPF attach: `GEWY_REMOTE_EBPF_ADMIN_USER` +
  `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`; current validation keeps workspace and
  build ownership on the ordinary host alias identity
- result: package smoke, runtime smoke, and remote eBPF smoke all passed
- default-route device for tc smoke: `wlp3s0`
- mode: warm remote source cache, warm package cache, warm SSH control path

These numbers are the current end-to-end physical-host reference for the
native remote validation shelf. They are not a substitute for the benchmark
tables above; they answer a different question: whether the real Linux release
path is getting slower or less reliable as the 1.0 core hardens.

| Phase | Seconds | Notes |
| --- | ---: | --- |
| `remote_preflight` | `0.557` | Linux/x86_64 capability and command snapshot |
| `remote_workspace_create` | `0.102` | creates remote workspace roots |
| `workspace_sync` | `0.324` | sync-key probe and rsync cache decision |
| `remote_workspace_materialize` | `0.110` | requested workspace repointed at source cache |
| `remote_package_build` | `0.401` | cached package build wrapper |
| `remote_artifact_verify` | `0.120` | DEB/RPM discovery |
| `remote_package_smoke` | `0.264` | packaged install/runtime shell |
| `remote_runtime_smoke` | `0.698` | host-mode runtime validation |
| `remote_ebpf_smoke` | `1.414` | tracepoint + kprobe + tc attach evidence |
| `remote_ebpf_evidence_sync` | `0.496` | syncs remote eBPF shelf back locally |
| `remote_workspace_cleanup` | `0.114` | removes transient remote workspace |
| `total` | `4.599` | full remote validation wall-clock time |

A publication-hardening verification on `2026-07-18` forced an incremental
Linux release rebuild after changing the loader and package builder. It passed
DEB/RPM construction, strict manifest resolution, package smoke, TCP/UDP
runtime smoke, and all three admin-assisted eBPF attach probes in `22.110`
seconds. The dominant phase was `remote_package_build=18.267`; workspace sync
was `0.957` and eBPF attach was `0.827`. This is a rebuild-path observation,
not a replacement for the warm-cache baseline above.

After adding the Linux all-target compile proof, a second `2026-07-18` physical
run completed in `30.908` seconds. `remote_linux_target_check=8.232` compiled
the filtered workspace's Linux targets before the release build;
`remote_package_build=18.849` remained dominant. Package/runtime smoke and all
eBPF probes stayed green, confirming the extra compile shelf composes with the
full release path.

The completed root-integration-target run finished in `26.190` seconds after
syncing the full 1.6 MiB test shelf. Workspace sync was `2.454`; the incremental
Linux all-target check no longer appeared among the three slowest phases, while
`remote_package_build=18.638` remained dominant. This is the current stronger
compile-coverage observation.

### Identity-Isolated Full-Stack Recheck

On `2026-08-24`, the physical `gewyvern-lab` alias resolved to an Ubuntu
`x86_64` host running Linux `7.0.0-28-generic` with no detected virtualization.
The run used the ordinary key-backed SSH identity for preflight, shared caches,
builds, evidence synchronization, and cleanup. No admin credentials were
present; the root-owned fixed helper handled only attach/kprobe/tc. Ownership
fences before materialization and after evidence collection found zero foreign
UID/GID entries, and no transient remote-run directory remained.

| Phase | Seconds | Notes |
| --- | ---: | --- |
| `workspace_sync` | `0.110` | unchanged filtered snapshot, warm source cache |
| `remote_rust_quality` | `0.620` | locked all-target clippy shelf |
| `remote_package_build` | `0.504` | manifest-bound warm package artifacts |
| `remote_leserpent_control_plane_aot` | `39.696` | packaged control-plane NativeAOT proof |
| `remote_leserpent_language_pack_local_orchestra_aot` | `53.081` | packaged Local Orchestra and saved-daemon proof |
| `remote_ebpf_attach` | `0.563` | fixed-helper tracepoint, kprobe, and tc proof |
| `total` | `96.275` | full current remote validation transaction |

No timing budget warning fired. This recheck proves repeatability and identity
isolation, but it does not increase matrix breadth: retained evidence still
contains one independent physical host fingerprint and one kernel release. The
bounded, secret-free record is
`docs/fixtures/gewyvern_remote_linux_workspace_identity_physical_20260824.json`.

### Ubuntu 22.04 VM Kernel Compatibility Recheck

Also on `2026-08-24`, the isolated `gewyvern-jammy` compatibility shelf ran the
same full transaction on Ubuntu 22.04, Linux `5.15.0-187-generic`, `x86_64`,
and KVM. Its first preflight rejected an installed `1.15.0` privileged helper
as incompatible with the current `1.16.0` package. After replacing only the
root-owned helper and root-only provisioner, the ordinary validation account
retained no unrestricted sudo and could invoke only `probe`, `run`, and
`cleanup` through the fixed sudoers contract.

| Phase | Seconds | Notes |
| --- | ---: | --- |
| `workspace_sync` | `0.106` | unchanged filtered snapshot, isolated VM cache |
| `remote_rust_quality` | `0.352` | locked all-target clippy shelf |
| `remote_package_build` | `0.452` | manifest-bound warm DEB/RPM artifacts |
| `remote_leserpent_control_plane_aot` | `44.565` | Linux-x64 control-plane NativeAOT proof |
| `remote_leserpent_language_pack_local_orchestra_aot` | `60.122` | Local Orchestra and saved-daemon NativeAOT proof |
| `remote_ebpf_attach` | `0.720` | fixed-helper tracepoint, kprobe, and tc proof |
| `total` | `107.865` | full VM compatibility transaction |

All 19 checks passed without a timing warning. The result is deliberately
reported as `compatibility_only`: its matrix has `release_eligible=false`, so
the additional `5.15` kernel evidence cannot satisfy or inflate the physical
host release gate. The bounded, secret-free record is
`docs/fixtures/gewyvern_remote_linux_vm_kernel_compatibility_20260824.json`.

### Ubuntu 22.04 HWE Package And Reboot Compatibility

Later on `2026-08-24`, the same isolated VM exercised a stateful deployment
lifecycle rather than another cache-only rerun. A native `1.16.0-1` DEB was
built from the validated release binaries, byte-compared with its payload,
installed, and verified with `dpkg -V`. The VM then rebooted from Linux
`5.15.0-187-generic` to `5.15.0-190-generic`, passed the packaged helper's
attach/kprobe/tc cycle, installed the official Jammy HWE meta-package, and
rebooted again into `6.8.0-138-generic`. Both 5.15 kernels remain available as
GRUB fallback entries.

The package lifecycle also exposed and fixed builder-umask-dependent payload
modes. The final archive contains 453 `0755` directories, exactly five `0755`
command entry points, and 1,569 `0644` shared files; it contains no symlink,
special, set-id, group/world-writable, or accidentally executable data entry.
After both reboots, the helper configuration remained root-owned, the sudoers
rule remained `0440`, and the ordinary account still had access only to
`probe`, `run`, and `cleanup`.

| Phase | Seconds | Notes |
| --- | ---: | --- |
| `workspace_sync` | `0.730` | changed compiler and packaging snapshot |
| `remote_rust_quality` | `7.272` | locked all-target clippy shelf |
| `remote_package_build` | `28.539` | `22.252` second release relink plus package assembly |
| `remote_leserpent_control_plane_aot` | `53.577` | Linux-x64 control-plane NativeAOT proof |
| `remote_leserpent_language_pack_local_orchestra_aot` | `63.720` | Local Orchestra and saved-daemon NativeAOT proof |
| `remote_ebpf_attach` | `0.756` | HWE tracepoint, kprobe, and tc proof |
| `total` | `159.755` | full HWE VM compatibility transaction |

All 19 checks passed. The package phase exceeded the `20s` warm-cache budget
because this run intentionally relinked changed Rust binaries, so its signal is
`watch`; that is a performance follow-up, not a functional failure. VM history
now contains one KVM host with successful `5.15` and `6.8` kernels, while
`release_eligible=false` and the physical matrix remains exactly one host and
one kernel. The bounded record is
`docs/fixtures/gewyvern_remote_linux_vm_hwe_compatibility_20260824.json`.

The synchronized evidence for this baseline lives under:

- `target/validation/remote-linux-host-validation/`
- `target/validation/remote-linux-host-validation/remote-ebpf/`

Suggested comparison workflow:

1. Rerun `remote-linux-host-validation` after a core Linux/eBPF change.
2. Compare `total` first, then `workspace_sync`, `remote_runtime_smoke`, and
   `remote_ebpf_smoke`.
3. If `remote_ebpf_smoke` regresses, inspect `remote-ebpf/*/environment.txt`,
   `run.log`, and `netdev.txt` before blaming the loader itself.
4. Check `remote-ebpf-history.jsonl` or `remote-ebpf-latest.json` to see
   whether the regression is a one-off or part of a recent pattern.
5. Use `remote-ebpf-recent.txt` or `remote-ebpf-status-summary.json` when you
   want the recent pattern without manually slicing the raw JSONL file.

Current soft warning budgets reflected by the CLI/JSON remote summary:

- `total <= 45s`
- `workspace_sync <= 8s`
- `remote_package_build <= 20s`
- `remote_package_smoke <= 2s`
- `remote_runtime_smoke <= 3s`
- `remote_ebpf_smoke <= 10s`
- `remote_ebpf_evidence_sync <= 5s`
