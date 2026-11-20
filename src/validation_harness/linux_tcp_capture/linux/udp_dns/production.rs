//! A real-target consumer of the formal CLI; no validation producer or fact filling.
use super::super::product_udp::{self, finish, start};
use super::*;

fn check_bytes(
    export: &ExportBundle,
    exchanges: &[Exchange],
    identities: &[(u64, u16); 2],
    family: u16,
    neighbor: u64,
) -> Result<(), ValidationError> {
    let mut expected = Vec::new();
    for exchange in exchanges {
        expected.push((exchange.cookie, 1, exchange.query.clone()));
        expected.extend(
            exchange
                .responses
                .iter()
                .map(|b| (exchange.cookie, 0, b.clone())),
        );
    }
    product_udp::check_bytes(export, &expected, identities, family, neighbor, 53)
}

fn check_phases(
    export: &ExportBundle,
    entry: &str,
    identities: &[(u64, u16); 2],
) -> Result<Vec<String>, ValidationError> {
    let phases: &[&str] = if entry == "udp" {
        &["send_request", "receive_reply"]
    } else {
        &[
            "receive_formerr",
            "receive_servfail",
            "receive_nxdomain",
            "receive_refused",
        ]
    };
    let wanted = if entry == "udp" {
        identities.iter().map(|(c, _)| *c).collect()
    } else {
        BTreeSet::from([identities[0].0])
    };
    for phase in phases {
        let dir = if *phase == "send_request" {
            PacketDir::Egress
        } else {
            PacketDir::Ingress
        };
        if stage_cookies(export, phase, dir)? != wanted {
            return Err(ValidationError::new(format!(
                "production CLI DNS phase {phase} differs from oracle"
            )));
        }
    }
    // An error-looking response on another socket, TC, or an absent reply must
    // not satisfy an error phase. Check every emitted candidate's actual bytes.
    for stage in export.program_flows.iter().flat_map(|flow| &flow.stages) {
        let code = match stage.phase.as_deref() {
            Some("receive_formerr") => 1,
            Some("receive_servfail") => 2,
            Some("receive_nxdomain") => 3,
            Some("receive_refused") => 5,
            _ => continue,
        };
        let valid = export.facts.iter().any(|fact| fact.id == stage.at && matches!(&fact.kind,
            FactKind::PacketMeta(p) if p.dir == PacketDir::Ingress && p.sk_cookie == Some(identities[0].0)
            && p.payload_bytes.get(&2).is_some_and(|b| b & 0x80 != 0)
            && p.payload_bytes.get(&3).is_some_and(|b| b & 15 == code)));
        if !valid {
            return Err(ValidationError::new(
                "DNS error phase lacks actual matching packet",
            ));
        }
    }
    Ok(phases.iter().map(|p| (*p).to_string()).collect())
}

pub(super) fn run(out: Option<PathBuf>, ipv6: bool) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(
        out,
        if ipv6 {
            "product-dns-ipv6"
        } else {
            "product-dns-ipv4"
        },
    )?;
    let binary = fs::canonicalize(repo_root().join("target/debug/gewyvern"))?;
    let binary_sha256 = sha256(&fs::read(&binary)?);
    let mut target = prepare_target(out.clone(), ipv6)?;
    let endpoint = SocketAddr::from((target.destination, 53));
    // Socket cgroup membership is assigned at creation, not when send is called.
    let neighbor = socket(target.source, endpoint)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    let mut scope = Scope::enter(&target.token)?;
    let sockets = [
        socket(target.source, endpoint)?,
        socket(target.source, endpoint)?,
    ];
    let identities = [
        (sockets[0].cookie()?, port(&sockets[0])?),
        (sockets[1].cookie()?, port(&sockets[1])?),
    ];
    if identities[0].0 == identities[1].0
        || identities
            .iter()
            .any(|(c, p)| *c == 0 || *p == 0 || *c == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "production DNS socket identities are not unique",
        ));
    }
    let [first, second] = sockets.map(UdpSocket::from);
    let mut packages = Vec::new();
    for entry in ["udp", "error"] {
        let directory = out.join(entry);
        let mut product = start(&binary, &scope, &directory, "dns", entry, 53)?;
        let mut exchanges = Vec::new();
        for (question, expected) in [
            ("ok.test.", Some((0, false))),
            ("form.test.", Some((1, false))),
            ("fail.test.", Some((2, false))),
            ("nx.test.", Some((3, false))),
            ("deny.test.", Some((5, false))),
            ("cut.test.", Some((0, true))),
            ("cut.test.", Some((0, false))),
            ("drop.test.", None),
            ("ok.test.", Some((0, false))),
        ] {
            eprintln!("Production DNS {entry} case: {question}");
            exchanges.push(exchange(&first, identities[0].0, question, expected)?);
        }
        exchanges.push(exchange(
            &second,
            identities[1].0,
            "ok.test.",
            Some((0, false)),
        )?);
        let excluded = exchange(&neighbor, neighbor_cookie, "nx.test.", Some((3, false)))?;
        let (export, snapshot) = finish(&mut product, &directory)?;
        check_bytes(
            &export,
            &exchanges,
            &identities,
            target.addresses.l3_proto(),
            neighbor_cookie,
        )?;
        let phases = check_phases(&export, entry, &identities)?;
        if snapshot.retained != export.facts.len() as u64 {
            return Err(ValidationError::new(
                "product DNS fact/counter total differs",
            ));
        }
        let links = tool(
            Command::new("bpftool")
                .args(["-j", "cgroup", "show"])
                .arg(scope.path()),
            &out,
        )?;
        if !links.iter().all(u8::is_ascii_whitespace)
            && !serde_json::from_slice::<Vec<Value>>(&links)?.is_empty()
        {
            return Err(ValidationError::new("product DNS hooks survived shutdown"));
        }
        let observations: Vec<_> = exchanges.iter().map(|e| json!({"cookie":e.cookie.to_string(),
            "question_wire_hex":e.query[12..].iter().map(|b|format!("{b:02x}")).collect::<String>(),
            "reply":e.reply,"response_count":e.responses.len(),"timeout":e.responses.is_empty(),
            "query_sha256":sha256(&e.query),"response_sha256":e.responses.iter().map(|b|sha256(b)).collect::<Vec<_>>()})).collect();
        fs::write(
            directory.join("oracle.json"),
            serde_json::to_vec_pretty(&observations)?,
        )?;
        packages.push(json!({"entry":entry,"source_sha256":sha256(&fs::read(repo_root().join(format!("protocols/dns/{entry}/main.gewy")))?),
            "capture":snapshot,"facts":export.facts.len(),"phases":phases,"observations":observations,
            "excluded_neighbor_reply":excluded.reply,"replay_identical":true,"full_payload_multiset_matches":true,
            "packet_only":true,"process_attribution":false}));
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.container.cleanup()?;
    target.network.cleanup()?;
    let checks = [
        "formal_CLI_owns_product_BPF",
        "real_private_CoreDNS",
        "same_ID_socket_isolation",
        "full_packet_payloads_match_independent_client_IO",
        "four_errors_truncation_timeout_recovery",
        "outside_cgroup_neighbor_excluded",
        "both_DNS_packages_replay",
        "fresh_second_capture",
        "owned_cleanup",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "mode":if ipv6 {"production-dns-ipv6"} else {"production-dns-ipv4"},"ip_version":target.addresses.version(),
        "production_cli":true,"validation_capture_used":false,"binary_sha256":binary_sha256,
        "pinned_target_truncation_alternates":true,
        "image":IMAGE,"image_id":String::from_utf8_lossy(&target.image_id).trim(),"config_sha256":sha256(CONFIG.as_bytes()),
        "container_id":target.container_info["Id"],"network_id":target.network_info["Id"],
        "internal_network":true,"published_ports":false,"source_address":target.source.to_string(),"target_address":target.destination.to_string(),
        "selected_sockets":identities.map(|(c,p)| json!({"cookie":c.to_string(),"port":p})),
        "excluded_neighbor_cookie":neighbor_cookie.to_string(),"neighbor_socket_created_outside_scope":true,
        "packages":packages,"checks":checks,"owned_resources_removed":true,
        "not_proven":["full_DNS_parser_or_vulnerability_detection","EDNS_DNSSEC_DoH_DoT","AAAA_queries",
        "TCP_fallback","recursive_resolution_or_cache","cross_window_transactions","full_IP_addresses_in_product_facts",
        "PID_attribution","high_load","other_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: format!(
            "Production CLI real IPv{} DNS field validation",
            if ipv6 { 6 } else { 4 }
        ),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{FactId, SessionId};
    use crate::packet_capture::{CapturedUdpDatagram, PacketObservation};
    use crate::runtime::{RuntimeSession, SessionConfig};

    #[test]
    fn oracle_rejects_missing_duplicate_foreign_and_corrupted_packet_evidence() {
        let query = wire::query("ok.test.", ID).unwrap();
        let mut reply = query.clone();
        reply[2] |= 0x80;
        let exchanges = vec![Exchange {
            cookie: 7,
            start_ns: 1,
            end_ns: 2,
            query: query.clone(),
            responses: vec![reply.clone()],
            reply: Some(wire::parse_reply(&reply, &query).unwrap()),
        }];
        let mut export = RuntimeSession::start(
            SessionConfig::for_template(crate::template::udp_debug_template()).unwrap(),
        )
        .unwrap()
        .into_export_bundle();
        export.ingest_trust_mode = "unverified-local".into();
        for (id, bytes, dir) in [
            (1, &query, PacketDir::Egress),
            (2, &reply, PacketDir::Ingress),
        ] {
            let local: SocketAddr = "127.0.0.1:40000".parse().unwrap();
            let remote: SocketAddr = "127.0.0.2:53".parse().unwrap();
            let (source, destination) = if dir == PacketDir::Egress {
                (local, remote)
            } else {
                (remote, local)
            };
            export.facts.push(
                CapturedUdpDatagram {
                    source,
                    destination,
                    ip_header_len: 20,
                    total_len: 28 + bytes.len() as u32,
                    payload_len: bytes.len() as u16,
                    payload_prefix: bytes,
                }
                .into_fact(PacketObservation {
                    id: FactId(id),
                    session: SessionId(1),
                    ts: UNIX_EPOCH,
                    cpu: 0,
                    ifindex: 1,
                    netns: 1,
                    socket_cookie: 7,
                    direction: dir,
                })
                .unwrap(),
            );
        }
        let identities = [(7, 40000), (8, 40001)];
        assert!(check_bytes(&export, &exchanges, &identities, 0x800, 9).is_ok());
        let mut missing = export.clone();
        missing.facts.pop();
        assert!(check_bytes(&missing, &exchanges, &identities, 0x800, 9).is_err());
        let mut duplicate = export.clone();
        duplicate.facts.push(export.facts[0].clone());
        assert!(check_bytes(&duplicate, &exchanges, &identities, 0x800, 9).is_err());
        for variant in 0..5 {
            let mut invalid = export.clone();
            let FactKind::PacketMeta(packet) = &mut invalid.facts[0].kind else {
                unreachable!()
            };
            match variant {
                0 => packet.sk_cookie = Some(9),
                1 => packet.l3_proto = 0x86dd,
                2 => {
                    packet.payload_bytes.insert(2, 0x81);
                }
                3 => {
                    packet.payload_bytes.remove(&3);
                }
                _ => packet.remote_port = Some(54),
            }
            assert!(check_bytes(&invalid, &exchanges, &identities, 0x800, 9).is_err());
        }
    }
}
