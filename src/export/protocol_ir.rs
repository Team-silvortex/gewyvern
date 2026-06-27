use super::{ExportError, JsonValue};
use crate::flow::{ProgramFlow, ProgramOperation};
use crate::protocol_profiles::{ProtocolSurfaceSummary, protocol_summaries, protocol_surface};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolIr {
    pub operation: String,
    pub protocol: String,
    pub entry: String,
    pub default_entry: String,
    pub selected_is_default: bool,
    pub sibling_entries: Vec<String>,
    pub cluster_key: Option<String>,
    pub cluster_label: Option<String>,
    pub shelf_key: Option<String>,
    pub shelf_label: Option<String>,
    pub semantics_category: Option<String>,
    pub operator_focus: Option<String>,
    pub typical_signal: Option<String>,
}

pub(crate) fn infer_protocol_ir(program_flows: &[ProgramFlow]) -> Vec<ProtocolIr> {
    let mut operations = program_flows
        .iter()
        .filter_map(|flow| operation_id(&flow.operation))
        .collect::<BTreeSet<_>>();
    let mut inferred = Vec::new();
    for summary in protocol_summaries() {
        for entry in summary.entries {
            let candidate = format!("{}_{}", summary.protocol, entry.mode.replace('-', "_"));
            if !operations.remove(&candidate) {
                continue;
            }
            if let Some(surface) = protocol_surface(&summary.protocol, &entry.mode) {
                inferred.push(protocol_ir_from_surface(candidate, surface));
            }
        }
    }
    inferred.sort_by(|left, right| left.operation.cmp(&right.operation));
    inferred
}

pub(crate) fn protocol_ir_json(ir: &ProtocolIr) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("operation".into(), JsonValue::String(ir.operation.clone())),
        ("protocol".into(), JsonValue::String(ir.protocol.clone())),
        ("entry".into(), JsonValue::String(ir.entry.clone())),
        (
            "default_entry".into(),
            JsonValue::String(ir.default_entry.clone()),
        ),
        (
            "selected_is_default".into(),
            JsonValue::Bool(ir.selected_is_default),
        ),
        ("sibling_entries".into(), string_array(&ir.sibling_entries)),
        ("cluster_key".into(), optional_string(&ir.cluster_key)),
        ("cluster_label".into(), optional_string(&ir.cluster_label)),
        ("shelf_key".into(), optional_string(&ir.shelf_key)),
        ("shelf_label".into(), optional_string(&ir.shelf_label)),
        (
            "semantics_category".into(),
            optional_string(&ir.semantics_category),
        ),
        ("operator_focus".into(), optional_string(&ir.operator_focus)),
        ("typical_signal".into(), optional_string(&ir.typical_signal)),
    ]))
}

pub(crate) fn parse_protocol_ir(value: &JsonValue) -> Result<ProtocolIr, ExportError> {
    let object = value.as_object()?;
    Ok(ProtocolIr {
        operation: required_string(object, "protocol_ir.operation")?,
        protocol: required_string(object, "protocol_ir.protocol")?,
        entry: required_string(object, "protocol_ir.entry")?,
        default_entry: required_string(object, "protocol_ir.default_entry")?,
        selected_is_default: object
            .get("selected_is_default")
            .ok_or_else(|| ExportError::InvalidShape("protocol_ir.selected_is_default".into()))?
            .as_bool()?,
        sibling_entries: object
            .get("sibling_entries")
            .unwrap_or(&JsonValue::Array(vec![]))
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        cluster_key: optional_string_value(object.get("cluster_key").unwrap_or(&JsonValue::Null))?,
        cluster_label: optional_string_value(
            object.get("cluster_label").unwrap_or(&JsonValue::Null),
        )?,
        shelf_key: optional_string_value(object.get("shelf_key").unwrap_or(&JsonValue::Null))?,
        shelf_label: optional_string_value(object.get("shelf_label").unwrap_or(&JsonValue::Null))?,
        semantics_category: optional_string_value(
            object.get("semantics_category").unwrap_or(&JsonValue::Null),
        )?,
        operator_focus: optional_string_value(
            object.get("operator_focus").unwrap_or(&JsonValue::Null),
        )?,
        typical_signal: optional_string_value(
            object.get("typical_signal").unwrap_or(&JsonValue::Null),
        )?,
    })
}

fn protocol_ir_from_surface(operation: String, surface: ProtocolSurfaceSummary) -> ProtocolIr {
    ProtocolIr {
        operation,
        protocol: surface.protocol,
        entry: surface.entry,
        default_entry: surface.default_entry,
        selected_is_default: surface.selected_is_default,
        sibling_entries: surface.sibling_entries,
        cluster_key: surface.cluster_hint.as_ref().map(|hint| hint.key.clone()),
        cluster_label: surface.cluster_hint.as_ref().map(|hint| hint.label.clone()),
        shelf_key: surface.shelf.as_ref().map(|shelf| shelf.key.clone()),
        shelf_label: surface.shelf.as_ref().map(|shelf| shelf.label.clone()),
        semantics_category: surface
            .entry_semantics
            .as_ref()
            .map(|semantics| semantics.category.clone()),
        operator_focus: surface
            .entry_semantics
            .as_ref()
            .map(|semantics| semantics.operator_focus.clone()),
        typical_signal: surface
            .entry_semantics
            .and_then(|semantics| semantics.typical_signal),
    }
}

fn operation_id(operation: &ProgramOperation) -> Option<String> {
    match operation {
        ProgramOperation::Custom(value) => Some(value.clone()),
        _ => None,
    }
}

fn string_array(items: &[String]) -> JsonValue {
    JsonValue::Array(
        items
            .iter()
            .map(|item| JsonValue::String(item.clone()))
            .collect(),
    )
}

fn optional_string(value: &Option<String>) -> JsonValue {
    value
        .as_ref()
        .map_or(JsonValue::Null, |value| JsonValue::String(value.clone()))
}

fn optional_string_value(value: &JsonValue) -> Result<Option<String>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::String(value) => Ok(Some(value.clone())),
        _ => Err(ExportError::InvalidShape("expected optional string".into())),
    }
}

fn required_string(
    object: &BTreeMap<String, JsonValue>,
    field: &str,
) -> Result<String, ExportError> {
    object
        .get(field.rsplit('.').next().unwrap_or(field))
        .ok_or_else(|| ExportError::InvalidShape(field.into()))?
        .as_str()
        .map(str::to_string)
}
