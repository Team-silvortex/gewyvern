use super::*;

#[test]
fn daemon_request_reader_rejects_oversized_bodies() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        match read_daemon_request(&mut stream).expect("request should read") {
            DaemonRequestRead::TooLarge => {}
            DaemonRequestRead::Invalid => {
                panic!("oversized request should not be treated as invalid")
            }
            DaemonRequestRead::Complete(_) => panic!("oversized request should be rejected"),
        }
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    let body = "x".repeat(DAEMON_REQUEST_LIMIT_BYTES + 1);
    let request = format!(
        "POST /v1/memory-admin/load HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    client
        .write_all(request.as_bytes())
        .expect("request should write");
    handle.join().expect("server thread should join");
}

#[test]
fn daemon_request_reader_collects_full_declared_body() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        match read_daemon_request(&mut stream).expect("request should read") {
            DaemonRequestRead::Complete(request) => {
                assert!(request.contains("\"slot\":\"baseline\""));
                assert!(request.ends_with('}'));
            }
            DaemonRequestRead::Invalid => panic!("small request should not be treated as invalid"),
            DaemonRequestRead::TooLarge => panic!("small request should be accepted"),
        }
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    let body = "{\"slot\":\"baseline\",\"strategy\":\"merge\"}";
    let header = format!(
        "POST /v1/memory-admin/load HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    client
        .write_all(header.as_bytes())
        .expect("header should write");
    client
        .write_all(body.as_bytes())
        .expect("body should write");
    handle.join().expect("server thread should join");
}

#[test]
fn daemon_request_reader_waits_for_delayed_bytes_on_nonblocking_accepted_stream() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let (accepted_tx, accepted_rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        stream
            .set_nonblocking(true)
            .expect("accepted stream should become nonblocking");
        accepted_tx
            .send(())
            .expect("accepted stream signal should send");
        match read_daemon_request(&mut stream).expect("request should read") {
            DaemonRequestRead::Complete(request) => {
                assert!(request.starts_with("GET /health HTTP/1.1"));
            }
            DaemonRequestRead::Invalid => {
                panic!("delayed valid request should not be treated as invalid")
            }
            DaemonRequestRead::TooLarge => panic!("small request should be accepted"),
        }
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    accepted_rx
        .recv()
        .expect("accepted stream signal should arrive");
    thread::sleep(Duration::from_millis(25));
    client
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .expect("request should write");
    handle.join().expect("server thread should join");
}

#[test]
fn daemon_handler_returns_413_for_oversized_request() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let handle = thread::spawn(move || {
        let (stream, remote_addr) = listener.accept().expect("client should connect");
        let latest = Arc::new(Mutex::new(None));
        let invalidation = Arc::new(AtomicU64::new(0));
        handle_daemon_client(
            stream,
            remote_addr.ip(),
            &DaemonAccessPolicy::default(),
            &latest,
            &PythonWorkerConfig::default(),
            None,
            &invalidation,
        )
        .expect("handler should return cleanly");
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    let body = "x".repeat(DAEMON_REQUEST_LIMIT_BYTES + 1);
    let request = format!(
        "POST /v1/memory-admin/load HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    client
        .write_all(request.as_bytes())
        .expect("request should write");
    let mut response = String::new();
    client
        .read_to_string(&mut response)
        .expect("response should read");
    assert!(response.contains("413 Payload Too Large"));
    assert!(response.contains("daemon_request_too_large"));
    handle.join().expect("server thread should join");
}

#[test]
fn daemon_request_reader_rejects_duplicate_content_length_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        match read_daemon_request(&mut stream).expect("request should read") {
            DaemonRequestRead::Invalid => {}
            DaemonRequestRead::TooLarge => {
                panic!("duplicate content-length request should not be treated as oversized")
            }
            DaemonRequestRead::Complete(_) => {
                panic!("duplicate content-length request should be rejected")
            }
        }
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    let request = "POST /v1/memory-admin/load HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nContent-Length: 7\r\n\r\n{}".to_string();
    client
        .write_all(request.as_bytes())
        .expect("request should write");
    handle.join().expect("server thread should join");
}

#[test]
fn daemon_handler_returns_400_for_invalid_request_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let handle = thread::spawn(move || {
        let (stream, remote_addr) = listener.accept().expect("client should connect");
        let latest = Arc::new(Mutex::new(None));
        let invalidation = Arc::new(AtomicU64::new(0));
        handle_daemon_client(
            stream,
            remote_addr.ip(),
            &DaemonAccessPolicy::default(),
            &latest,
            &PythonWorkerConfig::default(),
            None,
            &invalidation,
        )
        .expect("handler should return cleanly");
    });

    let mut client = TcpStream::connect(addr).expect("client should connect");
    let request = "POST /v1/memory-admin/load HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nContent-Length: 7\r\n\r\n{}".to_string();
    client
        .write_all(request.as_bytes())
        .expect("request should write");
    let mut response = String::new();
    client
        .read_to_string(&mut response)
        .expect("response should read");
    assert!(response.contains("400 Bad Request"));
    assert!(response.contains("daemon_request_invalid"));
    handle.join().expect("server thread should join");
}
