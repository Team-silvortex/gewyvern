use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LinuxEbpfSmokeError {
    UnsupportedPlatform,
    InvalidTarget(String),
    Io(String),
    CommandFailed(String),
}

impl std::fmt::Display for LinuxEbpfSmokeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str("linux eBPF smoke requires Linux"),
            Self::InvalidTarget(message) => formatter.write_str(message),
            Self::Io(message) => formatter.write_str(message),
            Self::CommandFailed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for LinuxEbpfSmokeError {}

pub fn run_tracepoint_attach_smoke(
    hookpoint_name: &str,
    transcript_path: Option<&Path>,
) -> Result<(), LinuxEbpfSmokeError> {
    ensure_linux()?;
    validate_tracepoint_name(hookpoint_name)?;

    let Some((category, event)) = hookpoint_name.split_once('/') else {
        return Err(LinuxEbpfSmokeError::InvalidTarget(format!(
            "tracepoint target must contain category/event, got `{hookpoint_name}`"
        )));
    };

    let tmp_dir = temp_work_dir("linux-attach-smoke")?;
    let bpf_obj = tmp_dir.join("tracepoint_min.bpf.o");
    let loader_bin = tmp_dir.join("attach_smoke");
    let mut transcript = String::new();

    compile_bpf_smoke_object("ebpf/smoke/tracepoint_min.bpf.c", &bpf_obj, &mut transcript)?;
    compile_linux_smoke_loader("ebpf/smoke/attach_smoke.c", &loader_bin, &mut transcript)?;
    run_command(
        Command::new(&loader_bin)
            .arg(&bpf_obj)
            .arg(category)
            .arg(event),
        &mut transcript,
    )?;
    transcript.push_str("linux attach smoke ok\n");

    finish_run(tmp_dir, transcript_path, transcript)
}

pub fn run_kprobe_attach_smoke(
    symbol_name: &str,
    transcript_path: Option<&Path>,
) -> Result<(), LinuxEbpfSmokeError> {
    ensure_linux()?;
    validate_symbol_name(symbol_name)?;

    let tmp_dir = temp_work_dir("linux-kprobe-smoke")?;
    let bpf_obj = tmp_dir.join("kprobe_min.bpf.o");
    let loader_bin = tmp_dir.join("attach_kprobe_smoke");
    let mut transcript = String::new();

    compile_bpf_smoke_object("ebpf/smoke/kprobe_min.bpf.c", &bpf_obj, &mut transcript)?;
    compile_linux_smoke_loader(
        "ebpf/smoke/attach_kprobe_smoke.c",
        &loader_bin,
        &mut transcript,
    )?;
    run_command(
        Command::new(&loader_bin).arg(&bpf_obj).arg(symbol_name),
        &mut transcript,
    )?;
    transcript.push_str("linux kprobe smoke ok\n");

    finish_run(tmp_dir, transcript_path, transcript)
}

pub fn run_tc_attach_smoke(
    dev_name: &str,
    transcript_path: Option<&Path>,
) -> Result<(), LinuxEbpfSmokeError> {
    ensure_linux()?;
    validate_netdev_name(dev_name)?;

    let tmp_dir = temp_work_dir("linux-tc-smoke")?;
    let bpf_obj = tmp_dir.join("tc_min.bpf.o");
    let mut transcript = String::new();

    compile_bpf_smoke_object("ebpf/smoke/tc_min.bpf.c", &bpf_obj, &mut transcript)?;

    let cleanup_result = tc_qdisc_delete(dev_name, &mut transcript);
    let run_result = (|| {
        tc_qdisc_add(dev_name, &mut transcript)?;
        tc_filter_replace(dev_name, &bpf_obj, &mut transcript)?;
        transcript.push_str("linux tc smoke ok\n");
        Ok(())
    })();
    let cleanup_after_run = tc_qdisc_delete(dev_name, &mut transcript);

    let result = run_result
        .and(cleanup_after_run)
        .and(cleanup_result.or(Ok(())));
    finish_run(tmp_dir, transcript_path, transcript)?;
    result
}

pub fn validate_tracepoint_name(name: &str) -> Result<(), LinuxEbpfSmokeError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '/' | '-'),
        "tracepoint",
    )
}

pub fn validate_symbol_name(name: &str) -> Result<(), LinuxEbpfSmokeError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'),
        "symbol",
    )
}

pub fn validate_netdev_name(name: &str) -> Result<(), LinuxEbpfSmokeError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'),
        "netdev",
    )
}

fn ensure_linux() -> Result<(), LinuxEbpfSmokeError> {
    if cfg!(target_os = "linux") {
        Ok(())
    } else {
        Err(LinuxEbpfSmokeError::UnsupportedPlatform)
    }
}

fn validate_probe_target<F>(
    value: &str,
    is_allowed: F,
    label: &str,
) -> Result<(), LinuxEbpfSmokeError>
where
    F: Fn(char) -> bool,
{
    if value.is_empty() || value.len() > 128 || value.chars().any(|ch| !is_allowed(ch)) {
        return Err(LinuxEbpfSmokeError::InvalidTarget(format!(
            "invalid {label} target '{value}'"
        )));
    }
    Ok(())
}

fn temp_work_dir(prefix: &str) -> Result<PathBuf, LinuxEbpfSmokeError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gewyvern-{prefix}-{nonce}"));
    fs::create_dir_all(&path).map_err(io_err)?;
    Ok(path)
}

fn repo_root() -> PathBuf {
    if let Ok(current_dir) = std::env::current_dir() {
        if current_dir.join("ebpf").join("smoke").is_dir() {
            return current_dir;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn compile_bpf_smoke_object(
    source_file: &str,
    output_file: &Path,
    transcript: &mut String,
) -> Result<(), LinuxEbpfSmokeError> {
    let arch_include = format!("/usr/include/{}-linux-gnu", std::env::consts::ARCH);
    let mut command = Command::new("clang");
    command
        .arg("-O2")
        .arg("-g")
        .arg("-target")
        .arg("bpf")
        .arg("-I/usr/include")
        .arg(format!("-I{arch_include}"))
        .arg("-c")
        .arg(repo_root().join(source_file))
        .arg("-o")
        .arg(output_file);
    run_command(&mut command, transcript)
}

fn compile_linux_smoke_loader(
    source_file: &str,
    output_file: &Path,
    transcript: &mut String,
) -> Result<(), LinuxEbpfSmokeError> {
    let mut command = Command::new("cc");
    command
        .arg("-O2")
        .arg("-g")
        .arg(repo_root().join(source_file))
        .arg("-lbpf")
        .arg("-lelf")
        .arg("-lz")
        .arg("-o")
        .arg(output_file);
    run_command(&mut command, transcript)
}

fn tc_qdisc_add(dev_name: &str, transcript: &mut String) -> Result<(), LinuxEbpfSmokeError> {
    run_command(
        Command::new("tc")
            .arg("qdisc")
            .arg("add")
            .arg("dev")
            .arg(dev_name)
            .arg("clsact"),
        transcript,
    )
}

fn tc_filter_replace(
    dev_name: &str,
    bpf_obj: &Path,
    transcript: &mut String,
) -> Result<(), LinuxEbpfSmokeError> {
    run_command(
        Command::new("tc")
            .arg("filter")
            .arg("replace")
            .arg("dev")
            .arg(dev_name)
            .arg("ingress")
            .arg("bpf")
            .arg("da")
            .arg("obj")
            .arg(bpf_obj)
            .arg("sec")
            .arg("classifier/tc_ingress"),
        transcript,
    )
}

fn tc_qdisc_delete(dev_name: &str, transcript: &mut String) -> Result<(), LinuxEbpfSmokeError> {
    match run_command_allow_failure(
        Command::new("tc")
            .arg("qdisc")
            .arg("del")
            .arg("dev")
            .arg(dev_name)
            .arg("clsact"),
        transcript,
    ) {
        Ok(_) => Ok(()),
        Err(err) if err.contains("No such file or directory") => Ok(()),
        Err(err) => Err(LinuxEbpfSmokeError::CommandFailed(err)),
    }
}

fn run_command(command: &mut Command, transcript: &mut String) -> Result<(), LinuxEbpfSmokeError> {
    let output = command.output().map_err(io_err)?;
    record_command(transcript, command, &output);
    if output.status.success() {
        Ok(())
    } else {
        Err(LinuxEbpfSmokeError::CommandFailed(command_failure_message(
            command, &output,
        )))
    }
}

fn run_command_allow_failure(command: &mut Command, transcript: &mut String) -> Result<(), String> {
    let output = command.output().map_err(|err| err.to_string())?;
    record_command(transcript, command, &output);
    if output.status.success() {
        Ok(())
    } else {
        Err(command_failure_message(command, &output))
    }
}

fn record_command(transcript: &mut String, command: &Command, output: &Output) {
    transcript.push_str("$ ");
    transcript.push_str(&render_command(command));
    transcript.push('\n');
    if !output.stdout.is_empty() {
        transcript.push_str("stdout:\n");
        transcript.push_str(&String::from_utf8_lossy(&output.stdout));
        if !transcript.ends_with('\n') {
            transcript.push('\n');
        }
    }
    if !output.stderr.is_empty() {
        transcript.push_str("stderr:\n");
        transcript.push_str(&String::from_utf8_lossy(&output.stderr));
        if !transcript.ends_with('\n') {
            transcript.push('\n');
        }
    }
    transcript.push_str(&format!("status: {}\n\n", output.status));
}

fn command_failure_message(command: &Command, output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("command exited with status {}", output.status)
    };
    format!("{} failed: {detail}", render_command(command))
}

fn render_command(command: &Command) -> String {
    let program = command.get_program().to_string_lossy();
    let args = command
        .get_args()
        .map(render_os_arg)
        .collect::<Vec<_>>()
        .join(" ");
    if args.is_empty() {
        program.into_owned()
    } else {
        format!("{program} {args}")
    }
}

fn render_os_arg(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if text
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        text.into_owned()
    } else {
        format!("{text:?}")
    }
}

fn finish_run(
    tmp_dir: PathBuf,
    transcript_path: Option<&Path>,
    transcript: String,
) -> Result<(), LinuxEbpfSmokeError> {
    if let Some(path) = transcript_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        fs::write(path, transcript).map_err(io_err)?;
    }
    fs::remove_dir_all(tmp_dir).map_err(io_err)?;
    Ok(())
}

fn io_err(err: std::io::Error) -> LinuxEbpfSmokeError {
    LinuxEbpfSmokeError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        LinuxEbpfSmokeError, validate_netdev_name, validate_symbol_name, validate_tracepoint_name,
    };

    #[test]
    fn tracepoint_validation_rejects_shell_metacharacters() {
        let err = validate_tracepoint_name("syscalls/sys_enter_openat;touch").unwrap_err();
        assert!(matches!(err, LinuxEbpfSmokeError::InvalidTarget(_)));
    }

    #[test]
    fn symbol_validation_rejects_path_characters() {
        let err = validate_symbol_name("../tcp_v4_connect").unwrap_err();
        assert!(matches!(err, LinuxEbpfSmokeError::InvalidTarget(_)));
    }

    #[test]
    fn netdev_validation_rejects_whitespace() {
        let err = validate_netdev_name("eth0 prod").unwrap_err();
        assert!(matches!(err, LinuxEbpfSmokeError::InvalidTarget(_)));
    }
}
