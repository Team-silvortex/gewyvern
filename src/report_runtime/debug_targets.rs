use super::*;

pub(super) fn ranked_targets<'a>(
    outputs: &'a [(String, ExportBundle)],
    analyses: &'a [AnalysisSnapshot],
) -> Vec<(&'a str, &'a AnalysisSnapshot)> {
    let mut indexed = outputs
        .iter()
        .zip(analyses.iter())
        .map(|((name, _), analysis)| (name.as_str(), analysis))
        .collect::<Vec<_>>();
    indexed.sort_by(|(left_name, left), (right_name, right)| {
        rank(left)
            .cmp(&rank(right))
            .then_with(|| left_name.cmp(right_name))
    });
    indexed
}

pub(super) fn scan_target_protocol_entry(name: &str) -> Option<(&str, &str)> {
    let mut parts = name.splitn(3, ':');
    if parts.next()? != "scan" {
        return None;
    }
    Some((parts.next()?, parts.next()?))
}

pub(super) fn rank(analysis: &AnalysisSnapshot) -> usize {
    match analysis.automation_outcome.as_str() {
        "targeted_escalation" => 0,
        "collect_more_evidence" => 1,
        "multi_hypothesis" => 2,
        "manual_review" => 3,
        "advisory_only" => 4,
        _ => match analysis.evidence_posture.as_str() {
            "direct_protocol_signal" => 0,
            "missing_transition" => 1,
            "ambiguous_multi_hypothesis" => 2,
            "heuristic_summary" => 3,
            _ => 5,
        },
    }
}
