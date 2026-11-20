//! Independent byte reconciliation and candidate provenance checks, not product predicates.
use super::super::super::{
    CaptureEvent,
    packet_evidence::{PacketEvent, reconstruct_with_prefix},
};
use super::super::bounded_stream::captured_response;
use super::super::http_evidence::sha256;
use super::super::repo_root;
use super::catalog::PackageCase;
use super::{Protocol, ValidationError, rtsp, wire};
use crate::export::ExportBundle;
use crate::ledger::FactKind;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, time::SystemTime};

pub(super) struct CapturedEvidence<'a> {
    pub(super) tcp: &'a ExportBundle,
    pub(super) states: &'a [CaptureEvent],
    pub(super) packets: &'a [PacketEvent],
    pub(super) clock: (u64, SystemTime),
    pub(super) cookies: [u64; 2],
}

pub(super) struct EvidenceReport {
    pub(super) observations: Vec<Value>,
    pub(super) packages: Vec<Value>,
    pub(super) package_failures: Vec<String>,
}

impl CapturedEvidence<'_> {
    pub(super) fn verify(
        self,
        protocol: Protocol,
        responses: &[(Vec<u8>, Value)],
        out: &Path,
    ) -> Result<EvidenceReport, ValidationError> {
        let Self {
            tcp,
            states,
            packets,
            clock,
            cookies,
        } = self;
        let mut observations = Vec::new();
        for (index, (response, code)) in responses.iter().enumerate() {
            let captured = captured_response(packets, cookies[index])?;
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
        let mut package_failures = Vec::new();
        fs::write(
            out.join("client-observations.json"),
            serde_json::to_vec_pretty(&observations)?,
        )?;
        fs::write(
            out.join("packet-events.json"),
            serde_json::to_vec_pretty(packets)?,
        )?;
        for (index, case) in protocol.cases().iter().enumerate() {
            let PackageCase {
                path,
                source: rules,
                terminal_phase: phase,
                expected_connections: matches,
            } = *case;
            if fs::read(repo_root().join(path))? != rules.as_bytes() {
                return Err(ValidationError::new(
                    "built-in database/cache rules differ from workspace",
                ));
            }
            let export = reconstruct_with_prefix(tcp, states, packets, clock, rules, 1024)?;
            fs::write(
                out.join(format!("package-{index}-export.json")),
                export.to_json(),
            )?;
            let observed = match stage_cookies(&export, phase, protocol) {
                Ok(cookies) => cookies,
                Err(error) => {
                    package_failures.push(error.to_string());
                    BTreeSet::new()
                }
            };
            for id in ["route_meta_fragment", "sock_lineage_fragment"] {
                if export
                    .attach_report
                    .fragments_loaded
                    .iter()
                    .any(|name| name == id)
                    || (export
                        .attach_plan
                        .fragments
                        .iter()
                        .any(|fragment| fragment.id == id)
                        && !export
                            .attach_report
                            .hookpoints_failed
                            .iter()
                            .any(|name| name.starts_with(id)))
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
                || export.facts.iter().any(|fact| {
                    !matches!(fact.kind, FactKind::TcpState(_) | FactKind::PacketMeta(_))
                })
            {
                package_failures.push(format!(
                "data package {path} phase {phase}: observed {observed:?}, expected {wanted:?}; check packet candidates versus framed response"
            ));
            }
            packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"phase":phase,
            "matched_cookies":observed,"replay_identical":true,"facts":export.facts.len(),
            "unobserved_fragments_explicit":true,"process_attribution":false,"request_payload_observed":false}));
        }

        Ok(EvidenceReport {
            observations,
            packages,
            package_failures,
        })
    }
}

fn valid_candidate(
    protocol: Protocol,
    phase: &str,
    packet: &crate::ledger::PacketMetaFact,
) -> bool {
    let at = |offset| {
        packet
            .payload_bytes
            .iter()
            .find_map(|(at, value)| (*at == offset).then_some(*value))
    };
    match protocol {
        Protocol::Rtsp => rtsp::candidate(packet, phase),
        Protocol::Pop3 => {
            let starts = |offset: u16, text: &[u8]| {
                text.iter()
                    .enumerate()
                    .all(|(i, b)| at(offset + i as u16) == Some(*b))
            };
            let text: &[u8] = match phase {
                "receive_banner" => b"+OK Mailpit POP3 server\r\n",
                "receive_user_ok" => b"+OK User accepted\r\n",
                "receive_auth_ok" => b"+OK signed in\r\n",
                "receive_auth_denied" => b"-ERR invalid password\r\n",
                "receive_list_ready" => {
                    return starts(0, b"+OK ")
                        && (1..=3).any(|digits| {
                            (4..4 + digits)
                                .all(|offset| at(offset).is_some_and(|b| b.is_ascii_digit()))
                                && starts(4 + digits, b" messages (")
                        });
                }
                _ => return false,
            };
            starts(0, text)
        }
        Protocol::Smtp => {
            if phase == "receive_rcpt_denied" {
                return [b"550 5.1.0 ", b"550 5.1.1 "].iter().any(|prefix| {
                    prefix
                        .iter()
                        .enumerate()
                        .all(|(offset, value)| at(offset as u16) == Some(*value))
                });
            }
            let prefix: &[u8] = match phase {
                "receive_banner" => b"220 ",
                "receive_ehlo_ok" => b"250-",
                "receive_auth_ok" => b"235 ",
                "receive_auth_denied" => b"535 ",
                "receive_mail_ok" => b"250 2.1.0 ",
                "receive_rcpt_ok" => b"250 2.1.5 ",
                "receive_data_ready" => b"354 ",
                "receive_message_queued" => b"250 2.0.0 Ok: queued as ",
                "receive_message_denied" => b"550 5.7.1 Message rejected",
                _ => return false,
            };
            prefix
                .iter()
                .enumerate()
                .all(|(offset, value)| at(offset as u16) == Some(*value))
        }
        Protocol::Mqtt => match phase {
            "receive_connack" => {
                at(0) == Some(32)
                    && at(1) == Some(2)
                    && matches!(at(2), Some(0..=1))
                    && matches!(at(3), Some(0..=5))
            }
            "receive_puback" | "receive_pubrec" | "receive_pubcomp" => {
                let kind = match phase {
                    "receive_puback" => 64,
                    "receive_pubrec" => 80,
                    _ => 112,
                };
                at(0) == Some(kind)
                    && at(1) == Some(2)
                    && at(2).zip(at(3)).is_some_and(|id| id != (0, 0))
            }
            "receive_suback" => {
                at(0) == Some(144)
                    && at(1) == Some(3)
                    && at(2).zip(at(3)).is_some_and(|id| id != (0, 0))
                    && at(4) == Some(0)
            }
            _ => false,
        },
        Protocol::Nats => {
            let prefix: &[u8] = match phase {
                "receive_info" => b"INFO ",
                "receive_error" => b"-ERR '",
                "receive_message" => b"MSG ",
                _ => return false,
            };
            prefix
                .iter()
                .enumerate()
                .all(|(offset, value)| at(offset as u16) == Some(*value))
        }
        Protocol::Mysql => match phase {
            "receive_auth_denied" => {
                at(4) == Some(255)
                    && at(5) == Some(0x15)
                    && at(6) == Some(4)
                    && (7..13)
                        .zip(b"#28000")
                        .all(|(offset, value)| at(offset) == Some(*value))
            }
            "receive_error" => {
                at(3) == Some(1) && at(4) == Some(255) && at(7) == Some(b'#') && at(12).is_some()
            }
            "receive_ok" => at(3) == Some(2) && at(4) == Some(0),
            _ => false,
        },
        Protocol::Memcached => {
            let status = at(6).zip(at(7)).map(|(a, b)| u16::from_be_bytes([a, b]));
            at(0) == Some(129)
                && at(23).is_some()
                && match phase {
                    "receive_value" => at(1) == Some(0) && at(4) == Some(4) && status == Some(0),
                    "receive_stored" => at(1) == Some(1) && status == Some(0),
                    "receive_not_found" => at(1) == Some(0) && status == Some(1),
                    "receive_not_stored" => matches!(at(1), Some(1 | 14 | 15)) && status == Some(5),
                    _ => false,
                }
        }
        _ => true,
    }
}

fn stage_cookies(
    export: &ExportBundle,
    phase: &str,
    protocol: Protocol,
) -> Result<BTreeSet<u64>, ValidationError> {
    let mut cookies = BTreeSet::new();
    for stage in export
        .program_flows
        .iter()
        .flat_map(|flow| &flow.stages)
        .filter(|stage| {
            stage.phase.as_deref() == Some(phase)
                || (matches!(protocol, Protocol::Smtp | Protocol::Pop3 | Protocol::Rtsp)
                    && stage
                        .phase
                        .as_deref()
                        .is_some_and(|p| p.starts_with("receive_")))
        })
    {
        let packet = export
            .facts
            .iter()
            .find_map(|fact| match &fact.kind {
                FactKind::PacketMeta(packet) if fact.id == stage.at => Some(packet),
                _ => None,
            })
            .ok_or_else(|| ValidationError::new("database/cache stage has no packet provenance"))?;
        let actual_phase = stage.phase.as_deref().unwrap_or(phase);
        if !valid_candidate(protocol, actual_phase, packet) {
            return Err(ValidationError::new(format!(
                "{} {actual_phase} matched the wrong actual packet at fact {:?}, cookie {:?}, bytes {:?}",
                protocol.name(),
                stage.at,
                packet.sk_cookie,
                packet.payload_bytes.iter().take(26).collect::<Vec<_>>()
            )));
        }
        let cookie = packet
            .sk_cookie
            .ok_or_else(|| ValidationError::new("candidate has no owned cookie"))?;
        if actual_phase == phase {
            cookies.insert(cookie);
        }
    }
    Ok(cookies)
}
