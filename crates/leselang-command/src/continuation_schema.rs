//! Released Leserpent continuation vocabulary, not a host-neutral VM format.

use crate::ResultBinding;

pub const CONTINUATION_SCHEMA_VERSION: u32 = 1;
pub const RESULT_BINDING_CONTINUATION_SCHEMA_VERSION: u32 = 2;
pub const SUCCESSOR_CONTINUATION_SCHEMA_VERSION: u32 = 3;
pub const DATAFLOW_CONTINUATION_SCHEMA_VERSION: u32 = 4;
pub const CONDITIONAL_CONTINUATION_SCHEMA_VERSION: u32 = 5;
pub const GROUP_RESULT_CONTINUATION_SCHEMA_VERSION: u32 = 6;
pub const GROUP_TAIL_CONTINUATION_SCHEMA_VERSION: u32 = 7;
pub const GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION: u32 = 8;
pub const PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION: u32 = 9;
pub const GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION: u32 = 10;
pub const GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION: u32 = 11;

/// Product compatibility refusal, without saved payloads or VM fault codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationSchemaError {
    Group,
    Version { actual: u32, expected: u32 },
}

impl std::fmt::Display for ContinuationSchemaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Group => formatter.write_str("incompatible continuation group"),
            Self::Version { actual, expected } => write!(
                formatter,
                "unsupported continuation version {actual}, expected {expected}"
            ),
        }
    }
}

impl std::error::Error for ContinuationSchemaError {}

/// Check only the released predecessor/successor version relationship.
///
/// Group owners are compared verbatim; callers must already have admitted both
/// frames, including canonical owner spelling. This does not validate captures,
/// inherited authority, budgets, monotonic identities, replies or durable state.
pub fn successor_schema_matches(
    prior_version: u32,
    prior_group_owner: Option<&str>,
    next_version: u32,
    next_group_owner: Option<&str>,
) -> bool {
    match prior_version {
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION => next_version == CONTINUATION_SCHEMA_VERSION,
        DATAFLOW_CONTINUATION_SCHEMA_VERSION => matches!(
            next_version,
            CONTINUATION_SCHEMA_VERSION | DATAFLOW_CONTINUATION_SCHEMA_VERSION
        ),
        CONDITIONAL_CONTINUATION_SCHEMA_VERSION => matches!(
            next_version,
            CONTINUATION_SCHEMA_VERSION
                | DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ),
        GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
        | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION => {
            next_version == prior_version
                && prior_group_owner.is_some()
                && prior_group_owner == next_group_owner
        }
        _ => false,
    }
}

/// Select the released schema for an already structurally validated result flow.
///
/// Call `validate_result_binding_structure` before entry for untrusted bindings;
/// body classification assumes bounded IR. This does not validate lexical types,
/// captures, group ownership, authority or restoration eligibility.
pub fn preflighted_result_binding_schema_version(binding: &ResultBinding) -> u32 {
    if !binding.groups.is_empty() {
        if binding.body.is_pure() {
            GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
        } else if binding.body.is_result_chain() {
            GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
        } else {
            GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        }
    } else if !binding.body.is_pure() && !binding.body.is_result_chain() {
        CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    } else if !binding.results.is_empty()
        || (!binding.body.is_pure() && !binding.body.is_atomic_tail())
    {
        DATAFLOW_CONTINUATION_SCHEMA_VERSION
    } else if binding.body.is_pure() {
        RESULT_BINDING_CONTINUATION_SCHEMA_VERSION
    } else {
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION
    }
}

/// Check the released version/group relationship after binding structure preflight.
///
/// The caller bounds ingress and validates any binding's structure before entry.
/// `max_sequence` is explicit policy, including zero/full-width values; no VM
/// defaults are supplied. The group spelling is checked, but no identifier is
/// allocated or authenticated. Legacy group-only versions keep their historical
/// compatibility; success alone never grants dispatch or recovery authority.
pub fn validate_preflighted_continuation_schema(
    binding: Option<&ResultBinding>,
    group_owner: Option<&str>,
    actual_version: u32,
    max_sequence: u64,
) -> Result<(), ContinuationSchemaError> {
    let expected = if let Some(owner) = group_owner {
        let sequence = owner
            .strip_prefix("merge-")
            .and_then(|value| value.parse::<u64>().ok());
        if binding.is_some_and(|binding| {
            !matches!(
                actual_version,
                GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
                    | PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
                    | GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
                    | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
            ) || binding.groups.is_empty()
                || !(binding.body.is_pure()
                    || (actual_version == GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
                        && binding.body.is_result_chain())
                    || (actual_version == GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
                        && binding.body.is_result_flow()))
        }) || sequence.is_none_or(|value| {
            value == 0 || value > max_sequence || owner != format!("merge-{value}")
        }) {
            return Err(ContinuationSchemaError::Group);
        }
        match actual_version {
            GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION => {
                GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
            }
            GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION => {
                GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
            }
            PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION => {
                PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
            }
            GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION => GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION,
            GROUP_TAIL_CONTINUATION_SCHEMA_VERSION => GROUP_TAIL_CONTINUATION_SCHEMA_VERSION,
            _ => GROUP_RESULT_CONTINUATION_SCHEMA_VERSION,
        }
    } else if let Some(binding) = binding {
        if !binding.groups.is_empty() {
            return Err(ContinuationSchemaError::Group);
        }
        preflighted_result_binding_schema_version(binding)
    } else {
        CONTINUATION_SCHEMA_VERSION
    };
    if actual_version != expected {
        return Err(ContinuationSchemaError::Version {
            actual: actual_version,
            expected,
        });
    }
    Ok(())
}
