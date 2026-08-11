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

## Leserpent 2 Named Benchmark Shelf

Run `gewyvern_validate leserpent-benchmark` to measure the bounded
Leserpent runtime, renderer-neutral UI IR, and release binary surfaces. The
shelf enforces broad disaster-regression budgets and retains exact measurements
for same-host-class trend comparison. Timing values from unrelated machines are
not directly comparable.

The fixed workload contains 16 fresh SQLite opens, 2,000 list queries over 256
runtimes, 10,000 effects inserted as batches of 100, and 100 iterations over a
1,539-node UI document. The UI phase measures document generation,
diff-plus-apply, and JSON encode-plus-decode. A separate .NET Release probe runs
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
minutes, and builds or suites after 30 minutes. Captured stdout and stderr are
each capped at 32 MiB. A blocked compiler, runaway output stream, build server,
or host integration therefore fails with the named phase and limit instead of
leaving a release job waiting indefinitely or exhausting operator memory.

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

For the active `1.14.x` line, the intended acceptance rule is:

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
| `benchmark_analysis_snapshot_large_protocol_flow_export` | `1217.617` | 200 iterations, 256 flows |
| `benchmark_analysis_snapshot_json_large_protocol_flow_export` | `139.603` | 200 iterations, precomputed snapshot |
| `benchmark_summary_json_large_protocol_flow_export` | `1328.418` | 200 iterations, single-target JSON surface |
| `benchmark_findings_json_large_protocol_flow_export` | `77.081` | 200 iterations, single-target findings JSON |
| `benchmark_scan_report_json_large_protocol_flow_export` | `6006.350` | 40 iterations, 24 targets, 256 flows each |
| `benchmark_scan_report_text_large_protocol_flow_export` | `6988.309` | 40 iterations, 24 targets, 256 flows each |
| `benchmark_scan_report_html_large_protocol_flow_export` | `1049.030` | 10 iterations, 12 targets, 256 flows each |
| `benchmark_http_transactions_json_large_view` | `155.222` | 200 iterations, 256 synthetic HTTP transactions |
| `benchmark_http_transactions_text_large_view` | `78.767` | 200 iterations, 256 synthetic HTTP transactions |

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
```

## Ubuntu Physical Host Remote Validation Baseline

Measurement notes:

- date: `2026-07-11`
- host: `kyuubiki-lab`, Ubuntu 24.04, Linux `6.17.0-35-generic`
- method: `cargo run --quiet --bin gewyvern_validate -- remote-linux-host-validation`
- admin-assisted eBPF path: `GEWY_REMOTE_EBPF_ADMIN_USER` +
  `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`
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
