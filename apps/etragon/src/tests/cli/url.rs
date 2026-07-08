use super::*;

#[test]
fn cli_analyzes_snapshot_url() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request).expect("request should read");
        let body = "{\"primary_module_kind\":\"authentication_exchange\",\"primary_failure_mode\":\"server_denied\",\"primary_failure_detail\":\"access_denied\",\"primary_failure_confidence\":\"high\",\"primary_failure_basis\":\"direct_protocol_signal\",\"ambiguous\":false,\"competing_hypotheses\":[]}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("response should write");
    });

    let output = run_cli(&[
        "analyze-url".to_string(),
        format!("http://{}/v1/latest/analysis.json", addr),
    ])
    .expect("cli should analyze snapshot url");
    assert!(output.contains("\"augmentations\":["));
    assert!(output.contains("\"name\":\"ml_candidate_targeted_escalation\""));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_analyzes_target_specific_snapshot_url() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        let mut request = [0u8; 1024];
        let size = stream.read(&mut request).expect("request should read");
        let request_text = String::from_utf8_lossy(&request[..size]);
        assert!(
            request_text.starts_with("GET /v1/latest/targets/scan:http:request/analysis.json "),
            "request should target a concrete target-specific analysis route: {request_text}"
        );
        let body = "{\"primary_module_kind\":\"http_request_response\",\"primary_failure_mode\":\"no_response\",\"primary_failure_detail\":\"request_sent_no_reply\",\"primary_failure_confidence\":\"medium\",\"primary_failure_basis\":\"missing_transition\",\"ambiguous\":false,\"competing_hypotheses\":[]}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("response should write");
    });

    let output = run_cli(&[
        "analyze-url".to_string(),
        format!(
            "http://{}/v1/latest/targets/scan:http:request/analysis.json",
            addr
        ),
    ])
    .expect("cli should analyze target-specific snapshot url");
    assert!(output.contains("\"augmentations\":["));
    assert!(output.contains("\"name\":\"ml_candidate_observe_longer\""));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_watch_python_url_runs_single_cycle() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request).expect("request should read");
        let body = "{\"primary_module_kind\":\"authentication_exchange\",\"primary_failure_mode\":\"server_denied\",\"primary_failure_detail\":\"access_denied\",\"primary_failure_confidence\":\"high\",\"primary_failure_basis\":\"direct_protocol_signal\",\"ambiguous\":false,\"competing_hypotheses\":[]}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("response should write");
    });

    let mut args = vec![
        "watch-python-url".to_string(),
        format!("http://{}/v1/latest/analysis.json", addr),
        "--cycles".to_string(),
        "1".to_string(),
        "--interval-ms".to_string(),
        "1".to_string(),
    ];
    args.extend(default_worker_args());
    let output = run_cli(&args).expect("watch should succeed");
    assert!(output.contains("\"cycle\":1"));
    assert!(output.contains("\"source\":\"python-url\""));
    assert!(output.contains("\"name\":\"py_ml_candidate_targeted_escalation\""));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_watch_python_targets_url_runs_single_cycle() {
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
        "watch-python-targets-url".to_string(),
        format!("http://{}/v1/latest/targets", addr),
        "--filter".to_string(),
        "scan:".to_string(),
        "--cycles".to_string(),
        "1".to_string(),
        "--interval-ms".to_string(),
        "1".to_string(),
    ];
    args.extend(default_worker_args());
    let output = run_cli(&args).expect("watch should succeed");
    assert!(output.contains("\"cycle\":1"));
    assert!(output.contains("\"source\":\"python-targets-url\""));
    assert!(output.contains("\"path_segment\":\"scan:http:request\""));
    assert!(output.contains("\"py_ml_candidate_observe_longer\""));

    handle.join().expect("server thread should exit cleanly");
}

#[test]
fn cli_analyze_targets_url_percent_encodes_target_path_segments() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for expected_path in [
            "/v1/latest/targets",
            "/v1/latest/targets/scan%20target%3Fx%3D1/analysis.json",
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
                    "{\"kind\":\"scan\",\"target_count\":1,\"targets\":[\"scan target?x=1\"],\"target_refs\":[{\"name\":\"scan target?x=1\",\"path_segment\":\"scan target?x=1\",\"url_path\":\"/v1/latest/targets/scan%20target%3Fx%3D1\"}],\"path_segment_encoding\":\"percent-encoding\"}".to_string()
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
    .expect("cli should analyze targets url");
    assert!(output.contains("\"path_segment\":\"scan target?x=1\""));

    handle.join().expect("server thread should exit cleanly");
}
