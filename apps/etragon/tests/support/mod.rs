#![allow(dead_code)]
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread::{self, JoinHandle};

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn read_fixture(name: &str) -> String {
    fs::read_to_string(fixture_path(name)).expect("fixture should read")
}

pub fn spawn_http_server(routes: Vec<(String, String)>) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    let handle = thread::spawn(move || {
        for (expected_path, body) in routes {
            let (mut stream, _) = listener.accept().expect("client should connect");
            let mut request = [0u8; 4096];
            let size = stream.read(&mut request).expect("request should read");
            let request_text = String::from_utf8_lossy(&request[..size]);
            assert!(
                request_text.starts_with(&format!("GET {} ", expected_path)),
                "unexpected request path: {request_text}"
            );
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
    (format!("127.0.0.1:{}", addr.port()), handle)
}
