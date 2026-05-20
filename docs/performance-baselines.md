# Performance Baselines

These are lightweight local baselines for the current ignored benchmark tests.
They are intended as a practical comparison point for day-to-day optimization
work, not as a strict release gate.

Measurement notes:

- date: `2026-05-20`
- host: local developer machine
- method: `bash scripts/benchmark_summary.sh 3 <benchmark-filter>`
- value to compare first: `median`

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
