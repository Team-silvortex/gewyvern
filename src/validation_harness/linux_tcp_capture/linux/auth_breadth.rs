//! Real, private FTP/LDAP targets: client oracle -> kernel bytes -> package rules -> replay.
use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::super::packet_evidence::{PacketEvent, reconstruct_packets_with_rules};
use super::docker_target::{OwnedResource, docker, target_addresses};
use super::http_evidence::sha256;
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture_at, export_stage_at, network_namespace,
    owned_socket_on, port, prepare_output, repo_root, snapshot, tool, wait_state,
};
use crate::export::ExportBundle;
use crate::ledger::FactKind;

mod wire;

#[derive(Clone, Copy)]
enum Protocol {
    Ftp,
    Ldap,
}

impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Self::Ftp => "ftp",
            Self::Ldap => "ldap",
        }
    }
    fn port(self) -> u16 {
        match self {
            Self::Ftp => 21,
            Self::Ldap => 389,
        }
    }
    fn code(self, accepted: bool) -> u16 {
        match (self, accepted) {
            (Self::Ftp, false) => 530,
            (Self::Ftp, true) => 230,
            (Self::Ldap, false) => 49,
            (Self::Ldap, true) => 0,
        }
    }
    fn image(self) -> &'static str {
        match self {
            Self::Ftp => {
                "fauria/vsftpd@sha256:6d71d7c7f1b0ab2844ec7dc7999a30aef6d758b6d8179cf5967513f87c79c177"
            }
            Self::Ldap => {
                "osixia/openldap@sha256:18742e9c449c9c1afe129d3f2f3ee15fb34cc43e5f940a20f3399728f41d7c28"
            }
        }
    }
    fn rules(self) -> [(&'static str, &'static str, &'static str, [bool; 2]); 2] {
        match self {
            Self::Ftp => [
                (
                    "protocols/ftp/denied/main.gewy",
                    include_str!("../../../../protocols/ftp/denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/ftp/session/main.gewy",
                    include_str!("../../../../protocols/ftp/session/main.gewy"),
                    "receive_auth_ok",
                    [false, true],
                ),
            ],
            Self::Ldap => [
                (
                    "protocols/ldap/bind-denied/main.gewy",
                    include_str!("../../../../protocols/ldap/bind-denied/main.gewy"),
                    "receive_bind_denied",
                    [true, false],
                ),
                (
                    "protocols/ldap/bind/main.gewy",
                    include_str!("../../../../protocols/ldap/bind/main.gewy"),
                    "receive_bind_response",
                    [true, true],
                ),
            ],
        }
    }
}

// Only a complete, small server stream is accepted. Missing/truncated/conflicting
// bytes stay an error, rather than being filled from the independently read client.
fn captured_response(packets: &[PacketEvent], cookie: u64) -> Result<Vec<u8>, ValidationError> {
    let selected: Vec<_> = packets.iter().filter(|p| p.cookie == cookie).collect();
    let origins: BTreeSet<_> = selected
        .iter()
        .filter(|p| p.flags & 0x17 == 0x12 && p.payload_len == 0)
        .map(|p| p.seq.wrapping_add(1))
        .collect();
    if origins.len() != 1 {
        return Err(ValidationError::new(
            "authentication stream requires one captured SYN-ACK origin",
        ));
    }
    let origin = *origins
        .first()
        .ok_or_else(|| ValidationError::new("missing stream origin"))?;
    let mut bytes = vec![None; 4096];
    let mut extent = 0;
    for packet in selected.iter().filter(|p| p.payload_len > 0) {
        let start = packet.seq.wrapping_sub(origin) as usize;
        let length = usize::from(packet.payload_len);
        if length != usize::from(packet.prefix_len)
            || length > packet.prefix.len()
            || start > bytes.len()
            || length > bytes.len() - start
        {
            return Err(ValidationError::new(
                "authentication stream truncated or outside 4-KiB bound",
            ));
        }
        for (slot, byte) in bytes[start..start + length].iter_mut().zip(&packet.prefix) {
            if slot.is_some_and(|previous| previous != *byte) {
                return Err(ValidationError::new(
                    "conflicting authentication stream bytes",
                ));
            }
            *slot = Some(*byte);
        }
        extent = extent.max(start + length);
    }
    if extent == 0 {
        return Err(ValidationError::new("empty authentication stream"));
    }
    bytes[..extent]
        .iter()
        .map(|b| b.ok_or_else(|| ValidationError::new("gap in authentication stream")))
        .collect()
}

fn stage_cookies(export: &ExportBundle, phase: &str) -> Result<BTreeSet<u64>, ValidationError> {
    let mut cookies = BTreeSet::new();
    for stage in export
        .program_flows
        .iter()
        .flat_map(|flow| &flow.stages)
        .filter(|stage| stage.phase.as_deref() == Some(phase))
    {
        let cookie = export
            .facts
            .iter()
            .find_map(|fact| match &fact.kind {
                FactKind::PacketMeta(packet) if fact.id == stage.at => packet.sk_cookie,
                _ => None,
            })
            .ok_or_else(|| ValidationError::new("authentication stage has no packet provenance"))?;
        cookies.insert(cookie);
    }
    Ok(cookies)
}

fn phase(out: &Path, protocol: Protocol) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "auth-breadth")?;
    let image_id = tool(
        docker().args(["image", "inspect", protocol.image(), "--format", "{{.Id}}"]),
        &out,
    )?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut network = OwnedResource::create(
        "network",
        &token,
        &["--driver", "bridge", "--internal"],
        &out,
    )?;
    let network_info = network.inspect()?;
    let mut args = vec![
        "--pull=never",
        "--network",
        &network.name,
        "--cpus=1",
        "--memory=512m",
        "--pids-limit=128",
        "--cap-drop=ALL",
        "--cap-add=CHOWN",
        "--cap-add=DAC_OVERRIDE",
        "--cap-add=FOWNER",
        "--cap-add=SETGID",
        "--cap-add=SETUID",
        "--cap-add=SYS_CHROOT",
        "--cap-add=NET_BIND_SERVICE",
        "--security-opt=no-new-privileges",
    ];
    // Disposable test credentials only. No host account or operator credential enters the fixture.
    let password = format!(
        "{}={}",
        if matches!(protocol, Protocol::Ftp) {
            "FTP_PASS"
        } else {
            "LDAP_ADMIN_PASSWORD"
        },
        wire::PASSWORD
    );
    match protocol {
        Protocol::Ftp => args.extend(["--env", "FTP_USER=gewy", "--env", &password]),
        Protocol::Ldap => args.extend([
            "--env",
            "LDAP_ORGANISATION=Gewyvern Lab",
            "--env",
            "LDAP_DOMAIN=example.org",
            "--env",
            "LDAP_TLS=false",
            "--env",
            &password,
        ]),
    }
    args.push(protocol.image());
    let mut container = OwnedResource::create("container", &token, &args, &out)?;
    tool(docker().args(["container", "start", &container.name]), &out)?;
    let container_info = container.inspect()?;
    let volumes: Vec<_> = container_info["Mounts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|mount| mount["Type"] == "volume")
        .map(|mount| mount["Name"].as_str().unwrap_or_default().to_owned())
        .collect();
    if volumes
        .iter()
        .any(|name| name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ValidationError::new(
            "authentication target has an unexpected named volume",
        ));
    }
    let (source, destination) = target_addresses(&container_info, &network_info, &network.name)?;
    let target = SocketAddr::from((destination, protocol.port()));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match wire::exchange(owned_socket_on(source)?, target, protocol, true) {
            Ok(_) => break,
            Err(error) if Instant::now() >= deadline => {
                fs::write(
                    out.join("target-startup.log"),
                    tool(
                        docker().args(["logs", "--tail", "100", &container.name]),
                        &out,
                    )?,
                )?;
                return Err(ValidationError::new(format!(
                    "{} readiness failed: {error}",
                    protocol.name()
                )));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
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
    let packets_capture = Capture::attach(&scope, &expected, addresses, &out)?;
    let packet_ownership = (
        packets_capture.path().to_owned(),
        packets_capture.link_id()?,
    );
    let mut responses = Vec::new();
    for (index, socket) in sockets.into_iter().enumerate() {
        let (response, code) = wire::exchange(socket, target, protocol, index == 1)?;
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
    let (_, neighbor_code) = wire::exchange(neighbor, target, protocol, false)?;
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
            "unselected authentication neighbor entered capture",
        ));
    }
    let mut observations = Vec::new();
    for (index, (response, code)) in responses.iter().enumerate() {
        let captured = captured_response(&packets, cookies[index])?;
        if &captured != response || wire::response_code(protocol, &captured)? != *code {
            return Err(ValidationError::new(
                "captured authentication bytes disagree with independent client",
            ));
        }
        observations.push(json!({"cookie":cookies[index],"expected_accepted":index==1,"client_code":code,
            "captured_code":wire::response_code(protocol,&captured)?,"response_bytes":response.len(),
            "response_sha256":sha256(response),"captured_sha256":sha256(&captured)}));
    }
    let mut packages = Vec::new();
    for (index, (path, rules, phase, matches)) in protocol.rules().into_iter().enumerate() {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "built-in authentication rules differ from workspace",
            ));
        }
        let export = reconstruct_packets_with_rules(&tcp, &states, &packets, clock, rules)?;
        let observed = stage_cookies(&export, phase)?;
        for id in ["route_meta_fragment", "sock_lineage_fragment"] {
            if export
                .attach_report
                .fragments_loaded
                .iter()
                .any(|name| name == id)
                || !export
                    .attach_report
                    .hookpoints_failed
                    .iter()
                    .any(|name| name.starts_with(id))
            {
                return Err(ValidationError::new(
                    "unobserved protocol fragment reported as loaded",
                ));
            }
        }
        let wanted: BTreeSet<_> = cookies
            .into_iter()
            .zip(matches)
            .filter_map(|(cookie, yes)| yes.then_some(cookie))
            .collect();
        if observed != wanted
            || export.flows.iter().any(|flow| flow.process.is_some())
            || export
                .facts
                .iter()
                .any(|fact| !matches!(fact.kind, FactKind::TcpState(_) | FactKind::PacketMeta(_)))
        {
            return Err(ValidationError::new(
                "protocol package result, negative control or evidence scope diverged",
            ));
        }
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"phase":phase,
            "matched_cookies":observed,"replay_identical":true,"facts":export.facts.len(),
            "unobserved_fragments_explicit":true,"process_attribution":false,"request_payload_observed":false}));
    }
    fs::write(
        out.join("packet-events.json"),
        serde_json::to_vec_pretty(&packets)?,
    )?;
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        &out,
    )?)?;
    if packet_ownership.0.exists() || links.iter().any(|link| link["id"] == packet_ownership.1) {
        return Err(ValidationError::new(
            "authentication packet link survived cleanup",
        ));
    }
    states_capture.cleanup()?;
    scope.cleanup()?;
    container.cleanup()?;
    for volume in &volumes {
        let remaining = tool(
            docker().args([
                "volume",
                "ls",
                "--quiet",
                "--filter",
                &format!("name=^{volume}$"),
            ]),
            &out,
        )?;
        if !remaining.iter().all(u8::is_ascii_whitespace) {
            return Err(ValidationError::new(
                "owned anonymous target volume survived cleanup",
            ));
        }
    }
    network.cleanup()?;
    let report = json!({"schema_version":1,"protocol":protocol.name(),"ok":true,"image":protocol.image(),
        "image_id":String::from_utf8_lossy(&image_id).trim(),"container_id":container_info["Id"],"network_id":network_info["Id"],
        "internal_network":true,"published_ports":false,"selected_cookies":cookies,
        "unselected_neighbor_cookie":neighbor_cookie,"unselected_neighbor_code":neighbor_code,
        "observations":observations,"packages":packages,"packet_counts":packet_counts,
        "tcp_state_counts":state_counts,"owned_resources_removed":true,"anonymous_volume_ids":volumes,
        "anonymous_volumes_removed_with_container":true});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-auth-breadth")?;
    let mut phases = Vec::new();
    for protocol in [Protocol::Ftp, Protocol::Ldap] {
        eprintln!("capture authentication breadth: {}", protocol.name());
        phases.push(phase(&out.join(protocol.name()), protocol)?);
    }
    super::http_boundaries::run(Some(out.join("after-auth-restart")))?;
    let checks = [
        "two_real_protocol_implementations",
        "selected_success_and_denial",
        "same_target_unselected_denial_excluded",
        "kernel_bytes_match_independent_client",
        "existing_protocol_packages_and_replay",
        "unobserved_process_route_and_egress_not_invented",
        "owned_internal_network_and_anonymous_volume_cleanup",
        "fresh_HTTP_capture_after_auth",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "mode":"auth-breadth","phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "not_proven":["TLS_decryption","arbitrary_protocol_detection","general_TCP_reassembly",
            "vulnerability_exploitation_detection","FTP_data_channel","LDAP_search_modify_or_SASL",
            "PID_or_container_process_attribution","request_payload_capture","IPv6_or_UDP","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: "Linux real FTP/LDAP authentication capture breadth".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(seq: u32, bytes: &[u8], flags: u8) -> PacketEvent {
        PacketEvent {
            sequence: 1,
            monotonic_ns: 1,
            cookie: 1,
            cpu: 0,
            ifindex: 1,
            source: 0,
            destination: 0,
            seq,
            ack: 1,
            sport: 21,
            dport: 4000,
            total_len: 40 + bytes.len() as u16,
            window: 1,
            payload_len: bytes.len() as u16,
            prefix_len: bytes.len() as u16,
            flags,
            ip_header_len: 20,
            tcp_header_len: 20,
            prefix: bytes.to_vec(),
        }
    }
    #[test]
    fn authentication_bytes_need_origin_complete_ranges_and_consistent_overlap() {
        let origin = packet(u32::MAX - 1, b"", 0x12);
        let first = packet(u32::MAX, b"53", 0x18);
        let second = packet(1, b"0 Denied\r\n", 0x18);
        assert_eq!(
            captured_response(
                &[origin.clone(), second.clone(), first.clone(), first.clone()],
                1
            )
            .unwrap(),
            b"530 Denied\r\n"
        );
        assert!(captured_response(&[first.clone(), second.clone()], 1).is_err());
        assert!(captured_response(&[origin.clone(), second], 1).is_err());
        assert!(
            captured_response(
                &[origin.clone(), first.clone(), packet(u32::MAX, b"23", 0x18)],
                1
            )
            .is_err()
        );
        let mut truncated = first;
        truncated.payload_len = 100;
        assert!(captured_response(&[origin, truncated], 1).is_err());
    }
}
