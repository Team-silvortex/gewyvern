//! Controlled peers, not real-server evidence. An acknowledgement after each
//! read forces the selected stream boundaries independently of TCP timing.
use super::*;

fn chunks(case: &str) -> Vec<Vec<u8>> {
    let mut noop = vec![0; 24];
    noop[..2].copy_from_slice(&[0x81, 10]);
    let mut get = vec![0; 24];
    get[0] = 0x81;
    get[4] = 4;
    get[11] = 28;
    get.extend([0; 4]);
    get.extend_from_slice(&noop);
    match case {
        "control-split" => vec![
            noop.clone(),
            get[..1].to_vec(),
            get[1..10].to_vec(),
            get[10..22].to_vec(),
            get[22..31].to_vec(),
            [&get[31..], &noop].concat(),
        ],
        "control-header" => vec![noop, get[..19].to_vec()],
        "control-body" => vec![noop, get[..get.len() - 1].to_vec()],
        "control-flags" => {
            get[4] = 0;
            vec![noop, get]
        }
        _ => panic!("unknown control"),
    }
}
pub(super) fn receive(s: &mut TcpStream, dir: &Path, case: &str) {
    for (i, expected) in chunks(case).iter().enumerate() {
        let mut buf = vec![0; expected.len()];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, expected);
        fs::write(dir.join(format!("ack-{i}")), []).unwrap();
    }
    assert_eq!(s.read(&mut [0]).unwrap(), 0);
}
pub(super) fn send(listener: TcpListener, dir: &Path, case: &str) {
    listener.set_nonblocking(true).unwrap();
    let mut conn = None;
    until(|| {
        conn = listener.accept().ok();
        conn.is_some()
    });
    let mut s = conn.unwrap().0;
    s.set_nodelay(true).unwrap();
    for (i, chunk) in chunks(case).iter().enumerate() {
        s.write_all(chunk).unwrap();
        until(|| dir.join(format!("ack-{i}")).is_file());
    }
    s.shutdown(std::net::Shutdown::Write).unwrap();
}
