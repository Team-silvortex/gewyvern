use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::super::packet_evidence::{packet_status_candidates, reconstruct_packets};
use super::super::response_prefix::{verified_replay, verified_statuses};
use super::http_evidence::parse_response;
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture, export_stage, network_namespace,
    owned_socket, port, prepare_output, snapshot, wait_state,
};

pub(super) fn run(out_dir: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out_dir, "linux-http-boundaries")?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let target = listener.local_addr()?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut scope = Scope::enter(&token)?;
    let first = owned_socket()?;
    let second = owned_socket()?;
    let cookies = [first.cookie()?, second.cookie()?];
    let netns = network_namespace()?;
    let expected = [
        (cookies[0], port(&first)?, target.port(), netns),
        (cookies[1], port(&second)?, target.port(), netns),
    ];
    let (mut pins, clock) = attach_capture(cookies, &out)?;
    let capture = Capture::attach(&scope, &expected, ([127, 0, 0, 1], [127, 0, 0, 1]), &out)?;
    let body = b"HTTP/1.1 500 fake-body-prefix\r\n";
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let scenarios: [(&str, Vec<&[u8]>, u16); 2] = [
        (
            "split-403",
            vec![
                b"HTTP/1.",
                b"1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            ],
            403,
        ),
        ("body-decoy-200", vec![header.as_bytes(), body], 200),
    ];
    for (index, (socket, (name, chunks, status))) in
        [first, second].into_iter().zip(scenarios).enumerate()
    {
        socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
        let mut client: TcpStream = socket.into();
        let (mut peer, address) = listener.accept()?;
        if address.port() != expected[index].1 {
            return Err(ValidationError::new("boundary peer identity mismatch"));
        }
        peer.set_write_timeout(Some(Duration::from_secs(2)))?;
        peer.set_read_timeout(Some(Duration::from_secs(2)))?;
        peer.set_nodelay(true)?;
        let request = b"GET /boundary HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        client.write_all(request)?;
        let mut read = vec![0; request.len()];
        peer.read_exact(&mut read)?;
        if read != request {
            return Err(ValidationError::new("boundary request mismatch"));
        }
        let mut response = Vec::new();
        // Each chunk is consumed before the next write; do not assume write()
        // boundaries are packet boundaries. The kernel evidence checks the split.
        for chunk in chunks {
            peer.write_all(chunk)?;
            let mut received = vec![0; chunk.len()];
            client.read_exact(&mut received)?;
            if received != chunk {
                return Err(ValidationError::new("boundary response chunk mismatch"));
            }
            response.extend(received);
        }
        fs::write(out.join(format!("{name}-response.http")), &response)?;
        if parse_response(&response)?.0 != status {
            return Err(ValidationError::new("boundary client status mismatch"));
        }
        drop(peer);
        drop(client);
        wait_state(&pins, &out, name, cookies[index], 7)?;
    }
    let decoy = owned_socket()?;
    let decoy_cookie = decoy.cookie()?;
    decoy.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let (peer, _) = listener.accept()?;
    drop(peer);
    drop(decoy);
    pins.detach();
    let (states, stats) = snapshot(&pins, &out, "final")?;
    let tcp = export_stage(&states, stats, &expected, clock, &out, "final")?;
    let (packets, packet_stats) = capture.finish(&out)?;
    fs::write(
        out.join("packet-events.json"),
        serde_json::to_vec_pretty(&packets)?,
    )?;
    let export = reconstruct_packets(&tcp, &states, &packets, clock)?;
    fs::write(out.join("packet-export.json"), export.to_json())?;
    let prefixes = verified_replay(&export)?;
    fs::write(
        out.join("response-prefixes.json"),
        serde_json::to_vec_pretty(&prefixes)?,
    )?;
    let candidates = packet_status_candidates(&export)?;
    let statuses = verified_statuses(&prefixes);
    let mut expected_statuses = vec![(cookies[0], 403), (cookies[1], 200)];
    expected_statuses.sort_unstable();
    if statuses != expected_statuses
        || candidates != vec![(cookies[1], 500)]
        || prefixes
            .iter()
            .find(|p| p.cookie == cookies[0])
            .is_none_or(|p| p.payload_fact_ids.len() < 2)
        || packets.iter().any(|p| p.cookie == decoy_cookie)
        || states.iter().any(|s| s.cookie == decoy_cookie)
    {
        return Err(ValidationError::new(
            "HTTP boundary proof did not preserve split, origin or decoy exclusion",
        ));
    }
    pins.cleanup()?;
    scope.cleanup()?;
    let checks = vec![
        "captured_syn_ack_origins",
        "real_split_status_line",
        "body_prefix_candidate_not_promoted",
        "derived_prefix_provenance",
        "raw_packets_unchanged",
        "export_and_derived_replay",
        "decoy_exclusion",
        "zero_reported_capture_loss",
        "owned_pins_and_cgroup_removed",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"ok":true,"mode":"http-boundaries","kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
            "kernel_events":stats.total,"packet_events":packet_stats.total,"dropped_events":stats.dropped,
            "packet_dropped_events":packet_stats.dropped,"flows":export.flows.len(),"selected_sockets":expected,
            "packet_dsl_status_candidates":candidates,"verified_http_statuses":statuses,"decoy_cookie":decoy_cookie,"decoy_events":0,
            "clock_anchor_monotonic_ns":clock.0,"clock_anchor_unix_ns":clock.1.duration_since(UNIX_EPOCH).map_err(|e| ValidationError::new(e.to_string()))?.as_nanos().to_string(),
            "checks":checks,"ingest_trust_mode":"unverified-local","pins_removed":true,"cgroup_removed":true,
            "derived_prefix_limit":13,"not_proven":["general-TCP-stream-reassembly","subsequent-HTTP-responses","HTTP-informational-responses","TLS","TCP-Fast-Open","vulnerability-detection"]
        }))?,
    )?;
    Ok(ValidationReport {
        name: "Linux HTTP split-prefix and body-decoy boundaries".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
