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
        .filter_map(|flow| {
            let operation = operation_id(&flow.operation)?;
            protocol_flow_has_required_phases(flow, &operation).then_some(operation)
        })
        .collect::<BTreeSet<_>>();
    let mut inferred = Vec::new();
    for summary in protocol_summaries() {
        for entry in summary.entries {
            let matched = operation_candidates(&summary.protocol, &entry.mode)
                .into_iter()
                .find(|candidate| operations.remove(candidate));
            let Some(operation) = matched else { continue };
            if let Some(surface) = protocol_surface(&summary.protocol, &entry.mode) {
                inferred.push(protocol_ir_from_surface(operation, surface));
            }
        }
    }
    inferred.sort_by(|left, right| left.operation.cmp(&right.operation));
    inferred
}

fn protocol_flow_has_required_phases(flow: &ProgramFlow, operation: &str) -> bool {
    let Some(required_phases) = required_protocol_ir_phases(operation) else {
        return true;
    };
    match required_phases {
        RequiredProtocolPhases::All(phases) => {
            phases.iter().all(|phase| flow_has_phase(flow, phase))
        }
        RequiredProtocolPhases::Any(phases) => {
            phases.iter().any(|phase| flow_has_phase(flow, phase))
        }
    }
}

enum RequiredProtocolPhases {
    All(&'static [&'static str]),
    Any(&'static [&'static str]),
}

fn flow_has_phase(flow: &ProgramFlow, phase: &str) -> bool {
    flow.stages
        .iter()
        .any(|stage| stage.phase.as_deref() == Some(phase))
}

fn required_protocol_ir_phases(operation: &str) -> Option<RequiredProtocolPhases> {
    match operation {
        "cassandra_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "coap_delete" => Some(RequiredProtocolPhases::All(&["receive_deleted"])),
        "coap_post" => Some(RequiredProtocolPhases::All(&["receive_created"])),
        "dhcp_discover" => Some(RequiredProtocolPhases::All(&["receive_offer"])),
        "dhcp_nak" => Some(RequiredProtocolPhases::All(&["receive_nak"])),
        "dhcp_request" => Some(RequiredProtocolPhases::All(&["receive_ack"])),
        "dns_error" => Some(RequiredProtocolPhases::All(&["receive_nxdomain"])),
        "dns_tcp_error" => Some(RequiredProtocolPhases::Any(&[
            "receive_formerr",
            "receive_servfail",
            "receive_nxdomain",
            "receive_refused",
        ])),
        "dns_tcp_query" => Some(RequiredProtocolPhases::All(&["receive_response"])),
        "dhcpv6_release" => Some(RequiredProtocolPhases::All(&["send_release"])),
        "dhcpv6_request" => Some(RequiredProtocolPhases::All(&["receive_reply"])),
        "dhcpv6_solicit" => Some(RequiredProtocolPhases::All(&[
            "send_solicit",
            "receive_advertise",
        ])),
        "ftp_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "grpc_call" => Some(RequiredProtocolPhases::All(&["receive_message"])),
        "http_connect_denied" => Some(RequiredProtocolPhases::All(&["receive_connect_denied"])),
        "icmp_unreachable" => Some(RequiredProtocolPhases::All(&["receive_unreachable"])),
        "icmpv6_unreachable" => Some(RequiredProtocolPhases::All(&["receive_unreachable"])),
        "imap_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "amqp_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_connection_close"])),
        "kerberos_as_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "ldap_bind_denied" => Some(RequiredProtocolPhases::All(&["receive_bind_denied"])),
        "ldap_modify_denied" => Some(RequiredProtocolPhases::All(&["receive_modify_denied"])),
        "llmnr_error" => Some(RequiredProtocolPhases::Any(&[
            "receive_formerr",
            "receive_servfail",
            "receive_nxdomain",
        ])),
        "llmnr_query" => Some(RequiredProtocolPhases::All(&["send_query"])),
        "llmnr_response" => Some(RequiredProtocolPhases::All(&["receive_response"])),
        "nbns_negative" => Some(RequiredProtocolPhases::Any(&[
            "receive_name_error",
            "receive_refused",
        ])),
        "nbns_query" => Some(RequiredProtocolPhases::All(&["send_query"])),
        "nbns_response" => Some(RequiredProtocolPhases::All(&["receive_response"])),
        "rip_request" => Some(RequiredProtocolPhases::All(&["send_request"])),
        "rip_response" => Some(RequiredProtocolPhases::All(&["receive_response"])),
        "rip_unreachable" => Some(RequiredProtocolPhases::All(&["receive_metric16"])),
        "mongodb_query_failure" => Some(RequiredProtocolPhases::All(&["receive_query_failure"])),
        "mssql_error" => Some(RequiredProtocolPhases::All(&["receive_error_token"])),
        "mysql_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "mysql_query_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "nats_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "ntp_query" => Some(RequiredProtocolPhases::All(&["receive_response"])),
        "ntp_sync" => Some(RequiredProtocolPhases::All(&[
            "send_sync_request",
            "receive_sync_response",
        ])),
        "otlp_export_error" => Some(RequiredProtocolPhases::All(&[
            "receive_error_headers",
            "receive_error_status",
        ])),
        "http3_close_observation" => Some(RequiredProtocolPhases::All(&[
            "send_request_stream",
            "receive_close",
        ])),
        "http3_server_close_observation" => Some(RequiredProtocolPhases::All(&[
            "send_response_stream",
            "send_close",
        ])),
        "quic_close_observation" => Some(RequiredProtocolPhases::All(&["receive_close"])),
        "quic_retry_validation" => Some(RequiredProtocolPhases::All(&["receive_retry"])),
        "radius_denied" => Some(RequiredProtocolPhases::All(&["receive_access_reject"])),
        "rdp_denied" => Some(RequiredProtocolPhases::Any(&[
            "receive_x224_disconnect",
            "receive_negotiation_failure",
        ])),
        "redis_wrongtype" => Some(RequiredProtocolPhases::All(&[
            "receive_wrongtype_constraint",
        ])),
        "snmp_bulk" => Some(RequiredProtocolPhases::All(&["receive_bulk_response"])),
        "snmp_inform" => Some(RequiredProtocolPhases::All(&["receive_inform_response"])),
        "snmp_set" => Some(RequiredProtocolPhases::All(&["receive_set_response"])),
        "snmp_v3_priv" => Some(RequiredProtocolPhases::All(&["receive_v3_priv_response"])),
        "sip_denied" => Some(RequiredProtocolPhases::Any(&[
            "receive_4xx",
            "receive_5xx",
            "receive_6xx",
        ])),
        "socks5_auth_connect_denied" => {
            Some(RequiredProtocolPhases::All(&["receive_connect_denied"]))
        }
        "socks5_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "socks5_denied" => Some(RequiredProtocolPhases::All(&["receive_connect_denied"])),
        "ssh_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "smtp_rcpt_denied" => Some(RequiredProtocolPhases::All(&["receive_rcpt_denied"])),
        "smtp_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "smtp_data_denied" => Some(RequiredProtocolPhases::All(&["receive_message_denied"])),
        "postgres_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "postgres_query_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "pop3_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_auth_denied"])),
        "stun_allocate" => Some(RequiredProtocolPhases::All(&["receive_allocate_response"])),
        "stun_binding_error" => Some(RequiredProtocolPhases::All(&["receive_error_response"])),
        "stun_refresh" => Some(RequiredProtocolPhases::All(&["receive_refresh_response"])),
        "syslog_tcp_message" => Some(RequiredProtocolPhases::All(&["send_syslog_frame"])),
        "syslog_tls_transport" => Some(RequiredProtocolPhases::All(&["send_tls_client_hello"])),
        "syslog_udp_message" => Some(RequiredProtocolPhases::All(&["send_syslog_message"])),
        "tftp_error" => Some(RequiredProtocolPhases::All(&["receive_error"])),
        "tftp_read" => Some(RequiredProtocolPhases::All(&["receive_data"])),
        "tftp_write" => Some(RequiredProtocolPhases::All(&["receive_ack"])),
        "tls_alert" => Some(RequiredProtocolPhases::Any(&[
            "send_alert",
            "receive_alert",
        ])),
        "tls_client" => Some(RequiredProtocolPhases::All(&["send_client_hello"])),
        "websocket_close" => Some(RequiredProtocolPhases::Any(&[
            "send_close",
            "receive_close",
        ])),
        "websocket_upgrade" => Some(RequiredProtocolPhases::All(&[
            "receive_switching_protocols",
        ])),
        "zookeeper_auth_denied" => Some(RequiredProtocolPhases::All(&["receive_denial"])),
        _ => None,
    }
}

fn operation_candidates(protocol: &str, entry: &str) -> Vec<String> {
    let canonical = format!("{}_{}", protocol, entry.replace('-', "_"));
    let aliases = match (protocol, entry) {
        ("dns", "udp") => &["dns_lookup"][..],
        ("dns", "tcp") => &["dns_tcp_query"][..],
        ("dns", "tcp-error") => &["dns_tcp_error"][..],
        ("http", "response") => &["http_server_response"][..],
        ("http", "connect") => &["http_connect_tunnel"][..],
        ("http", "denied") => &["http_connect_denied"][..],
        ("http", "auth-required") => &["http_connect_auth_required"][..],
        ("http", "auth-tunnel") => &["http_connect_authenticated_tunnel"][..],
        ("amqp", "start") => &["amqp_connection_start"][..],
        ("amqp", "publish") => &["amqp_basic_publish"][..],
        ("amqp", "consume") => &["amqp_basic_consume"][..],
        ("amqp", "session") => &["amqp_publish_session"][..],
        ("mysql", "connect") => &["mysql_connect"][..],
        ("mysql", "query") => &["mysql_simple_query"][..],
        ("mysql", "session") => &["mysql_query_session"][..],
        ("mysql", "error") => &["mysql_query_error"][..],
        ("postgres", "connect") => &["postgres_connect"][..],
        ("postgres", "query") => &["postgres_simple_query"][..],
        ("postgres", "session") => &["postgres_query_session"][..],
        ("postgres", "error") => &["postgres_query_error"][..],
        ("quic", "retry") => &["quic_retry_validation"][..],
        ("quic", "close") => &["quic_close_observation"][..],
        ("quic", "local-close") => &["quic_local_close_observation"][..],
        ("http3", "close") => &["http3_close_observation"][..],
        ("http3", "server-close") => &["http3_server_close_observation"][..],
        ("otlp", "traces") => &["otlp_traces_export"][..],
        ("otlp", "metrics") => &["otlp_metrics_export"][..],
        ("otlp", "logs") => &["otlp_logs_export"][..],
        ("dhcpv6", "release") => &["dhcpv6_release"][..],
        ("dhcpv6", "request") => &["dhcpv6_request"][..],
        ("dhcpv6", "solicit") => &["dhcpv6_solicit"][..],
        ("llmnr", "error") => &["llmnr_error"][..],
        ("llmnr", "query") => &["llmnr_query"][..],
        ("llmnr", "response") => &["llmnr_response"][..],
        ("nbns", "negative") => &["nbns_negative"][..],
        ("nbns", "query") => &["nbns_query"][..],
        ("nbns", "response") => &["nbns_response"][..],
        ("rip", "request") => &["rip_request"][..],
        ("rip", "response") => &["rip_response"][..],
        ("rip", "unreachable") => &["rip_unreachable"][..],
        ("syslog", "udp") => &["syslog_udp_message"][..],
        ("syslog", "tcp") => &["syslog_tcp_message"][..],
        ("syslog", "tls") => &["syslog_tls_transport"][..],
        ("ldap", "bind-denied") => &["ldap_bind_denied"][..],
        ("ldap", "denied") => &["ldap_modify_denied"][..],
        ("ldap", "constraint") => &["ldap_modify_constraint_violation"][..],
        ("ldap", "session") => &["ldap_directory_session"][..],
        ("ldap", "write") => &["ldap_directory_write_session"][..],
        ("ldap", "sync") => &["ldap_directory_sync_session"][..],
        ("ssh", "channel") => &["ssh_channel_session"][..],
        _ => &[],
    };
    std::iter::once(canonical)
        .chain(aliases.iter().map(|alias| (*alias).to_string()))
        .collect()
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
