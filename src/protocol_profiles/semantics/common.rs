use crate::protocol_profiles::ProtocolEntrySemanticsSummary;

pub(super) fn summary(
    category: &str,
    operator_focus: &str,
    typical_signal: Option<&str>,
    primary_failure_mode: Option<&str>,
    primary_failure_detail: Option<&str>,
    primary_failure_basis: Option<&str>,
) -> Option<ProtocolEntrySemanticsSummary> {
    Some(ProtocolEntrySemanticsSummary {
        category: category.into(),
        operator_focus: operator_focus.into(),
        typical_signal: typical_signal.map(str::to_string),
        primary_failure_mode: primary_failure_mode.map(str::to_string),
        primary_failure_detail: primary_failure_detail.map(str::to_string),
        primary_failure_basis: primary_failure_basis.map(str::to_string),
    })
}

pub(super) fn failure(
    operator_focus: &str,
    typical_signal: Option<&str>,
    failure_mode: Option<&str>,
    failure_detail: Option<&str>,
) -> Option<ProtocolEntrySemanticsSummary> {
    summary(
        "failure-path",
        operator_focus,
        typical_signal,
        failure_mode,
        failure_detail,
        Some("direct_protocol_signal"),
    )
}
