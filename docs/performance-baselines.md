# Performance Baselines

These are lightweight local baselines for the current ignored benchmark tests.
They now serve two purposes:

- a practical comparison point for day-to-day optimization work
- the current release-candidate acceptance baseline for pre-`1.0` runtime and
  report performance

They are still not a promise that every machine will produce identical
numbers.

They are the agreed local reference point for deciding whether a change keeps
`gewyvern` within its currently accepted operational envelope.

Measurement notes:

- date: `2026-05-20`
- host: local developer machine
- method: `bash scripts/benchmark_summary.sh 3 <benchmark-filter>`
- value to compare first: `median`

## Release-Candidate Interpretation

For the current pre-`1.0` line, the intended acceptance rule is:

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

The expected workflow before calling a release candidate acceptable is:

1. run the targeted benchmark with `scripts/benchmark_summary.sh`
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

Recommended workflow:

1. Make one focused optimization change.
2. Run the targeted benchmark with `scripts/benchmark_summary.sh`.
3. Compare the new `median` against this table.
4. Only update this file when a new result is stable enough to be a useful team reference.

Suggested commands:

```bash
bash scripts/benchmark_summary.sh 3 benchmark_analysis_snapshot_large_protocol_flow_export
bash scripts/benchmark_summary.sh 3 benchmark_scan_report_
bash scripts/benchmark_summary.sh 3 benchmark_findings_json_large_protocol_flow_export
bash scripts/benchmark_summary.sh 3 benchmark_http_transactions_
```
