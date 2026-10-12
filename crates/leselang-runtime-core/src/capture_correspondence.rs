use std::fmt;

use crate::MAX_GROUP_GRAPH_MEMBERS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureCorrespondenceLimits {
    pub max_saved_bindings: usize,
    pub max_original_rows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureCorrespondenceSite {
    Group(usize),
    Result { capture: usize, original: usize },
}

pub enum CaptureCorrespondenceError<Native> {
    InvalidLimits,
    BindingCount,
    OriginalCount,
    GroupMismatch {
        index: usize,
    },
    ResultMismatch {
        index: usize,
    },
    Native {
        site: CaptureCorrespondenceSite,
        error: Native,
    },
}

impl<Native> fmt::Display for CaptureCorrespondenceError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "capture correspondence limit exceeds safety ceiling",
            Self::BindingCount => "capture correspondence saved count exceeds policy",
            Self::OriginalCount => "capture correspondence original count exceeds policy",
            Self::GroupMismatch { .. } => "saved group differs from original results",
            Self::ResultMismatch { .. } => "saved result has no original correspondence",
            Self::Native { .. } => "capture correspondence native observation failed",
        })
    }
}
impl<Native> fmt::Debug for CaptureCorrespondenceError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for CaptureCorrespondenceError<Native> {}

/// Borrow original saved groups/results and a complete original result prefix.
/// Explicit inclusive aggregate saved/original limits (each ceiling 64) precede
/// every hook, bounding work to groups + results * original rows without allocation.
/// Empty lists are structurally allowed; language arity and required captures are
/// native policy. Admit groups once in original order, then each saved result's
/// complete original-row scan in capture-major order. Even after a true match,
/// later rows still run: their native errors must not be hidden by a shortcut.
/// A wholly unmatched capture refuses before the next capture. Any native refusal
/// or unwind stops later hooks without retry. No sorting or normalization occurs.
///
/// All payloads remain borrowed and need no Clone/Debug/Eq/serde/Default/Send/Sync
/// bounds. Native errors move unchanged with payload-free diagnostics and no
/// Error::source. Both hook owners release before result/error handoff. Native
/// side effects/cleanup cannot be rolled back or sandboxed; a second panic may abort.
///
/// This is transient correspondence, not receipt authenticity, a graph certificate
/// or execution grant. Native hooks own schema/version, names, operation/value
/// domains, exact group signatures and raw-result projection policy. Hosts also
/// own original identity/captures/current authority and the same storage snapshot
/// with commit-before-publish. No body execution, fuel, dispatch, replay or writes.
pub fn validate_capture_correspondence<Group, ResultData, Row, Native>(
    groups: &[Group],
    results: &[ResultData],
    originals: &[Row],
    limits: CaptureCorrespondenceLimits,
    mut match_group: impl FnMut(usize, &Group) -> Result<bool, Native>,
    mut match_result: impl FnMut(CaptureCorrespondenceSite, &ResultData, &Row) -> Result<bool, Native>,
) -> Result<(), CaptureCorrespondenceError<Native>> {
    let outcome = (|| {
        if limits.max_saved_bindings > MAX_GROUP_GRAPH_MEMBERS
            || limits.max_original_rows > MAX_GROUP_GRAPH_MEMBERS
        {
            return Err(CaptureCorrespondenceError::InvalidLimits);
        }
        if groups
            .len()
            .checked_add(results.len())
            .is_none_or(|count| count > limits.max_saved_bindings)
        {
            return Err(CaptureCorrespondenceError::BindingCount);
        }
        if originals.len() > limits.max_original_rows {
            return Err(CaptureCorrespondenceError::OriginalCount);
        }
        for (index, group) in groups.iter().enumerate() {
            let site = CaptureCorrespondenceSite::Group(index);
            if !match_group(index, group)
                .map_err(|error| CaptureCorrespondenceError::Native { site, error })?
            {
                return Err(CaptureCorrespondenceError::GroupMismatch { index });
            }
        }
        for (capture, result) in results.iter().enumerate() {
            let mut matched = false;
            for (original, row) in originals.iter().enumerate() {
                let site = CaptureCorrespondenceSite::Result { capture, original };
                matched |= match_result(site, result, row)
                    .map_err(|error| CaptureCorrespondenceError::Native { site, error })?;
            }
            if !matched {
                return Err(CaptureCorrespondenceError::ResultMismatch { index: capture });
            }
        }
        Ok(())
    })();
    drop(match_group);
    drop(match_result);
    outcome
}
