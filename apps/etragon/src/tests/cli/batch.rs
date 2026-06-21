use super::*;

#[test]
fn cli_analyzes_python_target_batch_once() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/scan:http:request/analysis.json",
        ] {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
            let body = match expected_path {
                "/v1/latest/targets" => {
                    "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
                }
                _ => {
                    "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let mut args = vec![
        "analyze-python-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--filter".to_string(),
        "scan:".to_string(),
    ];
    args.extend(default_worker_args());
    let output = run_cli(&args).expect("batch analyze should succeed");
    assert!(output.contains("\"path_segment\":\"scan:http:request\""));
    assert!(output.contains("\"py_ml_candidate_observe_longer\""));
    assert!(output.contains("\"producer_pass\":\"python_baseline_worker\""));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_trains_python_target_batch_once() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path =
        std::env::temp_dir().join(format!("etragon-python-target-train-state-{unique}.json"));
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/scan:http:request/analysis.json",
        ] {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
            let body = match expected_path {
                "/v1/latest/targets" => {
                    "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
                }
                _ => {
                    "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let mut args = vec![
        "train-python-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--label".to_string(),
        "http_request_followup".to_string(),
        "--filter".to_string(),
        "scan:".to_string(),
    ];
    args.extend(default_worker_args());
    args.push("--python-state".to_string());
    args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&args).expect("batch train should succeed");
    assert!(output.contains("\"path_segment\":\"scan:http:request\""));
    assert!(output.contains("\"status\":\"trained\""));
    assert!(output.contains("\"label\":\"http_request_followup\""));
    assert!(output.contains("\"train_count\":1"));
    assert!(output.contains("\"last_trained_unix_ms\":"));

    let _ = fs::remove_file(state_path);
    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_batch_training_then_analysis_emits_learned_route_candidate() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let state_path = std::env::temp_dir().join(format!(
        "etragon-python-target-train-analyze-state-{unique}.json"
    ));
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            let body = if request_text.starts_with("GET /v1/latest/targets ") {
                "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
            } else if request_text
                .starts_with("GET /v1/latest/targets/scan:http:request/analysis.json ")
            {
                "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
            } else {
                panic!("unexpected request path: {request_text}");
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let mut train_args = vec![
        "train-python-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--label".to_string(),
        "http_request_followup".to_string(),
        "--filter".to_string(),
        "scan:".to_string(),
    ];
    train_args.extend(default_worker_args());
    train_args.push("--python-state".to_string());
    train_args.push(state_path.to_string_lossy().to_string());
    let train_output = run_cli(&train_args).expect("batch train should succeed");
    assert!(train_output.contains("\"status\":\"trained\""));

    let mut analyze_args = vec![
        "analyze-python-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--filter".to_string(),
        "scan:".to_string(),
    ];
    analyze_args.extend(default_worker_args());
    analyze_args.push("--python-state".to_string());
    analyze_args.push(state_path.to_string_lossy().to_string());
    let output = run_cli(&analyze_args).expect("batch analyze should succeed");
    assert!(output.contains("\"name\":\"py_ml_candidate_learned_route\""));
    assert!(output.contains("\"learned_label\":\"http_request_followup\""));
    assert!(output.contains("\"train_count\":1"));
    assert!(output.contains("\"last_trained_unix_ms\":"));
    assert!(output.contains("\"score_margin\":"));
    assert!(output.contains("\"train_count\":1"));
    assert!(output.contains("\"last_trained_unix_ms\":"));

    let _ = fs::remove_file(state_path);
    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_analyzes_all_targets_from_target_index_url() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/socket_session/analysis.json",
            "/v1/latest/targets/scan:http:request/analysis.json",
        ] {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
            let body = match expected_path {
                "/v1/latest/targets" => {
                    "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
                }
                "/v1/latest/targets/socket_session/analysis.json" => {
                    "{\"primary_module_kind\":\"connection_establishment\",\"primary_failure_mode\":\"attention\",\"primary_failure_detail\":\"attention\",\"primary_failure_confidence\":\"low\",\"primary_failure_basis\":\"phase_inference\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
                _ => {
                    "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let output = run_cli(&[
        "analyze-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
    ])
    .expect("cli should analyze all targets from target index url");
    assert!(output.contains("\"targets\":["));
    assert!(output.contains("\"path_segment\":\"socket_session\""));
    assert!(output.contains("\"path_segment\":\"scan:http:request\""));
    assert!(output.contains("\"name\":\"ml_candidate_manual_review\""));
    assert!(output.contains("\"name\":\"ml_candidate_observe_longer\""));
    assert!(output.contains("\"recommendation_summary\":["));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_filters_target_batch_by_prefix() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/scan:http:request/analysis.json",
        ] {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
            let body = match expected_path {
                "/v1/latest/targets" => {
                    "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
                }
                _ => {
                    "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let output = run_cli(&[
        "analyze-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--filter".to_string(),
        "scan:".to_string(),
    ])
    .expect("cli should analyze filtered target batch");

    assert!(output.contains("\"path_segment\":\"scan:http:request\""));
    assert!(!output.contains("\"path_segment\":\"socket_session\""));
    assert!(output.contains("\"recommendation_summary\":"));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_merges_batch_recommendation_summary_counts() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/socket_session/analysis.json",
            "/v1/latest/targets/scan:http:request/analysis.json",
        ] {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 2048];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
            let body = match expected_path {
                "/v1/latest/targets" => {
                    "{\"kind\":\"scan\",\"target_count\":2,\"targets\":[\"socket_session\",\"scan:http:request\"],\"target_refs\":[{\"name\":\"socket_session\",\"path_segment\":\"socket_session\",\"url_path\":\"/v1/latest/targets/socket_session\"},{\"name\":\"scan:http:request\",\"path_segment\":\"scan:http:request\",\"url_path\":\"/v1/latest/targets/scan:http:request\"}],\"path_segment_encoding\":\"percent-encoding\",\"direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"}".to_string()
                }
                _ => {
                    "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}".to_string()
                }
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response should write");
        }
    });

    let output = run_cli(&[
        "analyze-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
    ])
    .expect("cli should analyze target batch");

    assert!(output.contains("\"recommendation_summary\":["));
    assert!(output.contains("\"name\":\"ml_candidate_observe_longer\""));
    assert!(output.contains("\"producer_stage\":\"candidate\""));
    assert!(output.contains("\"producer_pass\":\"MockMlAdvisoryEngine\""));
    assert!(output.contains("\"count\":2"));

    handle.join().expect("server thread should exit cleanly");
}
