use super::*;

fn options() -> TargetOptions {
    TargetOptions {
        base: ProviderDaemonOptions {
            bind:"127.0.0.1:0".parse().unwrap(), interval_ms:100,
            config:ProviderConfig::from_json(r#"{"id":"test","model":"fixture","endpoint":"http://127.0.0.1:1/v1/chat/completions"}"#).unwrap(), source_ca:None,
        }, filter:None, max_targets:24, max_inferences:1,
    }
}

fn shared() -> SharedIndex {
    Arc::new(Mutex::new(IndexState {
        status: "starting",
        ..Default::default()
    }))
}
fn index(names: &[&str]) -> String {
    json!({"target_refs":names.iter().map(|name| json!({"path_segment":name})).collect::<Vec<_>>()})
        .to_string()
}
fn input(name: &str) -> String {
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/missing_transition_analysis.json"
    ))
    .unwrap();
    value["primary_failure_detail"] = json!(name);
    value.to_string()
}
fn output() -> etragon::EngineOutput {
    etragon::EngineOutput::with_augmentation(etragon::AnalysisAugmentation::new(
        "external-advisory",
        "candidate",
        "review only",
        "candidate",
        Some("{}".into()),
    ))
}
fn due(poller: &mut TargetPoller) {
    for worker in poller.workers.values_mut() {
        worker.due = Instant::now();
    }
}
fn body(state: &SharedIndex, options: &TargetOptions) -> Value {
    index_view(&state.lock().unwrap(), options, true).unwrap()
}
fn request(path: &str, state: &SharedIndex, options: &TargetOptions) -> String {
    route(
        &format!("GET {path} HTTP/1.1\r\n\r\n"),
        "127.0.0.1".parse().unwrap(),
        &DaemonAccessPolicy::default(),
        state,
        options,
    )
    .unwrap()
}

#[test]
fn one_call_budget_is_fair_even_when_all_evidence_keeps_changing() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    let mut calls = Vec::new();
    for round in 0..6 {
        due(&mut poller);
        poller
            .poll(
                &state,
                &options,
                &AtomicBool::new(false),
                || Ok(index(&["c", "a", "b"])),
                |name| Ok(input(&format!("{name}-{round}"))),
                |input| {
                    calls.push(
                        serde_json::from_str::<Value>(input).unwrap()["primary_failure_detail"]
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    );
                    Ok(output())
                },
            )
            .unwrap();
        assert_eq!(calls.len(), round + 1);
        assert_eq!(body(&state, &options)["ready_count"], 1);
        assert_eq!(body(&state, &options)["analysis_runs"], round + 1);
        assert_eq!(body(&state, &options)["last_cycle_inferences"], 1);
    }
    assert_eq!(calls, ["a-0", "b-1", "c-2", "a-3", "b-4", "c-5"]);
}

#[test]
fn identical_evidence_is_not_shared_between_target_identities() {
    let state = shared();
    let mut options = options();
    options.max_inferences = 2;
    let mut poller = TargetPoller::default();
    let mut calls = 0;
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b"])),
            |_| Ok(input("same")),
            |_| {
                calls += 1;
                Ok(output())
            },
        )
        .unwrap();
    assert_eq!(calls, 2);
    let result = body(&state, &options);
    for (i, name) in ["a", "b"].iter().enumerate() {
        assert_eq!(
            result["targets"][i]["output"]["augmentations"][0]["data"]["target_path_segment"],
            *name
        );
    }
    due(&mut poller);
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["b", "a"])),
            |_| Ok(input("same")),
            |_| panic!("each target should use its own cache"),
        )
        .unwrap();
    assert_eq!(body(&state, &options)["targets"][0]["cache_hits"], 1);
    assert_eq!(body(&state, &options)["targets"][1]["cache_hits"], 1);
}

#[test]
fn one_failed_target_backs_off_without_hiding_healthy_advice() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b"])),
            |name| Ok(input(name)),
            |_| Err(ProviderError::HttpStatus(429)),
        )
        .unwrap();
    poller.workers.get_mut("b").unwrap().due = Instant::now();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b"])),
            |name| {
                assert_eq!(name, "b");
                Ok(input(name))
            },
            |_| Ok(output()),
        )
        .unwrap();
    assert!(
        request("/v1/latest/targets/a/output.json", &state, &options).starts_with("HTTP/1.1 503")
    );
    assert!(
        request("/v1/latest/targets/b/output.json", &state, &options).starts_with("HTTP/1.1 200")
    );
    let result = body(&state, &options);
    assert_eq!(result["status"], "partial");
    assert!(result["targets"][0]["output"].is_null());
    assert_eq!(
        result["targets"][0]["last_error"],
        "etragon_provider_http_status: 429"
    );
    assert_eq!(result["ready_count"], 1);
}

#[test]
fn removal_and_reappearance_make_a_new_cold_incarnation() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a"])),
            |_| Ok(input("same")),
            |_| Ok(output()),
        )
        .unwrap();
    let old = body(&state, &options)["targets"][0]["target_incarnation"].clone();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&[])),
            |_| panic!(),
            |_| panic!(),
        )
        .unwrap();
    assert_eq!(body(&state, &options)["status"], "ready");
    assert_eq!(body(&state, &options)["target_count"], 0);
    assert!(poller.workers.is_empty());
    assert!(
        request("/v1/latest/targets/a/output.json", &state, &options).starts_with("HTTP/1.1 404")
    );
    let mut called = false;
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a"])),
            |_| Ok(input("same")),
            |_| {
                called = true;
                Ok(output())
            },
        )
        .unwrap();
    assert!(called);
    assert_ne!(
        body(&state, &options)["targets"][0]["target_incarnation"],
        old
    );
    assert_eq!(body(&state, &options)["analysis_runs"], 2);
}

#[test]
fn invalid_index_revokes_visible_results_and_recovery_starts_cold() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a"])),
            |_| Ok(input("same")),
            |_| Ok(output()),
        )
        .unwrap();
    let delay = poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "a"])),
            |_| panic!("invalid index must block target reads"),
            |_| panic!(),
        )
        .unwrap();
    assert_eq!(delay, 1000);
    assert!(poller.workers.is_empty());
    assert_eq!(body(&state, &options)["index_status"], "error");
    assert!(body(&state, &options)["targets"][0]["output"].is_null());
    let mut called = false;
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a"])),
            |_| Ok(input("same")),
            |_| {
                called = true;
                Ok(output())
            },
        )
        .unwrap();
    assert!(called);
    assert_eq!(body(&state, &options)["status"], "ready");
}

#[test]
fn source_failure_is_local_and_selected_limit_is_not_silently_truncated() {
    let state = shared();
    let mut options = options();
    options.max_inferences = 2;
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b"])),
            |name| {
                if name == "a" {
                    Err(ProviderError::Transport)
                } else {
                    Ok(input(name))
                }
            },
            |_| Ok(output()),
        )
        .unwrap();
    assert_eq!(body(&state, &options)["ready_count"], 1);
    options.max_targets = 1;
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b"])),
            |_| panic!(),
            |_| panic!(),
        )
        .unwrap();
    assert_eq!(body(&state, &options)["index_status"], "error");
    assert!(body(&state, &options)["targets"][1]["output"].is_null());
}

#[test]
fn unvisited_ready_advice_expires_without_waiting_for_scheduler() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a"])),
            |_| Ok(input("a")),
            |_| Ok(output()),
        )
        .unwrap();
    poller.workers["a"]
        .record
        .state
        .lock()
        .unwrap()
        .source_checked_at = Some(Instant::now() - Duration::from_secs(61));
    let result = body(&state, &options);
    assert_eq!(result["targets"][0]["status"], "stale");
    assert!(result["targets"][0]["output"].is_null());
    assert_eq!(result["ready_count"], 0);
    assert!(
        request("/v1/latest/targets/a/output.json", &state, &options).starts_with("HTTP/1.1 503")
    );
}

#[test]
fn target_visits_are_bounded_even_when_inference_budget_is_exhausted() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    let names = (0..12).map(|n| format!("t{n:02}")).collect::<Vec<_>>();
    let refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let mut visits = 0;
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&refs)),
            |_| {
                visits += 1;
                Ok(input("a"))
            },
            |_| Ok(output()),
        )
        .unwrap();
    assert_eq!(visits, MAX_VISITS_PER_CYCLE);
    let result = body(&state, &options);
    assert_eq!(result["target_count"], 12);
    assert_eq!(result["ready_count"], 1);
}

#[test]
fn canonical_target_routes_and_auth_remain_read_only() {
    let state = shared();
    let options = options();
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["scan:?%2F# space"])),
            |_| Ok(input("a")),
            |_| Ok(output()),
        )
        .unwrap();
    assert!(
        request(
            "/v1/latest/targets/scan:%3F%252F%23%20space/output.json",
            &state,
            &options
        )
        .starts_with("HTTP/1.1 200")
    );
    assert!(
        request(
            "/v1/latest/targets/scan:%3F%2F%23%20space/output.json",
            &state,
            &options
        )
        .starts_with("HTTP/1.1 404")
    );
    let policy = DaemonAccessPolicy {
        admin_token: Some("secret".into()),
    };
    let reply = route(
        "GET /v1/latest/targets HTTP/1.1\r\n\r\nX-Etragon-Admin-Token: secret\r\n",
        "192.0.2.1".parse().unwrap(),
        &policy,
        &state,
        &options,
    )
    .unwrap();
    assert!(reply.starts_with("HTTP/1.1 403"));
    let reply = route(
        "POST /v1/train/latest HTTP/1.1\r\n\r\n",
        "127.0.0.1".parse().unwrap(),
        &policy,
        &state,
        &options,
    )
    .unwrap();
    assert!(reply.starts_with("HTTP/1.1 405"));
    assert_eq!(capabilities(&options)["multi_target_supported"], true);
    assert_eq!(
        capabilities(&options)["limits"]["max_inferences_per_cycle"],
        1
    );
}

#[test]
fn aggregate_output_has_a_cap_and_target_routes_remain_available() {
    let state = shared();
    let mut options = options();
    options.max_inferences = 8;
    let mut poller = TargetPoller::default();
    poller
        .poll(
            &state,
            &options,
            &AtomicBool::new(false),
            || Ok(index(&["a", "b", "c", "d", "e", "f", "g", "h"])),
            |_| Ok(input("same")),
            |_| Ok(output()),
        )
        .unwrap();
    for worker in poller.workers.values() {
        worker.record.state.lock().unwrap().output = Some(json!({"large": "x".repeat(140_000)}));
    }
    assert!(request("/v1/latest/output.json", &state, &options).starts_with("HTTP/1.1 413"));
    assert!(
        request("/v1/latest/targets/a/output.json", &state, &options).starts_with("HTTP/1.1 200")
    );
}

#[test]
fn unsafe_or_duplicate_budget_options_fail_before_opening_config() {
    for extra in [
        vec!["--max-targets", "0"],
        vec!["--max-targets", "65"],
        vec!["--max-inferences-per-cycle", "9"],
        vec!["--filter", ""],
        vec!["--filter", "a", "--filter", "b"],
        vec!["--daemon-state", "old.json"],
    ] {
        let mut args = vec!["--provider".to_owned(), "missing".to_owned()];
        args.extend(extra.into_iter().map(str::to_owned));
        assert!(parse_options(&args).is_err());
    }
}
