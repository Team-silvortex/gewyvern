use super::*;

const FLAGS: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
pub(super) const LARGE: usize = 96 * 1024;
fn request(op: u8, key: &[u8], data: &[u8], id: u32) -> Vec<u8> {
    let extras = if matches!(op, 1..=3) { 8 } else { 0 };
    let mut h = vec![0u8; 24];
    h[0] = 0x80;
    h[1] = op;
    h[2..4].copy_from_slice(&(key.len() as u16).to_be_bytes());
    h[4] = extras;
    h[8..12]
        .copy_from_slice(&((usize::from(extras) + key.len() + data.len()) as u32).to_be_bytes());
    h[12..16].copy_from_slice(&id.to_be_bytes());
    if extras != 0 {
        h.extend(FLAGS);
        h.extend([0; 4]);
    }
    h.extend(key);
    h.extend(data);
    h
}
fn response(s: &mut TcpStream, op: u8, status: u16, id: u32) -> Vec<u8> {
    let mut h = [0; 24];
    s.read_exact(&mut h).unwrap();
    assert_eq!(&h[..4], &[0x81, op, 0, 0]);
    assert_eq!(h[5], 0);
    assert_eq!(u16::from_be_bytes([h[6], h[7]]), status);
    assert_eq!(&h[12..16], &id.to_be_bytes());
    let n = u32::from_be_bytes(h[8..12].try_into().unwrap()) as usize;
    assert!(n <= 2 * 1024 * 1024);
    let mut body = vec![0; n];
    s.read_exact(&mut body).unwrap();
    if status == 0 && matches!(op, 0 | 9) {
        assert_eq!(h[4], 4);
        assert_eq!(&body[..4], &FLAGS);
        return body.split_off(4);
    }
    assert_eq!(h[4], 0);
    if status == 0 && op != 11 {
        assert!(body.is_empty());
    }
    body
}
pub(super) fn noop(s: &mut TcpStream) {
    s.write_all(&request(10, b"", b"", 42)).unwrap();
    response(s, 10, 0, 42);
}
pub(super) fn traffic(s: &mut TcpStream, case: &str) {
    match case {
        "mixed" => {
            let mut decoy = [0u8; 28];
            decoy[..2].copy_from_slice(&[0x81, 0]);
            decoy[7] = 1;
            decoy[24..].copy_from_slice(b"\xff\0\r\n");
            let large: Vec<u8> = decoy.into_iter().cycle().take(LARGE).collect();
            let oversized = vec![0; 1024 * 1024 + 4096];
            let requests: &[(u8, &[u8], &[u8])] = &[
                (0, b"missing", b""),
                (1, b"empty", b""),
                (0, b"empty", b""),
                (1, b"large", &large),
                (2, b"large", b"x"),
                (3, b"missing", b"x"),
                (14, b"missing", b"x"),
                (1, b"huge", &oversized),
                (0, b"large", b""),
                (9, b"missing", b""),
                (9, b"empty", b""),
                (10, b"", b""),
                (15, b"empty", b"x"),
                (0, b"empty", b""),
                (4, b"empty", b""),
                (4, b"empty", b""),
                (0, b"empty", b""),
                (10, b"", b""),
            ];
            let mut bytes = Vec::new();
            for (i, (op, key, value)) in requests.iter().enumerate() {
                bytes.extend(request(*op, key, value, i as u32 + 1));
            }
            s.write_all(&bytes).unwrap();
            for (id, op, status) in [
                (1, 0, 1),
                (2, 1, 0),
                (3, 0, 0),
                (4, 1, 0),
                (5, 2, 2),
                (6, 3, 1),
                (7, 14, 5),
                (8, 1, 3),
                (9, 0, 0),
                (11, 9, 0),
                (12, 10, 0),
                (13, 15, 0),
                (14, 0, 0),
                (15, 4, 0),
                (16, 4, 1),
                (17, 0, 1),
                (18, 10, 0),
            ] {
                let body = response(s, op, status, id);
                match id {
                    3 | 11 => assert!(body.is_empty()),
                    9 => assert!(body == large),
                    14 => assert_eq!(body, b"x"),
                    _ => {}
                }
            }
        }
        "limit32" | "overflow33" => {
            let n = if case == "limit32" { 32 } else { 33 };
            let bytes: Vec<_> = (0..n).flat_map(|i| request(10, b"", b"", i)).collect();
            s.write_all(&bytes).unwrap();
            for i in 0..n {
                response(s, 10, 0, i);
            }
        }
        "version" => {
            s.write_all(&request(11, b"", b"", 1)).unwrap();
            let version = response(s, 11, 0, 1);
            assert!(version.starts_with(b"1.6."));
            fs::write(
                PathBuf::from(std::env::var_os("GEWY_MC_DIR").unwrap()).join("version"),
                version,
            )
            .unwrap();
        }
        "preexisting" => noop(s),
        _ => panic!("unknown case"),
    }
}
pub(super) fn expected_mixed() -> Vec<Value> {
    let event = |op, status, result| json!({"operation":op,"status":status,"result":result});
    let hit = |op, n| json!({"operation":op,"status":0,"result":"hit","value_bytes":n});
    vec![
        event("get", 1, "miss"),
        event("set", 0, "stored"),
        hit("get", 0),
        event("set", 0, "stored"),
        event("add", 2, "key_exists"),
        event("replace", 1, "not_found"),
        event("append", 5, "not_stored"),
        event("set", 3, "too_large"),
        hit("get", LARGE),
        hit("getq", 0),
        event("noop", 0, "ok"),
        event("prepend", 0, "stored"),
        hit("get", 1),
        event("delete", 0, "deleted"),
        event("delete", 1, "not_found"),
        event("get", 1, "miss"),
        event("noop", 0, "ok"),
    ]
}
