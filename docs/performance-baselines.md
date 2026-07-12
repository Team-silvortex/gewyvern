# Performance Baselines

These are lightweight local baselines for the current ignored benchmark tests.
They now serve two purposes:

- a practical comparison point for day-to-day optimization work
- the current release-candidate acceptance baseline for `0.19.x` runtime and
  report performance

They are still not a promise that every machine will produce identical
numbers.

They are the agreed local reference point for deciding whether a change keeps
`gewyvern` within its currently accepted operational envelope.

Measurement notes:

- date: `2026-05-20`
- host: local developer machine
- method: `bash scripts/perf/benchmark_summary.sh 3 <benchmark-filter>`
- value to compare first: `median`

## Release-Candidate Interpretation

For the active `1.0.0` line, the intended acceptance rule is:

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
