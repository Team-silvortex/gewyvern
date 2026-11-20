use super::super::product_udp::{self, Observation, Plan};
use super::*;

fn phase(local: bool, out: &Path) -> Result<Value, ValidationError> {
    fs::create_dir(out)?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut target = target::Target::start(&token, local, out)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let probe: UdpSocket = socket(target.source, target.address)?.into();
        match exchange(&probe, 0, CASES[0], local) {
            Ok(_) => break,
            Err(e) if Instant::now() >= deadline => {
                target.log(out)?;
                return Err(ValidationError::new(format!("product NTP readiness: {e}")));
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
    for (entry, request_phase, response_phase) in [
        ("client", "send_request", "receive_response"),
        ("query", "send_query", "receive_response"),
        ("sync", "send_sync_request", "receive_sync_response"),
    ] {
        let plan = Plan {
            protocol: "ntp",
            entry,
            port: 123,
            request_phase,
            response_phase,
        };
        packages.push(product_udp::package(&plan,&scope,&out.join(entry),&identities,neighbor_cookie,|| {
            let mut observations = Vec::new();
            for (socket,cookie,case) in CASES.into_iter().map(|c|(&first,identities[0].0,c))
                .chain(std::iter::once((&second,identities[1].0,CASES[1]))) {
                eprintln!("Product NTP local={local} {entry}: {}",case.label);
                let e = exchange(socket,cookie,case,local)?;
                observations.push(Observation {cookie,request:e.request,response:e.response,
                    request_matches:!case.short,response_matches:!case.short && (entry != "sync" || local),
                    detail:json!({"case":case.label,"reply":e.reply,"clock_discipline_proven":false})});
            }
            let excluded = exchange(&neighbor,neighbor_cookie,CASES[1],local)?;
            Ok((observations,json!({"cookie":neighbor_cookie.to_string(),"reply":excluded.reply})))
        })?);
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.cleanup()?;
    Ok(json!({"local_reference":local,"target":target.metadata,
        "selected_sockets":identities.map(|(c,p)|json!({"cookie":c.to_string(),"port":p})),
        "neighbor_socket_created_outside_scope":true,"excluded_neighbor_cookie":neighbor_cookie.to_string(),
        "packages":packages,"owned_resources_removed":true}))
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "product-ntp")?;
    let phases = [
        phase(false, &out.join("unsynchronised"))?,
        phase(true, &out.join("local-reference"))?,
    ];
    let checks = [
        "formal_CLI_owns_product_BPF",
        "real_private_chrony",
        "host_clock_control_disabled",
        "NTPv3_v4",
        "unsynchronised_not_time_reply",
        "short_timeout_then_recovery",
        "full_payload_multiset_matches",
        "outside_cgroup_neighbor_excluded",
        "package_phases_and_replay",
        "owned_cleanup",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"ok":true,"mode":"production-ntp","ip_version":4,
            "production_cli":true,"validation_capture_used":false,"phases":phases,"checks":checks,
            "owned_resources_removed":true,"not_proven":["clock_accuracy_or_discipline","NTS_auth","external_time_sources","product_origin_correlation","IPv6","high_load","PID_attribution"]
        }))?,
    )?;
    Ok(ValidationReport {
        name: "Production CLI real NTP field validation".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
