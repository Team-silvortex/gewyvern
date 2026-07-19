use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

const HELPER_PATH: &str = "/usr/libexec/gewyvern-ebpf-helper";
const CONFIG_DIR: &str = "/etc/gewyvern";
const CONFIG_PATH: &str = "/etc/gewyvern/ebpf-helper.conf";
const SUDOERS_DIR: &str = "/etc/sudoers.d";
const SUDOERS_PATH: &str = "/etc/sudoers.d/gewyvern-ebpf-validation";
const MAX_ACCOUNT_RECORD_BYTES: usize = 4096;

fn main() {
    if let Err(error) = run(env::args().skip(1).collect()) {
        eprintln!("gewyvern eBPF provisioner: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let options = parse_args(&args)?;
    let uid = resolve_user_uid(&options.user)?;
    inspect_helper(Path::new(HELPER_PATH))?;
    let config = render_config(uid);
    let sudoers = render_sudoers(&options.user);

    if options.dry_run {
        println!("user={}", options.user);
        println!("uid={uid}");
        println!("config_path={CONFIG_PATH}");
        println!("sudoers_path={SUDOERS_PATH}");
        println!("status=dry-run-ok");
        return Ok(());
    }

    ensure_linux_root()?;
    ensure_secure_directory(Path::new(CONFIG_DIR), true)?;
    ensure_secure_directory(Path::new(SUDOERS_DIR), false)?;
    reject_unsafe_existing(Path::new(CONFIG_PATH))?;
    reject_unsafe_existing(Path::new(SUDOERS_PATH))?;

    let sudoers_temp = write_temp(Path::new(SUDOERS_PATH), sudoers.as_bytes(), 0o440)?;
    if let Err(error) = validate_sudoers(&sudoers_temp) {
        let _ = fs::remove_file(&sudoers_temp);
        return Err(error);
    }
    let config_temp = match write_temp(Path::new(CONFIG_PATH), config.as_bytes(), 0o644) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_file(&sudoers_temp);
            return Err(error);
        }
    };

    if let Err(error) = install_temp(&config_temp, Path::new(CONFIG_PATH)) {
        let _ = fs::remove_file(&config_temp);
        let _ = fs::remove_file(&sudoers_temp);
        return Err(error);
    }
    if let Err(error) = install_temp(&sudoers_temp, Path::new(SUDOERS_PATH)) {
        let _ = fs::remove_file(&sudoers_temp);
        return Err(error);
    }

    println!("user={}", options.user);
    println!("uid={uid}");
    println!("status=installed");
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    user: String,
    dry_run: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut user = None;
    let mut dry_run = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--user" if user.is_none() => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| "--user requires an account name".to_string())?;
                validate_user_name(value)?;
                user = Some(value.clone());
            }
            "--dry-run" if !dry_run => dry_run = true,
            _ => {
                return Err("usage: gewyvern-ebpf-provision --user ACCOUNT [--dry-run]".to_string());
            }
        }
        index += 1;
    }
    Ok(Options {
        user: user.ok_or_else(|| "--user is required".to_string())?,
        dry_run,
    })
}

fn validate_user_name(value: &str) -> Result<(), String> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return Err("account name must not be empty".to_string());
    };
    if value.len() > 32
        || !(first.is_ascii_lowercase() || first == b'_')
        || !bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err("account name must be a bounded portable Linux name".to_string());
    }
    Ok(())
}

fn resolve_user_uid(user: &str) -> Result<u32, String> {
    let output = Command::new("getent")
        .args(["passwd", user])
        .output()
        .map_err(|error| format!("cannot execute getent: {error}"))?;
    if !output.status.success() {
        return Err(format!("account '{user}' does not exist"));
    }
    if output.stdout.len() > MAX_ACCOUNT_RECORD_BYTES {
        return Err("account record exceeds size limit".to_string());
    }
    let record = std::str::from_utf8(&output.stdout)
        .map_err(|_| "account record is not UTF-8".to_string())?;
    parse_passwd_record(record, user)
}

fn parse_passwd_record(record: &str, expected_user: &str) -> Result<u32, String> {
    let mut lines = record.lines().filter(|line| !line.is_empty());
    let line = lines
        .next()
        .ok_or_else(|| "account lookup returned no record".to_string())?;
    if lines.next().is_some() {
        return Err("account lookup returned multiple records".to_string());
    }
    let fields = line.split(':').collect::<Vec<_>>();
    if fields.len() != 7 || fields[0] != expected_user {
        return Err("account lookup returned a malformed or mismatched record".to_string());
    }
    let uid = fields[2]
        .parse::<u32>()
        .map_err(|_| "account UID is invalid".to_string())?;
    if uid == 0 {
        return Err("root may not be configured as the validation account".to_string());
    }
    Ok(uid)
}

fn inspect_helper(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect helper '{}': {error}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.mode() & 0o111 == 0
    {
        return Err(
            "helper must be an executable root-owned non-symlink file that is not group/world writable"
                .to_string(),
        );
    }
    Ok(())
}

fn ensure_linux_root() -> Result<(), String> {
    if env::consts::OS != "linux" {
        return Err("provisioning is supported only on Linux".to_string());
    }
    let status = fs::read_to_string("/proc/self/status")
        .map_err(|error| format!("cannot read process status: {error}"))?;
    let effective_uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|value| value.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u32>().ok());
    if effective_uid != Some(0) {
        return Err("provisioner must run as root".to_string());
    }
    Ok(())
}

fn ensure_secure_directory(path: &Path, create: bool) -> Result<(), String> {
    if create && !path.exists() {
        fs::create_dir(path)
            .map_err(|error| format!("cannot create directory '{}': {error}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("cannot secure directory '{}': {error}", path.display()))?;
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect directory '{}': {error}", path.display()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
    {
        return Err(format!(
            "directory '{}' must be root-owned, non-symlink, and not group/world writable",
            path.display()
        ));
    }
    Ok(())
}

fn reject_unsafe_existing(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0 =>
        {
            Err(format!(
                "existing destination '{}' is not a secure root-owned regular file",
                path.display()
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot inspect destination '{}': {error}",
            path.display()
        )),
    }
}

fn render_config(uid: u32) -> String {
    format!("allowed_uid={uid}\n")
}

fn render_sudoers(user: &str) -> String {
    format!(
        "Cmnd_Alias GEWYVERN_EBPF_VALIDATION = {HELPER_PATH} probe, {HELPER_PATH} run --run-id * --device *, {HELPER_PATH} cleanup --run-id *\n{user} ALL=(root) NOPASSWD: GEWYVERN_EBPF_VALIDATION\n"
    )
}

fn write_temp(destination: &Path, bytes: &[u8], mode: u32) -> Result<PathBuf, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "destination has no parent directory".to_string())?;
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "destination filename is invalid".to_string())?;
    let temp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp)
        .map_err(|error| format!("cannot create temporary file '{}': {error}", temp.display()))?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "cannot write temporary file '{}': {error}",
            temp.display()
        ));
    }
    fs::set_permissions(&temp, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("cannot set temporary file mode: {error}"))?;
    Ok(temp)
}

fn validate_sudoers(path: &Path) -> Result<(), String> {
    let visudo = ["/usr/sbin/visudo", "/usr/bin/visudo"]
        .into_iter()
        .find(|candidate| Path::new(candidate).is_file())
        .ok_or_else(|| "visudo is required to provision helper access".to_string())?;
    let status = Command::new(visudo)
        .arg("-cf")
        .arg(path)
        .status()
        .map_err(|error| format!("cannot execute visudo: {error}"))?;
    if !status.success() {
        return Err("generated sudoers policy failed visudo validation".to_string());
    }
    Ok(())
}

fn install_temp(temp: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(temp, destination).map_err(|error| {
        format!(
            "cannot install '{}' as '{}': {error}",
            temp.display(),
            destination.display()
        )
    })?;
    let parent = destination
        .parent()
        .ok_or_else(|| "destination has no parent directory".to_string())?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "cannot synchronize directory '{}': {error}",
                parent.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_bounded_and_unambiguous() {
        assert_eq!(
            parse_args(&["--user".into(), "kyuubiki-dev".into(), "--dry-run".into()]).unwrap(),
            Options {
                user: "kyuubiki-dev".into(),
                dry_run: true,
            }
        );
        assert!(parse_args(&[]).is_err());
        assert!(parse_args(&["--user".into(), "root;sh".into()]).is_err());
        assert!(parse_args(&["--dry-run".into(), "--dry-run".into()]).is_err());
    }

    #[test]
    fn passwd_records_are_exact_and_never_authorize_root() {
        assert_eq!(
            parse_passwd_record(
                "operator:x:1001:1001::/home/operator:/bin/bash\n",
                "operator"
            )
            .unwrap(),
            1001
        );
        assert!(parse_passwd_record("root:x:0:0::/root:/bin/bash\n", "root").is_err());
        assert!(parse_passwd_record("other:x:1001:1001::/:/bin/sh\n", "operator").is_err());
        assert!(
            parse_passwd_record(
                "operator:x:1001:1001::/:/bin/sh\noperator:x:1002:1002::/:/bin/sh\n",
                "operator"
            )
            .is_err()
        );
    }

    #[test]
    fn rendered_policy_exposes_only_fixed_helper_commands() {
        let policy = render_sudoers("operator");
        assert!(policy.contains("/usr/libexec/gewyvern-ebpf-helper probe"));
        assert!(policy.contains("operator ALL=(root) NOPASSWD"));
        assert!(!policy.contains("/bin/sh"));
        assert!(!policy.contains(" env "));
        assert_eq!(render_config(1001), "allowed_uid=1001\n");
    }
}
