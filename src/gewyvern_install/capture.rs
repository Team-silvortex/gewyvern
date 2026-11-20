use super::*;

pub(super) fn validate_request(
    request: &GewyvernInstallerRequest,
) -> Result<(), GewyvernInstallError> {
    if request.capture.is_some() && !cfg!(target_os = "linux") {
        return Err(GewyvernInstallError::InvalidRequest);
    }
    Ok(())
}

fn roots(root: &Path) -> [(&'static str, PathBuf); 4] {
    [
        ("GEWY_CONFIG_HOME", root.join("config")),
        ("GEWY_DATA_HOME", root.join("data")),
        ("GEWY_STATE_HOME", root.join("state")),
        ("GEWY_CACHE_HOME", root.join("cache")),
    ]
}

#[cfg(target_os = "linux")]
pub(super) fn systemd_environment(
    root: &Path,
    plan: &ServicePlan,
) -> Result<String, GewyvernInstallError> {
    if plan.capture.is_none() {
        return Ok(String::new());
    }
    let mut output = String::new();
    for (key, path) in roots(root) {
        output.push_str(&format!(
            "Environment={}\n",
            super::systemd_quote(&format!("{key}={}", path_text(&path)?))?
        ));
    }
    output.push_str("UnsetEnvironment=GEWY_CERTIFICATE_ROOT GEWY_TRUST_ROOT GEWY_AUTHORITY_ROOT GEWY_IDENTITY_ROOT GEWY_CERTIFICATE_STATE_ROOT\n");
    // Exit 3 reports accounted loss, not a crash that should cause a restart loop.
    output.push_str("SuccessExitStatus=3\n");
    Ok(output)
}

pub(super) fn run(generation: &Path, plan: &ServicePlan, token: &str) -> Result<i32, MachineError> {
    let fail = |detail: &str| {
        MachineError::new(
            "capture_service_configuration_invalid",
            ErrorCategory::Configuration,
            detail,
            false,
            1,
        )
    };
    if !cfg!(target_os = "linux") {
        return Err(fail("installed capture requires Linux"));
    }
    let capture = plan
        .capture
        .as_ref()
        .ok_or_else(|| fail("missing capture plan"))?;
    capture.validate().map_err(fail)?;
    let root = generation
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| fail("invalid generation layout"))?;
    validate_root(root).map_err(service_error)?;
    for (key, expected) in roots(root) {
        if env::var_os(key).as_deref() != Some(expected.as_os_str()) {
            return Err(fail(
                "capture service requires its generated private runtime environment",
            ));
        }
    }
    for key in [
        "GEWY_CERTIFICATE_ROOT",
        "GEWY_TRUST_ROOT",
        "GEWY_AUTHORITY_ROOT",
        "GEWY_IDENTITY_ROOT",
        "GEWY_CERTIFICATE_STATE_ROOT",
    ] {
        if env::var_os(key).is_some() {
            return Err(fail("capture service rejects certificate path overrides"));
        }
    }
    for (_, path) in roots(root) {
        create_private_dir(&path).map_err(service_error)?;
    }
    let output = root.join("state/capture");
    create_private_dir(&output).map_err(service_error)?;
    let args = vec![
        "--serve".into(),
        "--json".into(),
        "--capture-cgroup".into(),
        capture.cgroup.clone(),
        "--capture-udp-port".into(),
        capture.udp_port.to_string(),
        "--capture-window-ms".into(),
        capture.window_ms.to_string(),
        "--capture-ring-kib".into(),
        capture.ring_kib.to_string(),
        "--capture-max-facts".into(),
        capture.max_facts.to_string(),
        "--capture-status".into(),
        path_text(&output.join("status.json")).map_err(service_error)?,
        "--out".into(),
        path_text(&output.join("export.json")).map_err(service_error)?,
    ];
    let cli = crate::Cli::from_args(args).map_err(|message| fail(&message))?;
    crate::capture_runtime::run_with_api_factory(&cli, || {
        crate::data_api::start_tls_api_service(
            &plan.listen,
            &plan.tls_certificate_file,
            &plan.tls_private_key_file,
            token,
        )
        .map(Some)
        .map_err(|detail| {
            MachineError::new(
                "capture_tls_start_failed",
                ErrorCategory::Environment,
                detail,
                true,
                1,
            )
        })
    })
}

pub(super) fn validate_health(
    value: &serde_json::Value,
    plan: &UdpCapturePlan,
) -> Result<(), GewyvernInstallError> {
    let snapshot: gewyvern::live_capture::CaptureSnapshot = serde_json::from_value(
        value
            .get("capture")
            .cloned()
            .ok_or(GewyvernInstallError::HealthProof)?,
    )
    .map_err(|_| GewyvernInstallError::HealthProof)?;
    if snapshot.schema_version != 1
        || snapshot.run_id.is_empty()
        || snapshot.cgroup != plan.cgroup
        || snapshot.udp_port != plan.udp_port
        || snapshot.state != gewyvern::live_capture::CaptureState::Running
        || snapshot.incomplete
        || snapshot.has_loss()
        || snapshot.final_accounted
    {
        return Err(GewyvernInstallError::HealthProof);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ready_requires_matching_live_capture_not_just_a_healthy_api() {
        let plan = UdpCapturePlan::new("/sys/fs/cgroup/test", 53);
        assert!(validate_health(&serde_json::json!({"ok": true}), &plan).is_err());
        let snapshot = gewyvern::live_capture::CaptureSnapshot {
            schema_version: 1,
            run_id: "test-1".into(),
            cgroup: plan.cgroup.clone(),
            udp_port: 53,
            state: gewyvern::live_capture::CaptureState::Running,
            kernel: Default::default(),
            received: 0,
            retained: 0,
            userspace_dropped: 0,
            decode_rejected: 0,
            final_accounted: false,
            incomplete: false,
        };
        let value = serde_json::json!({"ok":true,"capture":snapshot});
        assert!(validate_health(&value, &plan).is_ok());
        for (key, replacement) in [
            ("cgroup", serde_json::json!("/sys/fs/cgroup/other")),
            ("udp_port", serde_json::json!(54)),
            ("state", serde_json::json!("stopped")),
            ("incomplete", serde_json::json!(true)),
            ("final_accounted", serde_json::json!(true)),
        ] {
            let mut invalid = value.clone();
            invalid["capture"][key] = replacement;
            assert!(validate_health(&invalid, &plan).is_err(), "{key}");
        }
        let mut invalid = value;
        invalid["capture"]["kernel"]["ring_dropped"] = serde_json::json!("1");
        assert!(validate_health(&invalid, &plan).is_err());
    }
}
