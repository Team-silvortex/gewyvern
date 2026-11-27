use super::*;
fn docker(args: &[&str], env: &[(&str, &str)]) -> String {
    let out = Command::new("/usr/bin/docker")
        .args(args)
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "docker: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("/usr/bin/docker")
            .args(["rm", "-f", "-v", &self.0])
            .output();
    }
}
pub(super) fn run(
    dir: &Path,
    options: &[&str],
    image_args: &[&str],
    env: &[(&str, &str)],
    net_var: &str,
    test: &str,
) {
    let mut create = vec![
        "create",
        "--pull=never",
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
    ];
    create.extend_from_slice(options);
    create.extend_from_slice(image_args);
    let id = docker(&create, env);
    assert!(id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()));
    let owned = Container(id);
    docker(&["start", &owned.0], &[]);
    let pid = docker(&["inspect", "--format", "{{.State.Pid}}", &owned.0], &[])
        .parse::<u32>()
        .unwrap();
    assert!(pid > 1);
    let net = fs::read_link(format!("/proc/{pid}/ns/net")).unwrap();
    assert_ne!(net, fs::read_link("/proc/self/ns/net").unwrap());
    fs::write(dir.join("container"), &owned.0).unwrap();
    let mut child = Process(
        Command::new("/usr/bin/nsenter")
            .args(["--target", &pid.to_string(), "--net", "--"])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", test, "--ignored", "--nocapture"])
            .env(net_var, &net)
            .envs(env.iter().copied())
            .spawn()
            .unwrap(),
    );
    assert!(wait(&mut child, 150).success());
    docker(&["stop", "--time", "10", &owned.0], &[]);
    drop(owned);
}
