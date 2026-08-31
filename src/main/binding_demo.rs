use gewyvern::export::ExportBundle;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, QuicFrameType,
    QuicPacketType, SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::machine_error::{ErrorCategory, MachineError};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::TemplateBinding;
use std::time::{Duration, SystemTime};

use crate::{UiLocale, route_fact};

fn has_fragment(fragments: &[String], id: &str) -> bool {
    fragments.iter().any(|fragment| fragment == id)
}

pub(crate) fn try_run_binding_demo(binding: TemplateBinding) -> Result<ExportBundle, MachineError> {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let fragments = &binding.template.fragment_set;
    let tcp_demo_dport = binding
        .template
        .program_model
        .as_ref()
        .map(|model| match &model.operation {
            gewyvern::flow::ProgramOperation::Custom(value) if value == "postgres_connect" => 5432,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "redis_connect" => 6379,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "mysql_connect" => 3306,
            _ => 443,
        })
        .unwrap_or(443);
    let is_dns_lookup = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "dns_lookup"
            )
        });
    let is_http_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_request"
            )
        });
    let is_tls_client = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "tls_client"
            )
        });
    let is_http_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_server_response"
            )
        });
    let is_http3_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_request"
            )
        });
    let is_http3_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_server_response"
            )
        });
    let is_hy2_auth = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_auth"
            )
        });
    let is_hy2_udp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_udp_relay"
            )
        });
    let is_hy2_tcp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_tcp_relay"
            )
        });
    let is_socks5_session = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "socks5_session"
            )
        });
    let is_http_connect_tunnel = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_connect_tunnel"
            )
        });
    let facts = if has_fragment(fragments, "tcp_state_fragment")
        && has_fragment(fragments, "tcp_packet_meta_fragment")
        && has_fragment(fragments, "sock_lineage_fragment")
        && is_http_server_response
    {
        include!("binding_demo/http_server_response.rs")
    } else if has_fragment(fragments, "tcp_state_fragment")
        && has_fragment(fragments, "tcp_packet_meta_fragment")
        && has_fragment(fragments, "sock_lineage_fragment")
        && is_http_request
    {
        include!("binding_demo/http_request.rs")
    } else if has_fragment(fragments, "tcp_state_fragment")
        && has_fragment(fragments, "tcp_packet_meta_fragment")
        && has_fragment(fragments, "sock_lineage_fragment")
        && is_tls_client
    {
        include!("binding_demo/tls_client.rs")
    } else if has_fragment(fragments, "tcp_state_fragment")
        && has_fragment(fragments, "tcp_packet_meta_fragment")
    {
        include!("binding_demo/tcp_packet.rs")
    } else if has_fragment(fragments, "tcp_state_fragment")
        && has_fragment(fragments, "sock_lineage_fragment")
    {
        include!("binding_demo/tcp_sock_lineage.rs")
    } else if has_fragment(fragments, "udp_packet_meta_fragment")
        && has_fragment(fragments, "sock_lineage_fragment")
    {
        if is_http3_server_response {
            include!("binding_demo/udp_http3_server_response.rs")
        } else if is_http3_request {
            include!("binding_demo/udp_http3_request.rs")
        } else if is_hy2_tcp_relay {
            include!("binding_demo/udp_hy2_tcp_relay.rs")
        } else if is_hy2_udp_relay {
            include!("binding_demo/udp_hy2_udp_relay.rs")
        } else if is_hy2_auth {
            include!("binding_demo/udp_hy2_auth.rs")
        } else if is_socks5_session {
            include!("binding_demo/udp_socks5_session.rs")
        } else if is_http_connect_tunnel {
            include!("binding_demo/udp_http_connect_tunnel.rs")
        } else if is_dns_lookup {
            include!("binding_demo/udp_dns_lookup.rs")
        } else {
            include!("binding_demo/udp_sock_default.rs")
        }
    } else if has_fragment(fragments, "udp_packet_meta_fragment") {
        include!("binding_demo/udp_default.rs")
    } else {
        return Err(MachineError::new(
            "demo_fragment_combination_unsupported",
            ErrorCategory::Input,
            UiLocale::detect().msg("unsupported_fragment_combo"),
            false,
            2,
        ));
    };

    let config = SessionConfig::for_binding(binding).map_err(MachineError::from)?;
    let mut session = RuntimeSession::start(config).map_err(MachineError::from)?;
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);

    Ok(session.into_export_bundle())
}

#[cfg(test)]
pub(crate) fn run_binding_demo(binding: TemplateBinding) -> ExportBundle {
    try_run_binding_demo(binding).expect("test binding demo must be valid")
}
