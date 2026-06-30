use super::*;

fn precomputed_large_scan_inputs(
    target_count: usize,
) -> (Vec<(String, ExportBundle)>, Vec<AnalysisSnapshot>) {
    let outputs = synthesize_large_scan_outputs(target_count);
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    (outputs, analyses)
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_json_precomputed_analysis_large_protocol_flow_export() {
    let (outputs, analyses) = precomputed_large_scan_inputs(24);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..40 {
        total_len += scan_report_json_with_analyses(&outputs, &analyses).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_json_precomputed_analysis_large_protocol_flow_export: \
         iterations=40 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_text_precomputed_analysis_large_protocol_flow_export() {
    let (outputs, analyses) = precomputed_large_scan_inputs(24);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..40 {
        total_len += scan_report_text_with_analyses(&outputs, &analyses).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_text_precomputed_analysis_large_protocol_flow_export: \
         iterations=40 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_html_precomputed_analysis_large_protocol_flow_export() {
    let (outputs, analyses) = precomputed_large_scan_inputs(12);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..10 {
        total_len += scan_report_html_with_analyses(&outputs, &analyses).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_html_precomputed_analysis_large_protocol_flow_export: \
         iterations=10 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}
