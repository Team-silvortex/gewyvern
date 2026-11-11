use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::super::packet_evidence::{packet_status_candidates, reconstruct_packets};
use super::super::response_prefix::{matches_client_response, verified_replay, verified_statuses};
use super::docker_target::{IMAGE, OwnedResource, docker, target_addresses};
use super::http_evidence::{Observation, correlate, exchange, parse_response, sha256};
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture_at, clock_anchor, export_stage_at,
    network_namespace, owned_socket_on, port, prepare_output, snapshot, tool, wait_state,
};

pub(super) fn run(out_dir: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out_dir, "linux-tcp-juice-shop")?;
    let image = tool(
        docker().args(["image", "inspect", IMAGE, "--format", "{{.Id}}"]),
        &out,
    )?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ValidationError::new(error.to_string()))?
            .as_nanos()
    );
    let mut network = OwnedResource::create(
        "network",
        &token,
        &["--driver", "bridge", "--internal"],
        &out,
    )?;
    let network_info = network.inspect()?;
    if network_info["Internal"] != true {
        return Err(ValidationError::new("target network must be internal"));
    }
    let mut container = OwnedResource::create(
        "container",
        &token,
        &[
            "--pull=never",
            "--network",
            &network.name,
            "--cpus=1",
            "--memory=768m",
            "--pids-limit=256",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            IMAGE,
        ],
        &out,
    )?;
    tool(docker().args(["container", "start", &container.name]), &out)?;
    let container_info = container.inspect()?;
    let (source, destination) = target_addresses(&container_info, &network_info, &network.name)?;
    let addresses = (source.octets(), destination.octets());
    let target_port = 3000;
    let target = SocketAddr::from((destination, target_port));
    fs::write(
        out.join("target.json"),
        serde_json::to_vec_pretty(&json!({
            "image":IMAGE, "image_id":String::from_utf8_lossy(&image).trim(),
            "container_id":container_info["Id"], "container_name":container.name,
            "network_id":network_info["Id"], "network_name":network.name, "internal_network":true,
            "private_endpoint":target.to_string(), "source_address":source.to_string(), "published_ports":false,
            "attribution":"host client socket to owned private container address, not container PID or server socket"
        }))?,
    )?;
    let ready_deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let ready = exchange(owned_socket_on(source)?, target, "/", "readiness")
            .and_then(|(_, response)| parse_response(&response).map(|(status, _)| status));
        match ready {
            Ok(200) => break,
            result if Instant::now() >= ready_deadline => {
                let logs = tool(
                    docker().args(["logs", "--tail", "200", &container.name]),
                    &out,
                )?;
                fs::write(out.join("target-startup.log"), logs)?;
                return Err(ValidationError::new(format!(
                    "Juice Shop readiness failed: {result:?}"
                )));
            }
            _ => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    let mut packet_scope = Scope::enter(&token)?;
    let first = owned_socket_on(source)?;
    let second = owned_socket_on(source)?;
    let cookies = [first.cookie()?, second.cookie()?];
    let netns = network_namespace()?;
    let expected = [
        (cookies[0], port(&first)?, target_port, netns),
        (cookies[1], port(&second)?, target_port, netns),
    ];
    let (mut pins, clock) = attach_capture_at(cookies, &out, addresses)?;
    let packet_capture = Capture::attach(&packet_scope, &expected, addresses, &out)?;
    let mut observations = Vec::new();
    for (index, (socket, name, path, expected_status, marker)) in [
        (
            first,
            "file-guard",
            "/ftp/acquisitions.md.bak",
            403,
            "Only .md and .pdf files are allowed!",
        ),
        (
            second,
            "sql-error",
            "/rest/products/search?q=%27%20OR%201%3D1--",
            500,
            "SQLITE_ERROR",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let request_id = format!("{token}-{name}");
        let started_ns = clock_anchor()?.0;
        let (request, response) = exchange(socket, target, path, &request_id)?;
        fs::write(out.join(format!("{name}-request.http")), &request)?;
        fs::write(out.join(format!("{name}-response.http")), &response)?;
        let (status, body) = parse_response(&response)?;
        if status != expected_status
            || !body
                .windows(marker.len())
                .any(|bytes| bytes == marker.as_bytes())
        {
            return Err(ValidationError::new(format!(
                "{name} response does not preserve expected status and marker"
            )));
        }
        wait_state(&pins, &out, name, cookies[index], 7)?;
        observations.push(Observation {
            request_id,
            path: path.into(),
            status,
            cookie: cookies[index],
            sport: expected[index].1,
            dport: target_port,
            netns,
            started_ns,
            completed_ns: clock_anchor()?.0,
            request_sha256: sha256(&request),
            response_sha256: sha256(&response),
        });
    }
    let decoy = owned_socket_on(source)?;
    let decoy_cookie = decoy.cookie()?;
    let (_, decoy_response) = exchange(decoy, target, "/", "unselected-decoy")?;
    fs::write(out.join("decoy-response.http"), &decoy_response)?;
    let (decoy_status, _) = parse_response(&decoy_response)?;
    if decoy_status != 200 {
        return Err(ValidationError::new(
            "unselected baseline request did not return 200",
        ));
    }
    pins.detach();
    let (events, stats) = snapshot(&pins, &out, "final")?;
    let export = export_stage_at(&events, stats, &expected, clock, &out, "final", addresses)?;
    if events.iter().any(|event| event.cookie == decoy_cookie) {
        return Err(ValidationError::new(
            "unselected request leaked into captured facts",
        ));
    }
    let correlation = correlate(&events, &export, &observations)?;
    let (packets, packet_stats) = packet_capture.finish(&out)?;
    let packet_export = reconstruct_packets(&export, &events, &packets, clock)?;
    let candidates = packet_status_candidates(&packet_export)?;
    let response_prefixes = verified_replay(&packet_export)?;
    let statuses = verified_statuses(&response_prefixes);
    fs::write(out.join("packet-export.json"), packet_export.to_json())?;
    fs::write(
        out.join("response-prefixes.json"),
        serde_json::to_vec_pretty(&response_prefixes)?,
    )?;
    fs::write(
        out.join("packet-events.json"),
        serde_json::to_vec_pretty(&packets)?,
    )?;
    let mut expected_statuses: Vec<_> = observations.iter().map(|o| (o.cookie, o.status)).collect();
    expected_statuses.sort_unstable();
    if statuses != expected_statuses {
        return Err(ValidationError::new(
            "kernel-derived response prefix disagrees with client HTTP status",
        ));
    }
    for (cookie, name) in [(cookies[0], "file-guard"), (cookies[1], "sql-error")] {
        let response = fs::read(out.join(format!("{name}-response.http")))?;
        if !response_prefixes
            .iter()
            .any(|p| p.cookie == cookie && matches_client_response(p, &response))
        {
            return Err(ValidationError::new(
                "kernel prefix does not match independently saved client response",
            ));
        }
    }
    packet_scope.cleanup()?;
    fs::write(
        out.join("correlation.json"),
        serde_json::to_vec_pretty(&correlation)?,
    )?;
    // Retain target logs as supplemental evidence, not as socket attribution.
    let logs = tool(
        docker().args(["logs", "--tail", "200", &container.name]),
        &out,
    )?;
    fs::write(out.join("target.log"), logs)?;
    pins.cleanup()?;
    container.cleanup()?;
    network.cleanup()?;
    let checks = vec![
        "owned_internal_container_target",
        "client_http_403_and_500",
        "socket_cookie_tuple_netns_correlation",
        "continuous_kernel_tcp_lifecycles",
        "exact_export_fact_and_flow_join",
        "unselected_http_200_excluded",
        "zero_reported_capture_loss",
        "deterministic_replay",
        "kernel_packet_prefix_http_403_and_500",
        "packet_metadata_and_dsl_replay",
        "syn_ack_anchored_response_prefix_and_replay",
        "owned_cgroup_removed",
        "owned_resources_removed",
    ];
    let summary = json!({"schema_version":2,"ok":true,"mode":"juice-shop-client-correlation",
        "kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(), "kernel_events":stats.total,
        "dropped_events":stats.dropped, "flows":export.flows.len(), "selected_sockets":expected, "addresses":addresses,
        "client_http_statuses":observations.iter().map(|o| o.status).collect::<Vec<_>>(),
        "decoy_cookie":decoy_cookie,"decoy_http_status":decoy_status,"decoy_events":0,
        "image":IMAGE,"pins_removed":true,"container_removed":true,"network_removed":true,"checks":checks,
        "ingest_trust_mode":"unverified-local", "http_evidence_source":"owned-native-client-io",
        "payload_inferred_by_gewyvern":true,"packet_evidence_source":"bpf-cgroup-skb-ingress",
        "packet_events":packet_stats.total,"packet_dropped_events":packet_stats.dropped,
        "packet_dsl_status_candidates":candidates,"verified_http_statuses":statuses,
        "response_prefix_evidence":"derived-first-13-bytes-relative-to-captured-syn-ack",
        "capture_prefix_limit":64,"cgroup_removed":true,
        "clock_anchor_monotonic_ns":clock.0,
        "clock_anchor_unix_ns":clock.1.duration_since(UNIX_EPOCH).map_err(|e| ValidationError::new(e.to_string()))?.as_nanos().to_string(),
        "not_proven":["TCP-stream-reassembly","TLS-decryption","vulnerability-detection","container-PID-or-post-NAT-socket-attribution","general-live-CLI","sustained-load"]});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux container HTTP evidence and TCP flow correlation".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
