//! Real private PostgreSQL/Redis -> bounded kernel bytes -> package candidates -> replay.
use super::super::packet_evidence::reconstruct_with_prefix;
use super::bounded_stream::captured_response;
use super::docker_target::{OwnedResource, docker, target_addresses};
use super::http_evidence::sha256;
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture_at, export_stage_at, network_namespace,
    owned_socket_on, port, prepare_output, repo_root, snapshot, tool, wait_state,
};
use crate::export::ExportBundle;
use crate::ledger::FactKind;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
mod wire;

#[derive(Clone, Copy)]
enum Protocol {
    Postgres,
    Redis,
}
impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Redis => "redis",
        }
    }
    fn port(self) -> u16 {
        match self {
            Self::Postgres => 5432,
            Self::Redis => 6379,
        }
    }
    fn image(self) -> &'static str {
        match self {
            Self::Postgres => {
                "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"
            }
            Self::Redis => {
                "redis@sha256:7958cfbb9d152f87dadfbc89f1b0b15938f0b81ff8982feb327476f4911935c3"
            }
        }
    }
    fn rules(self) -> [(&'static str, &'static str, &'static str, [bool; 2]); 3] {
        match self {
            Self::Postgres => [
                (
                    "protocols/postgres/auth-denied/main.gewy",
                    include_str!("../../../../protocols/postgres/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/postgres/error/main.gewy",
                    include_str!("../../../../protocols/postgres/error/main.gewy"),
                    "receive_error",
                    [false, true],
                ),
                (
                    "protocols/postgres/auth/main.gewy",
                    include_str!("../../../../protocols/postgres/auth/main.gewy"),
                    "receive_auth",
                    [true, true],
                ),
            ],
            Self::Redis => [
                (
                    "protocols/redis/auth-denied/main.gewy",
                    include_str!("../../../../protocols/redis/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/redis/error/main.gewy",
                    include_str!("../../../../protocols/redis/error/main.gewy"),
                    "receive_error",
                    [true, false],
                ),
                (
                    "protocols/redis/get/main.gewy",
                    include_str!("../../../../protocols/redis/get/main.gewy"),
                    "receive_bulk",
                    [true, true],
                ),
            ],
        }
    }
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
            .ok_or_else(|| ValidationError::new("database/cache stage has no packet provenance"))?;
        cookies.insert(cookie);
    }
    Ok(cookies)
}

fn phase(out: &Path, protocol: Protocol) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "data-breadth")?;
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
    let password = format!("POSTGRES_PASSWORD={}", wire::PASSWORD);
    match protocol {
        Protocol::Postgres => args.extend([
            "--env",
            "POSTGRES_USER=gewy",
            "--env",
            "POSTGRES_DB=gewy",
            "--env",
            &password,
            "--env",
            "POSTGRES_HOST_AUTH_METHOD=password",
            "--env",
            "POSTGRES_INITDB_ARGS=--encoding=UTF8 --locale=C",
            "--tmpfs",
            "/var/lib/postgresql/data:rw,nosuid,nodev,size=128m",
        ]),
        Protocol::Redis => args.extend(["--tmpfs", "/data:rw,nosuid,nodev,size=32m"]),
    }
    args.push(protocol.image());
    match protocol {
        Protocol::Postgres => args.extend([
            "postgres",
            "-c",
            "shared_buffers=16MB",
            "-c",
            "max_connections=16",
        ]),
        Protocol::Redis => args.extend([
            "redis-server",
            "--save",
            "",
            "--appendonly",
            "no",
            "--requirepass",
            wire::PASSWORD,
        ]),
    }
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
            "database/cache target has an unexpected named volume",
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
    let packets_capture = Capture::attach_data_breadth(&scope, &expected, addresses, &out)?;
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
            "unselected database/cache neighbor entered capture",
        ));
    }
    let mut observations = Vec::new();
    for (index, (response, code)) in responses.iter().enumerate() {
        let captured = captured_response(&packets, cookies[index])?;
        if &captured != response || wire::describe(protocol, &captured)? != *code {
            return Err(ValidationError::new(
                "captured database/cache bytes disagree with independent client",
            ));
        }
        observations.push(
            json!({"cookie":cookies[index],"control":index==1,"client_result":code,
            "captured_result":wire::describe(protocol,&captured)?,"response_bytes":response.len(),
            "response_sha256":sha256(response),"captured_sha256":sha256(&captured)}),
        );
    }
    let mut packages = Vec::new();
    fs::write(
        out.join("client-observations.json"),
        serde_json::to_vec_pretty(&observations)?,
    )?;
    fs::write(
        out.join("packet-events.json"),
        serde_json::to_vec_pretty(&packets)?,
    )?;
    for (index, (path, rules, phase, matches)) in protocol.rules().into_iter().enumerate() {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "built-in database/cache rules differ from workspace",
            ));
        }
        let export = reconstruct_with_prefix(&tcp, &states, &packets, clock, rules, 1024)?;
        let observed = stage_cookies(&export, phase)?;
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
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
            return Err(ValidationError::new(format!(
                "data package {path} phase {phase}: observed {observed:?}, expected {wanted:?}; check packet candidates versus framed response"
            )));
        }
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"phase":phase,
            "matched_cookies":observed,"replay_identical":true,"facts":export.facts.len(),
            "unobserved_fragments_explicit":true,"process_attribution":false,"request_payload_observed":false}));
    }
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
        "unselected_neighbor_cookie":neighbor_cookie,"unselected_neighbor_result":neighbor_code,
        "observations":observations,"packages":packages,"packet_counts":packet_counts,
        "tcp_state_counts":state_counts,"owned_resources_removed":true,"anonymous_volume_ids":volumes,
        "anonymous_volumes_removed_with_container":true});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-data-breadth")?;
    let mut phases = Vec::new();
    for protocol in [Protocol::Postgres, Protocol::Redis] {
        eprintln!("capture database/cache breadth: {}", protocol.name());
        phases.push(phase(&out.join(protocol.name()), protocol)?);
    }
    super::http_boundaries::run(Some(out.join("after-data-restart")))?;
    let checks = [
        "real_PostgreSQL_and_Redis",
        "auth_denial_query_error_then_recovery",
        "same_target_unselected_denial_excluded",
        "kernel_bytes_match_independent_client",
        "existing_protocol_packages_and_replay",
        "unobserved_process_route_and_egress_not_invented",
        "owned_internal_network_and_anonymous_volume_cleanup",
        "fresh_HTTP_capture_after_data",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "mode":"data-breadth","phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "not_proven":["TLS_decryption","arbitrary_protocol_detection","general_TCP_reassembly",
            "vulnerability_exploitation_detection","PostgreSQL_TLS_SCRAM_extended_query","Redis_cluster_RESP3",
            "PID_or_container_process_attribution","request_payload_capture","IPv6_or_UDP","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: "Linux real PostgreSQL/Redis capture breadth".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
