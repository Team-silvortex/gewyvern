use std::env;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use gewyvern::transport_safety::open_bounded_regular_file;
use gewyvern::validation_harness::{
    run_linux_attach_smoke, run_linux_kprobe_smoke, run_linux_tc_smoke,
};

const CONFIG_PATH: &str = "/etc/gewyvern/ebpf-helper.conf";
const OUTPUT_ROOT: &str = "/var/lib/gewyvern-ebpf-validation";
const DEFAULT_HOOKPOINT: &str = "syscalls/sys_enter_nanosleep";
const DEFAULT_KPROBE: &str = "ip_route_output_flow";
const MAX_CONFIG_BYTES: u64 = 4096;
const HELPER_PROTOCOL_VERSION: u32 = 1;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("gewyvern eBPF helper: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    ensure_linux_root()?;
    let config = load_config(Path::new(CONFIG_PATH))?;
    let caller_uid = sudo_caller_uid()?;
    if caller_uid != config.allowed_uid {
        return Err("sudo caller is not authorized by helper configuration".to_string());
    }

    match args.as_slice() {
        [command] if command == "probe" => {
            println!("status=ready");
            println!("protocol={HELPER_PROTOCOL_VERSION}");
            println!("version={}", env!("CARGO_PKG_VERSION"));
            println!("allowed_uid={caller_uid}");
            Ok(())
        }
        [command, run_flag, run_id, device_flag, device]
            if command == "run" && run_flag == "--run-id" && device_flag == "--device" =>
        {
            run_smokes(run_id, device, caller_uid)
        }
        [command, run_flag, run_id]
            if command == "cleanup" && run_flag == "--run-id" =>
        {
            cleanup_run(run_id)
        }
        _ => Err(
            "usage: gewyvern_ebpf_helper probe | run --run-id ID --device IFACE | cleanup --run-id ID"
                .to_string(),
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HelperConfig {
    allowed_uid: u32,
}

fn load_config(path: &Path) -> Result<HelperConfig, String> {
    let mut file = open_bounded_regular_file(path, MAX_CONFIG_BYTES)
        .map_err(|error| format!("cannot securely open config '{}': {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect config '{}': {error}", path.display()))?;
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err("helper config must be root-owned and not group/world writable".to_string());
    }
    let mut input = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut input)
        .map_err(|error| format!("cannot read config '{}': {error}", path.display()))?;
    parse_config(&input)
}

fn parse_config(input: &str) -> Result<HelperConfig, String> {
    let mut allowed_uid = None;
    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| "helper config contains malformed line".to_string())?;
        if key != "allowed_uid" || allowed_uid.is_some() {
            return Err("helper config contains unknown or duplicate key".to_string());
        }
        let uid = value
            .parse::<u32>()
            .map_err(|_| "helper config allowed_uid must be an integer".to_string())?;
        if uid == 0 {
            return Err("helper config may not authorize root as the sudo caller".to_string());
        }
        allowed_uid = Some(uid);
    }
    Ok(HelperConfig {
        allowed_uid: allowed_uid
            .ok_or_else(|| "helper config is missing allowed_uid".to_string())?,
    })
}

fn ensure_linux_root() -> Result<(), String> {
    if env::consts::OS != "linux" {
        return Err("helper is supported only on Linux".to_string());
    }
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("cannot read process status: {error}"))?;
    let effective_uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|value| value.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u32>().ok());
    if effective_uid != Some(0) {
        return Err("helper must run as root through sudo".to_string());
    }
    Ok(())
}

fn sudo_caller_uid() -> Result<u32, String> {
    env::var("SUDO_UID")
        .map_err(|_| "SUDO_UID is required".to_string())?
        .parse::<u32>()
        .map_err(|_| "SUDO_UID is invalid".to_string())
}

fn run_smokes(run_id: &str, device: &str, _caller_uid: u32) -> Result<(), String> {
    validate_run_id(run_id)?;
    validate_device(device)?;
    let root = prepare_output_root()?;
    let run_dir = root.join(run_id);
    fs::create_dir(&run_dir).map_err(|error| {
        format!(
            "cannot create unique evidence directory '{}': {error}",
            run_dir.display()
        )
    })?;

    let result = (|| {
        run_linux_attach_smoke(DEFAULT_HOOKPOINT, Some(run_dir.join("linux-attach-smoke")))
            .map_err(|error| error.to_string())?;
        run_linux_kprobe_smoke(DEFAULT_KPROBE, Some(run_dir.join("linux-kprobe-smoke")))
            .map_err(|error| error.to_string())?;
        run_linux_tc_smoke(device, Some(run_dir.join("linux-tc-smoke")))
            .map_err(|error| error.to_string())?;
        make_evidence_read_only(&run_dir)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_dir_all(&run_dir);
        return Err(error);
    }
    println!("status=ok");
    println!("reason=all_smokes_passed_privileged_helper");
    println!("default_route_device={device}");
    Ok(())
}

fn cleanup_run(run_id: &str) -> Result<(), String> {
    validate_run_id(run_id)?;
    let root = inspect_output_root()?;
    let run_dir = root.join(run_id);
    let metadata = fs::symlink_metadata(&run_dir).map_err(|error| {
        format!(
            "cannot inspect evidence run '{}': {error}",
            run_dir.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != 0 {
        return Err("evidence run must be a root-owned non-symlink directory".to_string());
    }
    fs::remove_dir_all(&run_dir).map_err(|error| {
        format!(
            "cannot remove evidence run '{}': {error}",
            run_dir.display()
        )
    })
}

fn prepare_output_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(OUTPUT_ROOT);
    if !root.exists() {
        fs::create_dir_all(&root)
            .map_err(|error| format!("cannot create output root '{}': {error}", root.display()))?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("cannot secure output root '{}': {error}", root.display()))?;
    }
    inspect_output_root()
}

fn inspect_output_root() -> Result<PathBuf, String> {
    let root = PathBuf::from(OUTPUT_ROOT);
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| format!("cannot inspect output root '{}': {error}", root.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
    {
        return Err("output root must be a root-owned, non-writable real directory".to_string());
    }
    Ok(root)
}

fn make_evidence_read_only(path: &Path) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|error| {
        format!(
            "cannot read evidence directory '{}': {error}",
            path.display()
        )
    })? {
        let entry = entry.map_err(|error| format!("cannot read evidence entry: {error}"))?;
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("cannot inspect evidence entry: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("evidence output may not contain symlinks".to_string());
        }
        if metadata.is_dir() {
            make_evidence_read_only(&entry.path())?;
            fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("cannot secure evidence directory: {error}"))?;
        } else if metadata.is_file() {
            fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o644))
                .map_err(|error| format!("cannot secure evidence file: {error}"))?;
        } else {
            return Err("evidence output contains a non-regular entry".to_string());
        }
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("cannot secure evidence root: {error}"))
}

fn validate_run_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("run id must contain only 1-64 ASCII letters, digits, '-' or '_'".to_string());
    }
    Ok(())
}

fn validate_device(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 15
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err("device must be a valid bounded Linux interface name".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_unique_bounded_and_never_authorizes_root() {
        assert_eq!(
            parse_config("allowed_uid=1001\n").unwrap().allowed_uid,
            1001
        );
        assert!(parse_config("allowed_uid=0\n").is_err());
        assert!(parse_config("allowed_uid=1\nallowed_uid=2\n").is_err());
        assert!(parse_config("output_root=/tmp\n").is_err());
    }

    #[test]
    fn identifiers_reject_path_and_shell_syntax() {
        assert!(validate_run_id("run-123_ok").is_ok());
        assert!(validate_run_id("../escape").is_err());
        assert!(validate_run_id("run;id").is_err());
        assert!(validate_device("wlp3s0").is_ok());
        assert!(validate_device("eth0;sh").is_err());
        assert!(validate_device("../../dev").is_err());
    }

    #[test]
    fn helper_protocol_is_explicit_and_versioned() {
        assert_eq!(HELPER_PROTOCOL_VERSION, 1);
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn helper_config_loader_rejects_symlinks_before_trust_checks() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "gewyvern-ebpf-helper-config-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.conf");
        let link = root.join("helper.conf");
        fs::write(&target, "allowed_uid=1001\n").unwrap();
        symlink(&target, &link).unwrap();

        let error = load_config(&link).expect_err("helper config symlink must be rejected");

        assert!(error.contains("cannot securely open config"));
        fs::remove_dir_all(root).unwrap();
    }
}
