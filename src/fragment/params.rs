use crate::template::FragmentParamValue;

use super::{FragmentParamSpec, FragmentParamType};

pub(super) fn fragment_param_type_matches(
    spec: &FragmentParamSpec,
    value: &FragmentParamValue,
) -> bool {
    if spec.key == "sample_payload_offsets" {
        return matches!(
            value,
            FragmentParamValue::String(_) | FragmentParamValue::U64(_)
        );
    }
    matches!(
        (&spec.value_type, value),
        (FragmentParamType::Bool, FragmentParamValue::Bool(_))
            | (FragmentParamType::U64, FragmentParamValue::U64(_))
            | (FragmentParamType::String, FragmentParamValue::String(_))
    )
}

impl FragmentParamType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U64 => "u64",
            Self::String => "string",
        }
    }
}
