use std::fmt;

use crate::MAX_GROUP_GRAPH_MEMBERS;

/// Borrowed original or successor bindings, not a validated frame or authority.
pub struct CaptureExtensionBindings<'a, Group, ResultData> {
    pub groups: &'a [Group],
    pub results: &'a [ResultData],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCapture {
    pub matches_result: bool,
    pub matches_binding: bool,
}

/// Native data equality, named-binding equality and current-result projection.
/// Comparisons may short-circuit on their first match. `begin_results` runs once
/// after both group passes, even with no saved results. It must not execute a body.
pub trait CaptureExtensionDomain<Group, ResultData> {
    type Error;

    fn same_group_data(&mut self, old: &Group, saved: &Group) -> bool;
    fn same_group_binding(&mut self, old: &Group, saved: &Group) -> bool;
    fn begin_results(&mut self) -> Result<(), Self::Error>;
    fn current_result(&mut self, saved: &ResultData) -> Result<CurrentCapture, Self::Error>;
    fn same_result_data(&mut self, old: &ResultData, saved: &ResultData) -> bool;
    fn same_result_binding(&mut self, old: &ResultData, saved: &ResultData) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureExtensionSite {
    BeginResults,
    Result(usize),
}

pub enum CaptureExtensionError<Native> {
    InvalidLimit,
    OriginalCount,
    SavedCount,
    UnknownGroup {
        index: usize,
    },
    DroppedGroup {
        index: usize,
    },
    UnknownResult {
        index: usize,
    },
    MissingCurrent,
    DroppedResult {
        index: usize,
    },
    Native {
        site: CaptureExtensionSite,
        error: Native,
    },
}

impl<Native> fmt::Display for CaptureExtensionError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "capture extension limit exceeds safety ceiling",
            Self::OriginalCount => "capture extension original count exceeds policy",
            Self::SavedCount => "capture extension saved count exceeds policy",
            Self::UnknownGroup { .. } => "successor group has no original data",
            Self::DroppedGroup { .. } => "successor omitted an original group binding",
            Self::UnknownResult { .. } => "successor result is neither current nor original",
            Self::MissingCurrent => "successor omitted the current result binding",
            Self::DroppedResult { .. } => "successor omitted an original result binding",
            Self::Native { .. } => "capture extension native observation failed",
        })
    }
}
impl<Native> fmt::Debug for CaptureExtensionError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for CaptureExtensionError<Native> {}

/// Inspect a saved-capture extension without allocation or payload copies.
/// The explicit inclusive aggregate limit (ceiling 64) applies independently to
/// original and saved groups+results, before any hook. Native names, duplicates,
/// arity and data/schema admission remain host policy.
///
/// First reject invented group data, then require every original named group.
/// After native result preparation, observe every saved result in order, even
/// after finding the current binding. A non-current result must match original
/// data; aliases are allowed. Require the current named binding before checking
/// exact retention of every original named result. Membership scans stop on the
/// first match, preserving the native comparison order. Refusal/unwind stops
/// later hooks, never retries, and never normalizes or mutates original data.
///
/// Payloads/errors need no Clone/Debug/Eq/serde/Default/Send/Sync bounds. The domain
/// owner releases before result/error handoff; native errors move unchanged with
/// redacted diagnostics and no Error::source. Native side effects/cleanup cannot
/// be rolled back or sandboxed; a second cleanup panic may abort.
///
/// Success is transient retention, not provenance, receipt authenticity or an
/// execution grant. Original identities, bodies, current authority, budget/schema
/// succession, complete graph recovery and same-snapshot commit-before-publish
/// remain host-owned. No evaluation, fuel construction, dispatch or storage writes.
pub fn validate_capture_extension<Group, ResultData, Domain>(
    original: CaptureExtensionBindings<'_, Group, ResultData>,
    saved: CaptureExtensionBindings<'_, Group, ResultData>,
    max_bindings: usize,
    mut domain: Domain,
) -> Result<(), CaptureExtensionError<Domain::Error>>
where
    Domain: CaptureExtensionDomain<Group, ResultData>,
{
    let outcome = (|| {
        if max_bindings > MAX_GROUP_GRAPH_MEMBERS {
            return Err(CaptureExtensionError::InvalidLimit);
        }
        if original
            .groups
            .len()
            .checked_add(original.results.len())
            .is_none_or(|count| count > max_bindings)
        {
            return Err(CaptureExtensionError::OriginalCount);
        }
        if saved
            .groups
            .len()
            .checked_add(saved.results.len())
            .is_none_or(|count| count > max_bindings)
        {
            return Err(CaptureExtensionError::SavedCount);
        }
        for (index, group) in saved.groups.iter().enumerate() {
            if !original
                .groups
                .iter()
                .any(|old| domain.same_group_data(old, group))
            {
                return Err(CaptureExtensionError::UnknownGroup { index });
            }
        }
        for (index, old) in original.groups.iter().enumerate() {
            if !saved
                .groups
                .iter()
                .any(|group| domain.same_group_binding(old, group))
            {
                return Err(CaptureExtensionError::DroppedGroup { index });
            }
        }
        domain
            .begin_results()
            .map_err(|error| CaptureExtensionError::Native {
                site: CaptureExtensionSite::BeginResults,
                error,
            })?;
        let mut contains_current = false;
        for (index, result) in saved.results.iter().enumerate() {
            let current =
                domain
                    .current_result(result)
                    .map_err(|error| CaptureExtensionError::Native {
                        site: CaptureExtensionSite::Result(index),
                        error,
                    })?;
            contains_current |= current.matches_result && current.matches_binding;
            if !current.matches_result
                && !original
                    .results
                    .iter()
                    .any(|old| domain.same_result_data(old, result))
            {
                return Err(CaptureExtensionError::UnknownResult { index });
            }
        }
        if !contains_current {
            return Err(CaptureExtensionError::MissingCurrent);
        }
        for (index, old) in original.results.iter().enumerate() {
            if !saved
                .results
                .iter()
                .any(|result| domain.same_result_binding(old, result))
            {
                return Err(CaptureExtensionError::DroppedResult { index });
            }
        }
        Ok(())
    })();
    drop(domain);
    outcome
}
