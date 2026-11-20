//! Shared lifecycle and byte oracle for real-target consumers of the product CLI.
use super::http_evidence::sha256;
use super::packet_capture::Scope;
use super::{ValidationError, repo_root, tool};
use crate::export::ExportBundle;
use crate::ledger::{FactKind, PacketDir};
use crate::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub(super) type PacketBytes = (u64, u8, Vec<u8>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{ProgramFlow, ProgramFlowId, ProgramOperation, ProgramStage};
    use crate::ir::SignalKind;
    use crate::ledger::{FactId, SessionId};
    use crate::packet_capture::{CapturedUdpDatagram, PacketObservation};
    use crate::runtime::{RuntimeSession, SessionConfig};

    #[test]
    fn phase_oracle_rejects_missing_foreign_and_false_positive_candidates() {
        let mut export = RuntimeSession::start(
            SessionConfig::for_template(crate::template::udp_debug_template()).unwrap(),
        )
        .unwrap()
        .into_export_bundle();
        let payload = vec![1, 2, 3, 4];
        export.facts.push(
            CapturedUdpDatagram {
                source: "127.0.0.1:4000".parse().unwrap(),
                destination: "127.0.0.2:123".parse().unwrap(),
                ip_header_len: 20,
                total_len: 32,
                payload_len: 4,
                payload_prefix: &payload,
            }
            .into_fact(PacketObservation {
                id: FactId(1),
                session: SessionId(1),
                ts: std::time::UNIX_EPOCH,
                cpu: 0,
                ifindex: 1,
                netns: 1,
                socket_cookie: 7,
                direction: PacketDir::Egress,
            })
            .unwrap(),
        );
        export.program_flows.push(ProgramFlow {
            id: ProgramFlowId(1),
            process: None,
            operation: ProgramOperation::DatagramExchange,
            transport_flows: vec![],
            narrative: vec![],
            stages: vec![ProgramStage::from_signal(
                FactId(1),
                SignalKind::DatagramObserved,
                Some("request".into()),
            )],
        });
        let plan = Plan {
            protocol: "ntp",
            entry: "sync",
            port: 123,
            request_phase: "request",
            response_phase: "time",
        };
        let mut observations = vec![Observation {
            cookie: 7,
            request: payload,
            response: None,
            request_matches: true,
            response_matches: false,
            detail: Value::Null,
        }];
        assert!(check_phases(&export, &observations, &plan).is_ok());
        observations[0].request_matches = false;
        assert!(check_phases(&export, &observations, &plan).is_err());
        observations[0].request_matches = true;
        observations[0].cookie = 8;
        assert!(check_phases(&export, &observations, &plan).is_err());
        observations[0].cookie = 7;
        observations[0].request[1] ^= 1;
        assert!(check_phases(&export, &observations, &plan).is_err());
        observations[0].request[1] ^= 1;
        let mut missing = export.clone();
        missing.program_flows.clear();
        assert!(check_phases(&missing, &observations, &plan).is_err());
        export.program_flows[0].stages[0].phase = Some("time".into());
        assert!(check_phases(&export, &observations, &plan).is_err());
    }
}

pub(super) struct Plan {
    pub protocol: &'static str,
    pub entry: &'static str,
    pub port: u16,
    pub request_phase: &'static str,
    pub response_phase: &'static str,
}

pub(super) struct Observation {
    pub cookie: u64,
    pub request: Vec<u8>,
    pub response: Option<Vec<u8>>,
    pub request_matches: bool,
    pub response_matches: bool,
    pub detail: Value,
}

fn check_phases(
    export: &ExportBundle,
    observations: &[Observation],
    plan: &Plan,
) -> Result<Value, ValidationError> {
    let mut result = BTreeMap::new();
    for (phase, direction) in [
        (plan.request_phase, PacketDir::Egress),
        (plan.response_phase, PacketDir::Ingress),
    ] {
        let expected: Vec<_> = observations
            .iter()
            .filter_map(|o| {
                if direction == PacketDir::Egress {
                    o.request_matches
                        .then_some((o.cookie, o.request.as_slice()))
                } else {
                    o.response
                        .as_deref()
                        .filter(|_| o.response_matches)
                        .map(|b| (o.cookie, b))
                }
            })
            .collect();
        let wanted: BTreeSet<_> = expected.iter().map(|(cookie, _)| *cookie).collect();
        let mut actual = BTreeSet::new();
        for stage in export
            .program_flows
            .iter()
            .flat_map(|f| &f.stages)
            .filter(|s| s.phase.as_deref() == Some(phase))
        {
            let packet = export
                .facts
                .iter()
                .find_map(|f| match &f.kind {
                    FactKind::PacketMeta(p) if f.id == stage.at && p.dir == direction => Some(p),
                    _ => None,
                })
                .ok_or_else(|| ValidationError::new("product phase lacks packet provenance"))?;
            if !expected.iter().any(|(cookie, bytes)| {
                packet.sk_cookie == Some(*cookie)
                    && packet.payload_bytes.len() == bytes.len()
                    && bytes
                        .iter()
                        .enumerate()
                        .all(|(at, byte)| packet.payload_bytes.get(&(at as u16)) == Some(byte))
            }) {
                return Err(ValidationError::new(format!(
                    "product phase {phase} admits a rejected client outcome"
                )));
            }
            actual.insert(packet.sk_cookie.unwrap_or_default());
        }
        if actual != wanted {
            return Err(ValidationError::new(format!(
                "product phase {phase}: expected {wanted:?}, got {actual:?}"
            )));
        }
        result.insert(
            phase,
            actual
                .into_iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>(),
        );
    }
    Ok(serde_json::to_value(result)?)
}

pub(super) fn package(
    plan: &Plan,
    scope: &Scope,
    out: &Path,
    identities: &[(u64, u16); 2],
    neighbor: u64,
    exercise: impl FnOnce() -> Result<(Vec<Observation>, Value), ValidationError>,
) -> Result<Value, ValidationError> {
    if identities[0].0 == identities[1].0
        || identities
            .iter()
            .any(|(c, p)| *c == 0 || *c == neighbor || *p == 0)
    {
        return Err(ValidationError::new(
            "product UDP socket identities are not unique",
        ));
    }
    let binary = fs::canonicalize(repo_root().join("target/debug/gewyvern"))?;
    let mut child = start(&binary, scope, out, plan.protocol, plan.entry, plan.port)?;
    let (observations, neighbor_reply) = exercise()?;
    let (export, snapshot) = finish(&mut child, out)?;
    let mut expected = Vec::new();
    for o in &observations {
        expected.push((o.cookie, 1, o.request.clone()));
        expected.extend(o.response.iter().map(|b| (o.cookie, 0, b.clone())));
    }
    check_bytes(&export, &expected, identities, 0x800, neighbor, plan.port)?;
    if snapshot.retained != export.facts.len() as u64 {
        return Err(ValidationError::new(
            "product fact and retained counts differ",
        ));
    }
    let phases = check_phases(&export, &observations, plan)?;
    let links = tool(
        Command::new("bpftool")
            .args(["-j", "cgroup", "show"])
            .arg(scope.path()),
        out,
    )?;
    if !links.iter().all(u8::is_ascii_whitespace)
        && !serde_json::from_slice::<Vec<Value>>(&links)?.is_empty()
    {
        return Err(ValidationError::new("product UDP hooks survived shutdown"));
    }
    let oracle: Vec<_> = observations.iter().map(|o| json!({
        "cookie":o.cookie.to_string(),"detail":o.detail,"timeout":o.response.is_none(),
        "request_bytes":o.request.len(),"response_bytes":o.response.as_ref().map(Vec::len),
        "request_sha256":sha256(&o.request),"response_sha256":o.response.as_ref().map(|b|sha256(b)),
        "request_phase_expected":o.request_matches,"response_phase_expected":o.response_matches
    })).collect();
    fs::write(out.join("oracle.json"), serde_json::to_vec_pretty(&oracle)?)?;
    Ok(
        json!({"entry":plan.entry,"protocol":plan.protocol,"port":plan.port,
        "binary_sha256":sha256(&fs::read(binary)?),
        "source_sha256":sha256(&fs::read(repo_root().join(format!("protocols/{}/{}/main.gewy",plan.protocol,plan.entry)))?),
        "capture":snapshot,"facts":export.facts.len(),"observations":oracle,"phases":phases,
        "neighbor_reply":neighbor_reply,"production_cli":true,"validation_capture_used":false,
        "outside_cgroup_neighbor_excluded":true,"full_payload_multiset_matches":true,
        "replay_identical":true,"packet_only":true,"process_attribution":false,"hooks_detached":true}),
    )
}

pub(super) struct Product(Child);
impl Drop for Product {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) fn start(
    binary: &Path,
    scope: &Scope,
    out: &Path,
    protocol: &str,
    entry: &str,
    port: u16,
) -> Result<Product, ValidationError> {
    fs::create_dir(out)?;
    let mut command = Command::new(binary);
    command
        .env_clear()
        .env("HOME", out.join("home"))
        .env("GEWY_PROTOCOL_REGISTRY_ROOT", repo_root().join("protocols"));
    for (key, directory) in [
        ("GEWY_CONFIG_HOME", "config"),
        ("GEWY_DATA_HOME", "data"),
        ("GEWY_STATE_HOME", "state"),
        ("GEWY_CACHE_HOME", "cache"),
    ] {
        command.env(key, out.join(directory));
    }
    let mut product = Product(
        command
            .current_dir(out)
            .args([
                "--protocol",
                protocol,
                "--entry",
                entry,
                "--json",
                "--capture-cgroup",
            ])
            .arg(scope.path())
            .args([
                "--capture-udp-port",
                &port.to_string(),
                "--capture-seconds",
                "30",
                "--capture-max-facts",
                "4096",
                "--out",
            ])
            .arg(out.join("export.json"))
            .arg("--capture-status")
            .arg(out.join("status.json"))
            .stdin(Stdio::null())
            .stdout(fs::File::create(out.join("stdout.log"))?)
            .stderr(fs::File::create(out.join("stderr.log"))?)
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        if let Some(status) = product.0.try_wait()? {
            return Err(ValidationError::new(format!(
                "production UDP CLI exited before capture readiness: {status}; see {}",
                out.join("stderr.log").display()
            )));
        }
        if let Ok(bytes) = fs::read(out.join("status.json")) {
            let snapshot: CaptureSnapshot = serde_json::from_slice(&bytes)?;
            if snapshot.state == CaptureState::Running
                && snapshot.cgroup == scope.path().to_string_lossy()
                && snapshot.udp_port == port
                && !snapshot.incomplete
            {
                return Ok(product);
            }
        }
        if Instant::now() >= deadline {
            return Err(ValidationError::new("product capture readiness deadline"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn finish(
    product: &mut Product,
    out: &Path,
) -> Result<(ExportBundle, CaptureSnapshot), ValidationError> {
    // SAFETY: this guard owns the live child and has not reaped its PID.
    if unsafe { libc::kill(product.0.id() as i32, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = product.0.try_wait()? {
            if !status.success() {
                return Err(ValidationError::new(format!(
                    "production UDP CLI failed: {status}; see {}",
                    out.join("stderr.log").display()
                )));
            }
            break;
        }
        if Instant::now() >= deadline {
            return Err(ValidationError::new("product capture shutdown deadline"));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let export = ExportBundle::from_json(&fs::read_to_string(out.join("export.json"))?)
        .map_err(|e| ValidationError::new(format!("invalid product export: {e:?}")))?;
    let snapshot: CaptureSnapshot = serde_json::from_slice(&fs::read(out.join("status.json"))?)?;
    if export.capture.as_ref() != Some(&snapshot)
        || snapshot.state != CaptureState::Stopped
        || snapshot.incomplete
        || snapshot.has_loss()
        || !snapshot.final_accounted
        || !snapshot.reconciles()
    {
        return Err(ValidationError::new(
            "product UDP capture is incomplete or export/status disagree",
        ));
    }
    let replay = export
        .replay()
        .map_err(|e| ValidationError::new(format!("product export replay failed: {e:?}")))?;
    if replay.facts != export.facts
        || replay.program_flows != export.program_flows
        || replay.capture != export.capture
        || replay.attach_report != export.attach_report
    {
        return Err(ValidationError::new("production UDP export/replay differs"));
    }
    Ok((export, snapshot))
}

pub(super) fn check_bytes(
    export: &ExportBundle,
    expected: &[PacketBytes],
    identities: &[(u64, u16); 2],
    family: u16,
    neighbor: u64,
    port: u16,
) -> Result<(), ValidationError> {
    let mut expected = expected.to_vec();
    if expected.iter().any(|(_, _, bytes)| bytes.len() > 128) {
        return Err(ValidationError::new(
            "UDP oracle exceeds captured prefix; refusing full-byte claim",
        ));
    }
    let mut actual = Vec::new();
    let mut namespaces = BTreeSet::new();
    for fact in &export.facts {
        let FactKind::PacketMeta(packet) = &fact.kind else {
            return Err(ValidationError::new(
                "product fabricated non-packet UDP evidence",
            ));
        };
        let (cookie, local) = identities
            .iter()
            .copied()
            .find(|(c, _)| packet.sk_cookie == Some(*c))
            .ok_or_else(|| ValidationError::new("product captured an unselected UDP socket"))?;
        let outbound = packet.dir == PacketDir::Egress;
        if packet.sk_cookie == Some(neighbor)
            || packet.l3_proto != family
            || packet.l4_proto != 17
            || packet.local_port != Some(local)
            || packet.remote_port != Some(port)
        {
            return Err(ValidationError::new(
                "product UDP packet identity/family/ports disagree",
            ));
        }
        let bytes: Vec<u8> = (0..packet.payload_bytes.len())
            .map(|at| {
                packet
                    .payload_bytes
                    .get(&(at as u16))
                    .copied()
                    .ok_or_else(|| ValidationError::new("product UDP prefix has a hole"))
            })
            .collect::<Result<_, _>>()?;
        if packet.tot_len as usize != bytes.len() + if family == 0x86dd { 48 } else { 28 } {
            return Err(ValidationError::new(
                "product UDP IP length differs from full captured payload",
            ));
        }
        namespaces.insert(packet.netns);
        actual.push((cookie, u8::from(outbound), bytes));
    }
    expected.sort();
    actual.sort();
    if actual != expected
        || namespaces.len() != 1
        || export.ingest_trust_mode != "unverified-local"
        || export.flows.iter().any(|f| f.process.is_some())
    {
        return Err(ValidationError::new(
            "product UDP facts differ from independent client bytes/identity",
        ));
    }
    Ok(())
}
