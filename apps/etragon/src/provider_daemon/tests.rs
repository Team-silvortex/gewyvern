use super::*;

fn input() -> String {
    include_str!("../../tests/fixtures/missing_transition_analysis.json").to_owned()
}

fn config() -> ProviderConfig {
    ProviderConfig::from_json(r#"{"id":"test-provider","model":"test-model","endpoint":"http://127.0.0.1:1/v1/chat/completions"}"#).unwrap()
}

fn options() -> ProviderDaemonOptions {
    ProviderDaemonOptions {
        bind: "127.0.0.1:0".parse().unwrap(),
        interval_ms: 100,
        config: config(),
        source_ca: None,
    }
}

fn state() -> SharedSnapshot {
    Arc::new(Mutex::new(ProviderSnapshot {
        status: "starting",
        ..Default::default()
    }))
}

fn output() -> etragon::EngineOutput {
    etragon::EngineOutput::with_augmentation(etragon::AnalysisAugmentation::new(
        "external-advisory",
        "test",
        "review this candidate",
        "candidate",
        None,
    ))
}

#[test]
fn cache_is_checked_before_inference_and_only_uses_allowlisted_evidence() {
    let state = state();
    let mut poller = ProviderPoller { cached: None };
    poller
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    let success = state.lock().unwrap().last_success_unix_ms;
    let mut same: Value = serde_json::from_str(&input()).unwrap();
    same["unrelated_timestamp"] = json!(1234);
    same["augmentations"] = json!([{"summary":"do not put this in the cache key"}]);
    for _ in 0..3 {
        poller
            .poll(
                &state,
                100,
                || Ok(serde_json::to_string_pretty(&same).unwrap()),
                |_| panic!("cache must skip provider call"),
            )
            .unwrap();
    }
    let snapshot = state.lock().unwrap();
    assert_eq!(snapshot.analysis_runs, 1);
    assert_eq!(snapshot.cache_hits, 3);
    assert_eq!(snapshot.cycle, 4);
    assert_eq!(snapshot.last_success_unix_ms, success);
    assert_eq!(snapshot.status, "ready");
}

#[test]
fn changed_evidence_hides_old_advice_while_waiting_and_on_failure() {
    let state = state();
    let mut poller = ProviderPoller { cached: None };
    poller
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    let mut changed: Value = serde_json::from_str(&input()).unwrap();
    changed["primary_failure_mode"] = json!("different_failure");
    let delay = poller
        .poll(
            &state,
            100,
            || Ok(changed.to_string()),
            |_| {
                let snapshot = state.lock().unwrap();
                assert_eq!(snapshot.status, "analyzing");
                assert!(snapshot.output.is_none());
                drop(snapshot);
                let response = route(
                    "GET /v1/latest/output.json HTTP/1.1\r\n\r\n",
                    "127.0.0.1".parse().unwrap(),
                    &DaemonAccessPolicy::default(),
                    &state,
                    &options(),
                )
                .unwrap();
                assert!(response.starts_with("HTTP/1.1 503"));
                assert!(response.contains("\"output\":null"));
                Err(ProviderError::HttpStatus(429))
            },
        )
        .unwrap();
    assert_eq!(delay, 1000);
    assert!(poller.cached.is_none());
    let snapshot = state.lock().unwrap();
    assert_eq!(snapshot.status, "error");
    assert!(snapshot.output.is_none());
    assert_eq!(
        snapshot.last_error.as_deref(),
        Some("etragon_provider_http_status: 429")
    );
    assert!(snapshot.last_success_unix_ms.is_some());
    assert_eq!(snapshot.analysis_runs, 2);
}

#[test]
fn upstream_failure_invalidates_cache_and_recovery_reanalyzes() {
    let state = state();
    let mut poller = ProviderPoller { cached: None };
    poller
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    assert_eq!(
        poller
            .poll(
                &state,
                100,
                || Err(ProviderError::Transport),
                |_| panic!("no input")
            )
            .unwrap(),
        1000
    );
    assert_eq!(
        state.lock().unwrap().last_error.as_deref(),
        Some("etragon_source_transport_failed")
    );
    assert!(poller.cached.is_none());
    poller
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    let snapshot = state.lock().unwrap();
    assert_eq!(snapshot.analysis_runs, 2);
    assert_eq!(snapshot.cache_hits, 0);
    assert_eq!(snapshot.consecutive_failures, 0);
    assert!(snapshot.last_error.is_none());
}

#[test]
fn malformed_snapshot_never_calls_inference_and_backoff_is_bounded() {
    let state = state();
    let mut poller = ProviderPoller { cached: None };
    for expected in [1000, 2000, 4000, 8000, 16000, 32000, 60000, 60000] {
        let delay = poller
            .poll(
                &state,
                100,
                || Ok("{}".into()),
                |_| panic!("invalid evidence"),
            )
            .unwrap();
        assert_eq!(delay, expected);
    }
    assert_eq!(state.lock().unwrap().analysis_runs, 0);
    assert!(state.lock().unwrap().output.is_none());
    assert_eq!(
        poller
            .poll(&state, 120_000, || Ok("{}".into()), |_| panic!())
            .unwrap(),
        120_000
    );
}

#[test]
fn a_new_resident_does_not_restore_the_previous_provider_cache() {
    let state = state();
    ProviderPoller { cached: None }
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    ProviderPoller { cached: None }
        .poll(&state, 100, || Ok(input()), |_| Ok(output()))
        .unwrap();
    assert_eq!(state.lock().unwrap().analysis_runs, 2);
    assert_eq!(state.lock().unwrap().cache_hits, 0);
}

#[test]
fn capabilities_and_routes_expose_advice_without_training_or_execution() {
    let options = options();
    let capabilities: Value =
        serde_json::from_str(&provider_capabilities(&options.config)).unwrap();
    assert_eq!(capabilities["mode"], "external-provider");
    assert_eq!(capabilities["training_supported"], false);
    assert_eq!(capabilities["persistence_supported"], false);
    assert_eq!(capabilities["hot_reload_supported"], false);
    assert_eq!(
        capabilities["merge_capabilities"]["safe_automation_hints"],
        json!([])
    );
    assert_eq!(
        capabilities["handoff_capabilities"]["readiness_levels"],
        json!(["advisory_only"])
    );
    assert!(!capabilities.to_string().contains("127.0.0.1"));
    let state = state();
    for path in [
        "/v1/train/latest",
        "/v1/train/targets/one",
        "/v1/memory-admin/clear",
        "/v1/latest/output.json",
    ] {
        let request = format!("POST {path} HTTP/1.1\r\n\r\n");
        assert!(
            route(
                &request,
                "127.0.0.1".parse().unwrap(),
                &DaemonAccessPolicy::default(),
                &state,
                &options
            )
            .unwrap()
            .starts_with("HTTP/1.1 405")
        );
    }
    for path in [
        "/v1/memory-state.json",
        "/v1/training-labels.json",
        "/v1/latest/targets",
        "/v1/latest/diagnostic-opinion.json",
    ] {
        let request = format!("GET {path} HTTP/1.1\r\n\r\n");
        assert!(
            route(
                &request,
                "127.0.0.1".parse().unwrap(),
                &DaemonAccessPolicy::default(),
                &state,
                &options
            )
            .unwrap()
            .starts_with("HTTP/1.1 404")
        );
    }
}

#[test]
fn authentication_cannot_be_supplied_in_the_body_or_bypassed_on_metadata() {
    let options = options();
    let state = state();
    let peer = "192.0.2.7".parse().unwrap();
    let policy = DaemonAccessPolicy {
        admin_token: Some("test-secret".into()),
    };
    for request in [
        "GET /health HTTP/1.1\r\n\r\nX-Etragon-Admin-Token: test-secret\r\n",
        "GET /v1/protocol-capabilities.json HTTP/1.1\r\n\r\n",
        "GET /v1/latest/meta HTTP/1.1\r\nX-Etragon-Admin-Token: wrong\r\n\r\n",
    ] {
        assert!(
            route(request, peer, &policy, &state, &options)
                .unwrap()
                .starts_with("HTTP/1.1 403")
        );
    }
    assert!(
        route(
            "GET /health HTTP/1.1\r\nX-Etragon-Admin-Token: test-secret\r\n\r\n",
            peer,
            &policy,
            &state,
            &options
        )
        .unwrap()
        .starts_with("HTTP/1.1 200")
    );
}

fn get(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn health_is_available_during_inference_and_stop_releases_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let worker = thread::spawn(move || {
        run_provider_daemon_until(
            listener,
            &options(),
            DaemonAccessPolicy::default(),
            worker_stop,
            || Ok(input()),
            |_| {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(output())
            },
        )
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(get(addr, "/health").starts_with("HTTP/1.1 200"));
    let result = get(addr, "/v1/latest/output.json");
    assert!(result.starts_with("HTTP/1.1 503"));
    assert!(result.contains("\"status\":\"analyzing\""));
    stop.store(true, Ordering::Relaxed);
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    assert!(TcpListener::bind(addr).is_ok());
}

#[test]
fn poll_failure_unwinds_listener_instead_of_leaving_an_orphan() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let result = std::panic::catch_unwind(|| {
        run_provider_daemon_until(
            listener,
            &options(),
            DaemonAccessPolicy::default(),
            Arc::clone(&stop),
            || panic!("injected host failure"),
            |_| Ok(output()),
        )
    });
    assert!(result.is_err());
    assert!(stop.load(Ordering::Relaxed));
    assert!(TcpListener::bind(addr).is_ok());
}

#[test]
fn provider_options_reject_legacy_state_and_ambiguous_flags_before_loading_config() {
    for args in [
        vec!["--provider", "missing", "--state", "memory.json"],
        vec!["--provider", "missing", "--daemon-state", "daemon.json"],
        vec!["--provider", "missing", "--filter", "one"],
        vec!["--provider", "missing", "--provider", "second"],
        vec!["--provider", "missing", "--interval-ms", "0"],
        vec!["--provider", "missing", "--bind", "localhost:1234"],
        vec!["--provider"],
    ] {
        let args = args.into_iter().map(str::to_string).collect::<Vec<_>>();
        assert!(parse_provider_daemon_options(&args).is_err());
    }
}
