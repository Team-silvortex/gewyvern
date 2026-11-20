use super::super::super::udp_evidence::reconstruct;
use super::super::repo_root;
use super::*;
use crate::ledger::{FactKind, PacketDir};
use std::collections::BTreeSet;

pub(super) fn reconcile(
    exchanges: &[Exchange],
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
    local: bool,
) -> Result<Vec<Value>, ValidationError> {
    let mut seen = [BTreeSet::new(), BTreeSet::new()];
    let mut observations = Vec::new();
    for exchange in exchanges {
        let mut captured = [Vec::new(), Vec::new()];
        for (i, events) in [egress, ingress].into_iter().enumerate() {
            for event in events {
                let bytes = validate(event, i == 0, context)?;
                if event.cookie == exchange.cookie
                    && (exchange.start_ns..=exchange.end_ns).contains(&event.monotonic_ns)
                {
                    if !seen[i].insert(event.sequence) {
                        return Err(ValidationError::new("NTP datagram reused"));
                    }
                    captured[i].push(bytes.to_vec());
                }
            }
        }
        if captured[0] != [exchange.request.clone()]
            || captured[1] != exchange.response.iter().cloned().collect::<Vec<_>>()
        {
            return Err(ValidationError::new(
                "kernel NTP datagrams differ from independent client I/O",
            ));
        }
        let reply = captured[1]
            .first()
            .map(|b| wire::reply(b, &captured[0][0], local))
            .transpose()?;
        if reply != exchange.reply {
            return Err(ValidationError::new(
                "captured NTP status/origin differs from client",
            ));
        }
        observations.push(json!({"case":exchange.label,"cookie":exchange.cookie,"start_ns":exchange.start_ns,"end_ns":exchange.end_ns,
            "timeout":exchange.response.is_none(),"client_reply":exchange.reply,"captured_reply":reply,"request_bytes":exchange.request.len(),
            "client_request_sha256":sha256(&exchange.request),"captured_request_sha256":sha256(&captured[0][0]),
            "client_response_sha256":exchange.response.as_ref().map(|b|sha256(b)),"captured_response_sha256":captured[1].first().map(|b|sha256(b))}));
    }
    if seen[0].len() != egress.len() || seen[1].len() != ingress.len() {
        return Err(ValidationError::new("unaccounted NTP datagrams"));
    }
    Ok(observations)
}

pub(super) fn packages(
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
    local: bool,
    out: &Path,
) -> Result<Vec<Value>, ValidationError> {
    let mut packages = Vec::new();
    for (index, (path, rules, request, response)) in [
        (
            "protocols/ntp/client/main.gewy",
            include_str!("../../../../../protocols/ntp/client/main.gewy"),
            "send_request",
            "receive_response",
        ),
        (
            "protocols/ntp/query/main.gewy",
            include_str!("../../../../../protocols/ntp/query/main.gewy"),
            "send_query",
            "receive_response",
        ),
        (
            "protocols/ntp/sync/main.gewy",
            include_str!("../../../../../protocols/ntp/sync/main.gewy"),
            "send_sync_request",
            "receive_sync_response",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "compiled NTP package differs from workspace",
            ));
        }
        let export = reconstruct(ingress, egress, context, rules)?;
        let mut matched = Vec::new();
        for (phase, dir) in [(request, PacketDir::Egress), (response, PacketDir::Ingress)] {
            let cookies: BTreeSet<_> = export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .filter(|s| s.phase.as_deref() == Some(phase))
                .map(|s| {
                    export
                        .facts
                        .iter()
                        .find_map(|fact| match &fact.kind {
                            FactKind::PacketMeta(p) if fact.id == s.at && p.dir == dir => {
                                p.sk_cookie
                            }
                            _ => None,
                        })
                        .ok_or_else(|| ValidationError::new("NTP stage lacks packet provenance"))
                })
                .collect::<Result<_, _>>()?;
            let expected = if index == 2 && dir == PacketDir::Ingress && !local {
                BTreeSet::new()
            } else {
                context.sockets.iter().map(|s| s.0).collect()
            };
            if cookies != expected {
                return Err(ValidationError::new(format!(
                    "NTP package {path} phase {phase} differs: {cookies:?}"
                )));
            }
            matched.push(cookies);
        }
        let legacy = rules
            .replace(
                "ntp_message_observed:remote:ntp:request",
                &format!(
                    "datagram_observed:udp:remote:ntp:local_to_remote:byte0_mask:0xff:0x{}",
                    if index == 2 { "1b" } else { "23" }
                ),
            )
            .replace(
                "ntp_message_observed:remote:ntp:time_reply",
                "datagram_observed:udp:remote:ntp:remote_to_local:byte0_mask:0xff:0x24",
            )
            .replace(
                "ntp_message_observed:remote:ntp:response",
                "datagram_observed:udp:remote:ntp:remote_to_local:byte0_mask:0xff:0x24",
            );
        let old = reconstruct(ingress, egress, context, &legacy)?;
        let legacy_responses = old
            .program_flows
            .iter()
            .flat_map(|f| &f.stages)
            .filter(|s| s.phase.as_deref() == Some(response))
            .count();
        if legacy_responses != usize::from(local) {
            return Err(ValidationError::new(
                "NTP fixed-version regression was not reproduced",
            ));
        }
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"request_phase":request,"response_phase":response,
            "matched_request_cookies":matched[0],"matched_response_cookies":matched[1],"legacy_response_stages":legacy_responses,"replay_identical":true,
            "packet_only":true,"product_origin_correlation":false,"clock_discipline_proven":false}));
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kernel_reconciliation_rejects_missing_foreign_mutated_or_mistimed_datagrams() {
        let context = Context {
            sockets: [(1, 4000), (2, 4001)],
            remote_port: 123,
            addresses: ([127, 0, 0, 1], [127, 0, 0, 2]).into(),
            netns: 1,
            clock: (1, SystemTime::UNIX_EPOCH),
            end_ns: 10,
        };
        let request = wire::query(4, 0, true);
        let mut prefix = vec![0; 128];
        prefix[..request.len()].copy_from_slice(&request);
        let event = Datagram {
            sequence: 1,
            monotonic_ns: 3,
            cookie: 1,
            cpu: 0,
            ifindex: 1,
            source: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            destination: [127, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sport: 4000,
            dport: 123,
            total_len: 40,
            payload_len: 12,
            prefix_len: 12,
            ip_header_len: 20,
            ip_version: 4,
            prefix,
        };
        let exchanges = [Exchange {
            label: "timeout",
            cookie: 1,
            start_ns: 2,
            end_ns: 4,
            request,
            response: None,
            reply: None,
        }];
        assert!(
            reconcile(
                &exchanges,
                &[],
                std::slice::from_ref(&event),
                &context,
                true
            )
            .is_ok()
        );
        assert!(reconcile(&exchanges, &[], &[], &context, true).is_err());
        for kind in 0..4 {
            let mut bad = event.clone();
            match kind {
                0 => {
                    bad.cookie = 2;
                    bad.sport = 4001;
                }
                1 => bad.monotonic_ns = 5,
                2 => bad.prefix[0] ^= 1,
                _ => bad.dport = 53,
            }
            assert!(reconcile(&exchanges, &[], &[bad], &context, true).is_err());
        }
    }
}
