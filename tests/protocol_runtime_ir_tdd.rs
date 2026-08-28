// Keep these cases in one integration crate so Cargo compiles shared fixtures and links once.
#[path = "support/mod.rs"]
mod support;

#[path = "protocol_runtime_cases/access_proxy_protocol_runtime_ir_tdd.rs"]
mod access_proxy_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/cache_coordination_protocol_runtime_ir_tdd.rs"]
mod cache_coordination_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/database_protocol_runtime_ir_tdd.rs"]
mod database_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/dhcpv6_protocol_runtime_ir_tdd.rs"]
mod dhcpv6_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/discovery_protocol_runtime_ir_tdd.rs"]
mod discovery_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/document_store_protocol_runtime_ir_tdd.rs"]
mod document_store_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/identity_access_protocol_runtime_ir_tdd.rs"]
mod identity_access_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/llmnr_protocol_runtime_ir_tdd.rs"]
mod llmnr_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/mail_protocol_runtime_ir_tdd.rs"]
mod mail_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/mailbox_auth_protocol_runtime_ir_tdd.rs"]
mod mailbox_auth_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/nbns_protocol_runtime_ir_tdd.rs"]
mod nbns_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/network_control_protocol_runtime_ir_tdd.rs"]
mod network_control_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/overlay_tunnel_protocol_runtime_ir_tdd.rs"]
mod overlay_tunnel_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/quic_http3_protocol_runtime_ir_tdd.rs"]
mod quic_http3_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/rip_protocol_runtime_ir_tdd.rs"]
mod rip_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/search_control_protocol_runtime_ir_tdd.rs"]
mod search_control_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/session_control_media_protocol_runtime_ir_tdd.rs"]
mod session_control_media_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/snmp_protocol_runtime_ir_tdd.rs"]
mod snmp_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/stream_messaging_protocol_runtime_ir_tdd.rs"]
mod stream_messaging_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/syslog_protocol_runtime_ir_tdd.rs"]
mod syslog_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/tftp_protocol_runtime_ir_tdd.rs"]
mod tftp_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/transport_web_protocol_runtime_ir_tdd.rs"]
mod transport_web_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/udp_control_protocol_runtime_ir_tdd.rs"]
mod udp_control_protocol_runtime_ir_tdd;
#[path = "protocol_runtime_cases/web_observability_protocol_runtime_ir_tdd.rs"]
mod web_observability_protocol_runtime_ir_tdd;
