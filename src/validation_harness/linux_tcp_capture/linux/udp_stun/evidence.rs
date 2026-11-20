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
                        return Err(ValidationError::new("STUN datagram reused"));
                    }
                    captured[i].push(bytes.to_vec());
                }
            }
        }
        if captured[0] != [exchange.request.clone()]
            || captured[1] != exchange.response.iter().cloned().collect::<Vec<_>>()
        {
            return Err(ValidationError::new(
                "kernel STUN datagrams differ from independent client I/O",
            ));
        }
        let reply = captured[1]
            .first()
            .map(|b| {
                wire::reply(
                    b,
                    &captured[0][0],
                    exchange.local_address,
                    exchange.label == "unknown-required",
                )
            })
            .transpose()?;
        if reply != exchange.reply {
            return Err(ValidationError::new(
                "captured STUN transaction/address differs from client",
            ));
        }
        observations.push(json!({"case":exchange.label,"cookie":exchange.cookie,"start_ns":exchange.start_ns,"end_ns":exchange.end_ns,
            "timeout":exchange.response.is_none(),"client_reply":exchange.reply,"captured_reply":reply,"request_bytes":exchange.request.len(),
            "client_request_sha256":sha256(&exchange.request),"captured_request_sha256":sha256(&captured[0][0]),
            "client_response_sha256":exchange.response.as_ref().map(|b|sha256(b)),"captured_response_sha256":captured[1].first().map(|b|sha256(b))}));
    }
    if seen[0].len() != egress.len() || seen[1].len() != ingress.len() {
        return Err(ValidationError::new("unaccounted STUN datagrams"));
    }
    Ok(observations)
}

pub(super) fn packages(
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
    out: &Path,
) -> Result<Vec<Value>, ValidationError> {
    let mut packages = Vec::new();
    for (path, rules, phase, expected) in [
        (
            "protocols/stun/binding/main.gewy",
            include_str!("../../../../../protocols/stun/binding/main.gewy"),
            "receive_response",
            vec![context.sockets[0].0, context.sockets[1].0],
        ),
        (
            "protocols/stun/binding-error/main.gewy",
            include_str!("../../../../../protocols/stun/binding-error/main.gewy"),
            "receive_error_response",
            vec![context.sockets[0].0],
        ),
    ] {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "compiled STUN package differs from workspace",
            ));
        }
        let export = reconstruct(ingress, egress, context, rules)?;
        let cookies = |export: &crate::export::ExportBundle,
                       phase: &str|
         -> Result<BTreeSet<u64>, ValidationError> {
            export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .filter(|s| s.phase.as_deref() == Some(phase))
                .map(|s| {
                    export
                        .facts
                        .iter()
                        .find_map(|f| match &f.kind {
                            FactKind::PacketMeta(p)
                                if f.id == s.at
                                    && p.dir
                                        == if phase == "send_request" {
                                            PacketDir::Egress
                                        } else {
                                            PacketDir::Ingress
                                        } =>
                            {
                                p.sk_cookie
                            }
                            _ => None,
                        })
                        .ok_or_else(|| ValidationError::new("STUN stage lacks packet provenance"))
                })
                .collect()
        };
        let requests = cookies(&export, "send_request")?;
        let responses = cookies(&export, phase)?;
        if requests != context.sockets.iter().map(|s| s.0).collect()
            || responses != expected.into_iter().collect()
        {
            return Err(ValidationError::new(
                "STUN package phase/socket scope differs",
            ));
        }
        let short_matches = |bundle: &crate::export::ExportBundle| {
            bundle
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .filter(|s| s.phase.as_deref() == Some("send_request"))
                .filter(|s| {
                    bundle.facts.iter().any(|f| {
                        f.id == s.at
                            && matches!(&f.kind,FactKind::PacketMeta(p) if p.payload_bytes.len()==2)
                    })
                })
                .count()
        };
        let legacy = rules
            .replace(
                "stun_binding_observed:remote:stun:request",
                "datagram_observed:udp:remote:stun:local_to_remote:prefix2:0x0001",
            )
            .replace(
                "stun_binding_observed:remote:stun:response",
                "datagram_observed:udp:remote:stun:remote_to_local:prefix2:0x0101",
            )
            .replace(
                "stun_binding_observed:remote:stun:error",
                "datagram_observed:udp:remote:stun:remote_to_local:prefix2:0x0111",
            );
        let old = reconstruct(ingress, egress, context, &legacy)?;
        if short_matches(&old) != 1 || short_matches(&export) != 0 {
            return Err(ValidationError::new(
                "STUN short-datagram false positive was not reproduced",
            ));
        }
        fs::write(
            out.join(format!("package-{}-export.json", packages.len())),
            export.to_json(),
        )?;
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),
            "matched_request_cookies":requests,"matched_response_cookies":responses,
            "legacy_short_request_matches":1,"short_request_matches":0,
            "packet_only":true,"replay_identical":true,"product_transaction_correlation":false,
            "NAT_traversal_proven":false}));
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_rejects_missing_foreign_mutated_or_unaccounted_datagrams() {
        let context = Context {
            sockets: [(1, 4000), (2, 4001)],
            remote_port: 3478,
            addresses: ([127, 0, 0, 1], [127, 0, 0, 2]).into(),
            netns: 1,
            clock: (1, SystemTime::UNIX_EPOCH),
            end_ns: 10,
        };
        let request = wire::query(1, 2);
        let mut prefix = vec![0; 128];
        prefix[..2].copy_from_slice(&request);
        let event = Datagram {
            sequence: 1,
            monotonic_ns: 3,
            cookie: 1,
            cpu: 0,
            ifindex: 1,
            source: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            destination: [127, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sport: 4000,
            dport: 3478,
            total_len: 30,
            payload_len: 2,
            prefix_len: 2,
            ip_header_len: 20,
            ip_version: 4,
            prefix,
        };
        let exchanges = [Exchange {
            label: "short-timeout",
            cookie: 1,
            start_ns: 2,
            end_ns: 4,
            request,
            response: None,
            reply: None,
            local_address: "127.0.0.1:4000".parse().unwrap(),
        }];
        assert!(reconcile(&exchanges, &[], std::slice::from_ref(&event), &context).is_ok());
        assert!(reconcile(&exchanges, &[], &[], &context).is_err());
        for kind in 0..5 {
            let mut bad = event.clone();
            match kind {
                0 => bad.cookie = 3,
                1 => bad.monotonic_ns = 9,
                2 => bad.prefix[1] = 2,
                3 => bad.dport = 123,
                _ => bad.prefix_len = 1,
            }
            assert!(reconcile(&exchanges, &[], &[bad], &context).is_err());
        }
        let mut extra = event.clone();
        extra.sequence = 2;
        extra.monotonic_ns = 5;
        assert!(reconcile(&exchanges, &[], &[event.clone(), extra], &context).is_err());
        assert!(reconcile(&exchanges, &[], &[event.clone(), event], &context).is_err());
    }
}
