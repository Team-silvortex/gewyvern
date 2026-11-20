//! Coordinates real private targets, captured evidence and deterministic replay.
//! Fixture policy, resource ownership and verification have separate owners.
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture_at, export_stage_at, network_namespace,
    owned_socket_on, port, prepare_output, snapshot, tool, wait_state,
};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
mod catalog;
mod evidence;
mod memcached;
mod mqtt;
mod mysql;
mod nats;
mod pop3;
mod rtsp;
mod smtp;
mod target;
mod wire;

#[cfg(test)]
mod tests;

use catalog::{Group, Protocol};
use evidence::CapturedEvidence;
use target::Target;

fn phase(out: &Path, protocol: Protocol) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "data-breadth")?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut target = Target::start(protocol, &token, &out)?;
    let (source, destination) = (target.source, target.destination);
    let mut scope = Scope::enter(&token)?;
    let sockets = [owned_socket_on(source)?, owned_socket_on(source)?];
    let cookies = [sockets[0].cookie()?, sockets[1].cookie()?];
    let netns = network_namespace()?;
    let expected = [
        (cookies[0], port(&sockets[0])?, protocol.port(), netns),
        (cookies[1], port(&sockets[1])?, protocol.port(), netns),
    ];
    let addresses = (source.octets(), destination.octets());
    let (mut states_capture, clock) = attach_capture_at(cookies, &out, addresses)?;
    let packets_capture = Capture::attach_data_breadth(&scope, &expected, addresses, &out)?;
    let packet_ownership = (
        packets_capture.path().to_owned(),
        packets_capture.link_id()?,
    );
    let mut responses = Vec::new();
    for (index, socket) in sockets.into_iter().enumerate() {
        let (response, code) = wire::exchange(socket, target.address, protocol, index == 1)?;
        wait_state(
            &states_capture,
            &out,
            &format!("closed-{index}"),
            cookies[index],
            7,
        )?;
        fs::write(out.join(format!("client-response-{index}.bin")), &response)?;
        responses.push((response, code));
    }
    let neighbor = owned_socket_on(source)?;
    let neighbor_cookie = neighbor.cookie()?;
    let (_, neighbor_code) = wire::exchange(neighbor, target.address, protocol, false)?;
    target.finish_traffic();
    states_capture.detach();
    let (states, state_counts) = snapshot(&states_capture, &out, "final")?;
    let tcp = export_stage_at(
        &states,
        state_counts,
        &expected,
        clock,
        &out,
        "final",
        addresses,
    )?;
    let (packets, packet_counts) = packets_capture.finish(&out)?;
    if packets.iter().any(|p| p.cookie == neighbor_cookie)
        || states.iter().any(|s| s.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "unselected database/cache neighbor entered capture",
        ));
    }
    let evidence::EvidenceReport {
        observations,
        packages,
        package_failures,
    } = CapturedEvidence {
        tcp: &tcp,
        states: &states,
        packets: &packets,
        clock,
        cookies,
    }
    .verify(protocol, &responses, &out)?;
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        &out,
    )?)?;
    if packet_ownership.0.exists() || links.iter().any(|link| link["id"] == packet_ownership.1) {
        return Err(ValidationError::new(
            "database/cache packet link survived cleanup",
        ));
    }
    states_capture.cleanup()?;
    scope.cleanup()?;
    target.cleanup(&out)?;
    let report = json!({"schema_version":2,"protocol":protocol.name(),"ok":package_failures.is_empty(),"image":protocol.image(),
        "image_id":target.image_id,"container_id":target.container_id,"network_id":target.network_id,
        "internal_network":true,"published_ports":false,"selected_cookies":cookies,
        "unselected_neighbor_cookie":neighbor_cookie,"unselected_neighbor_result":neighbor_code,
        "client_read_chunk_bytes":wire::READ_CHUNK,"client_reads_are_packet_boundaries":false,
        "selected_query_or_command_count":protocol.selected_command_count(),
        "redis_pipeline_commands":if matches!(protocol, Protocol::Redis) {3} else {0},
        "observations":observations,"packages":packages,"packet_counts":packet_counts,
        "tcp_state_counts":state_counts,"owned_resources_removed":true,"anonymous_volume_ids":target.volumes,
        "anonymous_volumes_removed_with_container":true,"package_failures":package_failures});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    if !package_failures.is_empty() {
        return Err(ValidationError::new(package_failures.join("; ")));
    }
    Ok(report)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Data)
}

pub(super) fn run_mysql_memcached(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::MysqlMemcached)
}

pub(super) fn run_messaging(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Messaging)
}
pub(super) fn run_smtp(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Smtp)
}

pub(super) fn run_pop3(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Pop3)
}

pub(super) fn run_rtsp(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Rtsp)
}

fn run_group(out: Option<PathBuf>, group: Group) -> Result<ValidationReport, ValidationError> {
    let (mode, protocols) = group.selection();
    let out = prepare_output(out, "linux-data-breadth")?;
    let mut phases = Vec::new();
    for &protocol in protocols {
        eprintln!("capture protocol breadth: {}", protocol.name());
        phases.push(phase(&out.join(protocol.name()), protocol)?);
    }
    super::http_boundaries::run(Some(out.join("after-data-restart")))?;
    let checks = group.checks();
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":2,"ok":true,
        "mode":mode,"phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "not_proven":["TLS_decryption","arbitrary_protocol_detection","general_TCP_reassembly",
            "vulnerability_exploitation_detection","PostgreSQL_TLS_SCRAM_extended_query","Redis_cluster_RESP3","MySQL_caching_sha2_auth_switch_prepared_statements","Memcached_text_meta_SASL_UDP",
            "MQTT5_authentication_TLS_offline_delivery_or_crash_durability","NATS_JetStream_cluster_headers_TLS",
            "SMTP_STARTTLS_delivery_to_external_mailbox_or_crash_durability",
            "POP3_TLS_APOP_SASL_external_delivery_or_delete_commit_durability",
            "RTSP_TLS_authentication_RTP_delivery_quality_UDP_multicast_or_general_stream_decoding",
            "PID_or_container_process_attribution","request_payload_capture","IPv6_or_UDP","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: format!("Linux real {mode} capture breadth"),
        out_dir: out,
        checks: checks.iter().map(|check| (*check).to_owned()).collect(),
    })
}
