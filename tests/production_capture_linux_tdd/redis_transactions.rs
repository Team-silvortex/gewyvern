use super::*;

pub(super) fn exchange(sock: &mut TcpStream, request: &[u8], expected: &[u8]) {
    sock.write_all(request).unwrap();
    let mut got = vec![0; expected.len()];
    sock.read_exact(&mut got).unwrap();
    assert!(got == expected, "Redis wire replies differ");
}

pub(super) fn mixed(req: &mut Vec<u8>, replies: &mut Vec<u8>) {
    let large: Vec<_> = BINARY.iter().copied().cycle().take(LARGE).collect();
    // Redis constructs the nested error and bulk frames itself, not a scripted TCP peer.
    let nested = b"return {redis.error_reply('ERR inner'),ARGV[1]}";
    let commands: &[&[&[u8]]] = &[
        &[b"SET", b"tx-key", DECOY],
        &[b"MULTI"],
        &[b"MGET", b"missing", b"tx-key"],
        &[b"EVAL", nested, b"0", &large],
        &[b"LPUSH", b"tx-key", b"x"],
        &[b"ECHO", DECOY],
        &[b"LRANGE", b"missing", b"0", b"-1"],
        &[b"EXEC"],
        &[b"MGET", b"tx-key"],
        &[b"PING"],
    ];
    for args in commands {
        req.extend(command(args));
    }
    replies.extend(b"+OK\r\n+OK\r\n");
    replies.extend(b"+QUEUED\r\n".repeat(5));
    replies.extend(b"*5\r\n*2\r\n$-1\r\n");
    replies.extend(bulk(DECOY));
    replies.extend(b"*2\r\n-ERR inner\r\n");
    replies.extend(bulk(&large));
    replies.extend(b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n");
    replies.extend(bulk(DECOY));
    replies.extend(b"*0\r\n*1\r\n");
    replies.extend(bulk(DECOY));
    replies.extend(b"+PONG\r\n");
}

pub(super) fn watch(sock: &mut TcpStream, dir: &Path, req: &mut Vec<u8>, replies: &mut Vec<u8>) {
    req.extend(command(&[b"WATCH", b"watched"]));
    replies.extend(b"+OK\r\n");
    exchange(sock, req, replies);
    fs::write(dir.join("watch-active"), []).unwrap();
    until(|| dir.join("watch-invalidated").is_file());
    req.clear();
    replies.clear();
    let cases: &[(&[&[u8]], &[u8])] = &[
        (&[b"MULTI"], b"+OK\r\n"),
        (&[b"PING"], b"+QUEUED\r\n"),
        (&[b"EXEC"], b"*-1\r\n"),
        (&[b"MULTI"], b"+OK\r\n"),
        (&[b"EXEC"], b"*0\r\n"),
        (&[b"MULTI"], b"+OK\r\n"),
        (
            &[b"ECHO"],
            b"-ERR wrong number of arguments for 'echo' command\r\n",
        ),
        (
            &[b"EXEC"],
            b"-EXECABORT Transaction discarded because of previous errors.\r\n",
        ),
        (&[b"PING"], b"+PONG\r\n"),
    ];
    for (args, reply) in cases {
        req.extend(command(args));
        replies.extend(*reply);
    }
}

pub(super) fn invalidate_watch(sock: &mut TcpStream, token: &str) {
    let mut req = command(&[b"AUTH", token.as_bytes()]);
    req.extend(command(&[b"SET", b"watched", b"changed"]));
    exchange(sock, &req, b"+OK\r\n+OK\r\n");
}

pub(super) fn too_deep(req: &mut Vec<u8>, replies: &mut Vec<u8>) {
    req.extend(command(&[b"EVAL", b"return {{{ARGV[1]}}}", b"0", DECOY]));
    req.extend(command(&[b"PING"]));
    replies.extend(b"*1\r\n*1\r\n*1\r\n");
    replies.extend(bulk(DECOY));
    replies.extend(b"+PONG\r\n");
}

pub(super) fn check(case: &str, row: &Value) {
    let simple = json!({"type":"simple_string"});
    let err = json!({"type":"error"});
    assert_eq!(row["outcome"], "framed");
    let expected = if case == "transaction" {
        assert!(row["server_payload_segments"].as_u64().unwrap() > 1);
        json!([simple,simple,simple,simple,simple,simple,simple,simple,
            {"type":"array","elements":5,"error_frames":2},
            {"type":"array","elements":1,"error_frames":0},simple])
    } else {
        json!([simple,simple,simple,simple,{"type":"null_array"},simple,
            {"type":"array","elements":0,"error_frames":0},simple,err,err,simple])
    };
    assert_eq!(row["replies"], expected);
}
