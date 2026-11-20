use super::super::product_udp::{self, Observation, Plan};
use super::*;

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "product-stun")?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut target = target::Target::start(&token, &out)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let probe: UdpSocket = socket(target.source, target.address)?.into();
        match exchange(&probe, 0, CASES[1]) {
            Ok(_) => break,
            Err(e) if Instant::now() >= deadline => {
                target.log(&out)?;
                return Err(ValidationError::new(format!("product STUN readiness: {e}")));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let neighbor = socket(target.source, target.address)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    let mut scope = Scope::enter(&token)?;
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
    for (entry, response_phase) in [
        ("binding", "receive_response"),
        ("binding-error", "receive_error_response"),
    ] {
        let plan = Plan {
            protocol: "stun",
            entry,
            port: 3478,
            request_phase: "send_request",
            response_phase,
        };
        packages.push(product_udp::package(&plan,&scope,&out.join(entry),&identities,neighbor_cookie,|| {
            let mut observations = Vec::new();
            for (socket,cookie,case) in CASES.into_iter().map(|c|(&first,identities[0].0,c))
                .chain(std::iter::once((&second,identities[1].0,CASES[1]))) {
                eprintln!("Product STUN {entry}: {}",case.label);
                let e = exchange(socket,cookie,case)?;
                observations.push(Observation {cookie,request:e.request,response:e.response,
                    request_matches:case.kind != 2,
                    response_matches:case.kind != 2 && ((entry == "binding-error") == (case.kind == 1)),
                    detail:json!({"case":case.label,"reply":e.reply,"local_address":e.local_address.to_string(),
                        "product_transaction_correlation":false})});
            }
            let excluded = exchange(&neighbor,neighbor_cookie,CASES[1])?;
            Ok((observations,json!({"cookie":neighbor_cookie.to_string(),"reply":excluded.reply})))
        })?);
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.cleanup()?;
    let checks = [
        "formal_CLI_owns_product_BPF",
        "real_private_coturn",
        "XOR_MAPPED_ADDRESS",
        "420_unknown_required_attribute",
        "optional_padding",
        "short_timeout_then_recovery",
        "full_payload_multiset_matches",
        "outside_cgroup_neighbor_excluded",
        "package_phases_and_replay",
        "owned_cleanup",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"ok":true,"mode":"production-stun","ip_version":4,
            "production_cli":true,"validation_capture_used":false,"target":target.metadata,
            "selected_sockets":identities.map(|(c,p)|json!({"cookie":c.to_string(),"port":p})),
            "neighbor_socket_created_outside_scope":true,"excluded_neighbor_cookie":neighbor_cookie.to_string(),
            "packages":packages,"checks":checks,"owned_resources_removed":true,
            "not_proven":["TURN_relay","NAT_traversal","ICE","integrity_auth","product_transaction_correlation","IPv6","high_load","PID_attribution"]
        }))?,
    )?;
    Ok(ValidationReport {
        name: "Production CLI real STUN field validation".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
