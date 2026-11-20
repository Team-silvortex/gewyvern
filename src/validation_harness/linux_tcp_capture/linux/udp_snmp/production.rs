use super::super::product_udp::{self, Observation, Plan};
use super::*;

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "product-snmp")?;
    let mut target = prepare_target(&out)?;
    let neighbor = socket(target.source, target.address)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    let mut scope = Scope::enter(&target.token)?;
    let sockets = [
        socket(target.source, target.address)?,
        socket(target.source, target.address)?,
    ];
    let identities = [
        (sockets[0].cookie()?, port(&sockets[0])?),
        (sockets[1].cookie()?, port(&sockets[1])?),
    ];
    let [first, second] = sockets.map(UdpSocket::from);
    let mut packages = Vec::new();
    for (entry, pdu, request_phase, response_phase) in [
        ("get", 0xa0, "send_get_request", "receive_get_response"),
        (
            "get-next",
            0xa1,
            "send_get_next_request",
            "receive_get_next_response",
        ),
        ("bulk", 0xa5, "send_bulk_request", "receive_bulk_response"),
    ] {
        let plan = Plan {
            protocol: "snmp",
            entry,
            port: 161,
            request_phase,
            response_phase,
        };
        packages.push(product_udp::package(&plan, &scope, &out.join(entry), &identities, neighbor_cookie, || {
            let mut observations = Vec::new();
            for (socket, cookie, case) in CASES.into_iter().map(|c| (&first, identities[0].0, c))
                .chain(std::iter::once((&second, identities[1].0, CASES[0]))) {
                eprintln!("Product SNMP {entry}: {}",case.label);
                let e = exchange(socket, cookie, case)?;
                observations.push(Observation {cookie, request:e.request, response:e.response,
                    request_matches:case.pdu == pdu, response_matches:!case.timeout,
                    detail:json!({"case":case.label,"request_id":case.id,"no_such_object":case.unknown,
                        "response_is_generic_candidate":true,"transaction_correlation":false})});
            }
            let excluded = exchange(&neighbor, neighbor_cookie, CASES[0])?;
            Ok((observations,json!({"reply_validated":excluded.response.is_some(),"cookie":neighbor_cookie.to_string()})))
        })?);
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.container.cleanup()?;
    target.network.cleanup()?;
    let checks = [
        "formal_CLI_owns_product_BPF",
        "real_private_Net_SNMP",
        "GET_GETNEXT_GETBULK",
        "noSuchObject_is_not_success",
        "wrong_community_timeout_then_recovery",
        "full_payload_multiset_matches",
        "outside_cgroup_neighbor_excluded",
        "package_phases_and_replay",
        "owned_cleanup",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"ok":true,"mode":"production-snmp","ip_version":4,
            "production_cli":true,"validation_capture_used":false,"target":target.metadata,
            "selected_sockets":identities.map(|(c,p)|json!({"cookie":c.to_string(),"port":p})),
            "neighbor_socket_created_outside_scope":true,"excluded_neighbor_cookie":neighbor_cookie.to_string(),
            "packages":packages,"checks":checks,"owned_resources_removed":true,
            "not_proven":["SNMPv3_auth_privacy","SET_trap_inform","full_MIB_walk","product_transaction_correlation","IPv6","high_load","PID_attribution"]
        }))?,
    )?;
    Ok(ValidationReport {
        name: "Production CLI real SNMP field validation".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
