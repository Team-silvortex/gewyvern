use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LinuxEbpfSmokeError {
    UnsupportedPlatform,
    InvalidTarget(String),
    UnsafeHostState(String),
    Io(String),
    CommandFailed(String),
}

impl std::fmt::Display for LinuxEbpfSmokeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str("linux eBPF smoke requires Linux"),
            Self::InvalidTarget(message) => formatter.write_str(message),
            Self::UnsafeHostState(message) => formatter.write_str(message),
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

    let result = (|| {
        compile_bpf_smoke_object("ebpf/smoke/tracepoint_min.bpf.c", &bpf_obj, &mut transcript)?;
        compile_linux_smoke_loader("ebpf/smoke/attach_smoke.c", &loader_bin, &mut transcript)?;
        run_command(
            Command::new(&loader_bin)
                .arg(&bpf_obj)
                .arg(category)
                .arg(event),
            &mut transcript,
        )
    })();
    if result.is_ok() {
        transcript.push_str("linux attach smoke ok\n");
    }

    finalize_run_result(tmp_dir, transcript_path, transcript, result)
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

    let result = (|| {
        compile_bpf_smoke_object("ebpf/smoke/kprobe_min.bpf.c", &bpf_obj, &mut transcript)?;
        compile_linux_smoke_loader(
            "ebpf/smoke/attach_kprobe_smoke.c",
            &loader_bin,
            &mut transcript,
        )?;
        run_command(
            Command::new(&loader_bin).arg(&bpf_obj).arg(symbol_name),
            &mut transcript,
        )
    })();
    if result.is_ok() {
        transcript.push_str("linux kprobe smoke ok\n");
    }

    finalize_run_result(tmp_dir, transcript_path, transcript, result)
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

    let result = (|| {
        compile_bpf_smoke_object("ebpf/smoke/tc_min.bpf.c", &bpf_obj, &mut transcript)?;
        run_tc_attach_commands(dev_name, &bpf_obj, &mut transcript)?;
        transcript.push_str("linux tc smoke ok\n");
        Ok(())
    })();

    finalize_run_result(tmp_dir, transcript_path, transcript, result)
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
    for _ in 0..32 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "gewyvern-{prefix}-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_err(error)),
        }
    }
    Err(LinuxEbpfSmokeError::Io(
        "failed to allocate a unique eBPF smoke directory".to_string(),
    ))
}

fn repo_root() -> PathBuf {
    if let Ok(current_dir) = std::env::current_dir()
        && current_dir.join("ebpf").join("smoke").is_dir()
    {
        return current_dir;
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

fn run_tc_attach_commands_with<F>(
    dev_name: &str,
    bpf_obj: &Path,
    transcript: &mut String,
    mut run: F,
) -> Result<(), LinuxEbpfSmokeError>
where
    F: FnMut(&mut Command, &mut String) -> Result<Output, LinuxEbpfSmokeError>,
{
    let existing = run(
        Command::new("tc")
            .arg("qdisc")
            .arg("show")
            .arg("dev")
            .arg(dev_name),
        transcript,
    )?;
    if String::from_utf8_lossy(&existing.stdout)
        .lines()
        .any(|line| line.split_whitespace().take(2).eq(["qdisc", "clsact"]))
    {
        return Err(LinuxEbpfSmokeError::UnsafeHostState(format!(
            "refusing TC smoke on `{dev_name}` because a clsact qdisc already exists"
        )));
    }

    run(
        Command::new("tc")
            .arg("qdisc")
            .arg("add")
            .arg("dev")
            .arg(dev_name)
            .arg("clsact"),
        transcript,
    )?;

    let attach_result = run(
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
    );
    let cleanup_result = run(
        Command::new("tc")
            .arg("qdisc")
            .arg("del")
            .arg("dev")
            .arg(dev_name)
            .arg("clsact"),
        transcript,
    );

    match (attach_result, cleanup_result) {
        (Ok(_), Ok(_)) => Ok(()),
        (Err(attach), Ok(_)) => Err(attach),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(attach), Err(cleanup)) => Err(LinuxEbpfSmokeError::CommandFailed(format!(
            "{attach}; cleanup also failed: {cleanup}"
        ))),
    }
}

fn run_tc_attach_commands(
    dev_name: &str,
    bpf_obj: &Path,
    transcript: &mut String,
) -> Result<(), LinuxEbpfSmokeError> {
    run_tc_attach_commands_with(dev_name, bpf_obj, transcript, run_command_output)
}

fn run_command(command: &mut Command, transcript: &mut String) -> Result<(), LinuxEbpfSmokeError> {
    run_command_output(command, transcript).map(|_| ())
}

fn run_command_output(
    command: &mut Command,
    transcript: &mut String,
) -> Result<Output, LinuxEbpfSmokeError> {
    let output = command.output().map_err(|error| {
        transcript.push_str("$ ");
        transcript.push_str(&render_command(command));
        transcript.push_str("\nspawn error: ");
        transcript.push_str(&error.to_string());
        transcript.push_str("\n\n");
        io_err(error)
    })?;
    record_command(transcript, command, &output);
    if output.status.success() {
        Ok(output)
    } else {
        Err(LinuxEbpfSmokeError::CommandFailed(command_failure_message(
            command, &output,
        )))
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

fn write_transcript(
    transcript_path: Option<&Path>,
    transcript: &str,
) -> Result<(), LinuxEbpfSmokeError> {
    if let Some(path) = transcript_path {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        fs::write(path, transcript).map_err(io_err)?;
    }
    Ok(())
}

fn finalize_run_result(
    tmp_dir: PathBuf,
    transcript_path: Option<&Path>,
    transcript: String,
    result: Result<(), LinuxEbpfSmokeError>,
) -> Result<(), LinuxEbpfSmokeError> {
    let transcript_result = write_transcript(transcript_path, &transcript);
    let cleanup_result = fs::remove_dir_all(tmp_dir).map_err(io_err);
    result?;
    transcript_result?;
    cleanup_result
}

fn io_err(err: std::io::Error) -> LinuxEbpfSmokeError {
    LinuxEbpfSmokeError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::{ExitStatus, Output};

    use super::{
        LinuxEbpfSmokeError, finalize_run_result, render_command, run_tc_attach_commands_with,
        temp_work_dir, validate_netdev_name, validate_symbol_name, validate_tracepoint_name,
    };

    #[cfg(unix)]
    fn successful_output(stdout: &str) -> Output {
        use std::os::unix::process::ExitStatusExt;

        Output {
            status: ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

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

    #[cfg(unix)]
    #[test]
    fn tc_smoke_refuses_to_modify_an_existing_clsact_qdisc() {
        let mut commands = Vec::new();
        let error = run_tc_attach_commands_with(
            "eth0",
            Path::new("/tmp/tc.o"),
            &mut String::new(),
            |command, _| {
                commands.push(render_command(command));
                Ok(successful_output("qdisc clsact ffff: parent ffff:fff1\n"))
            },
        )
        .unwrap_err();

        assert!(matches!(error, LinuxEbpfSmokeError::UnsafeHostState(_)));
        assert_eq!(commands, vec!["tc qdisc show dev eth0"]);
    }

    #[cfg(unix)]
    #[test]
    fn tc_smoke_does_not_delete_when_qdisc_creation_fails() {
        let mut commands = Vec::new();
        let error = run_tc_attach_commands_with(
            "eth0",
            Path::new("/tmp/tc.o"),
            &mut String::new(),
            |command, _| {
                let rendered = render_command(command);
                commands.push(rendered.clone());
                if rendered.contains(" qdisc add ") {
                    Err(LinuxEbpfSmokeError::CommandFailed(
                        "simulated add failure".to_string(),
                    ))
                } else {
                    Ok(successful_output(""))
                }
            },
        )
        .unwrap_err();

        assert!(matches!(error, LinuxEbpfSmokeError::CommandFailed(_)));
        assert_eq!(commands.len(), 2);
        assert!(
            !commands
                .iter()
                .any(|command| command.contains(" qdisc del "))
        );
    }

    #[cfg(unix)]
    #[test]
    fn tc_smoke_cleans_up_its_qdisc_after_attach_failure() {
        let mut commands = Vec::new();
        let error = run_tc_attach_commands_with(
            "eth0",
            Path::new("/tmp/tc.o"),
            &mut String::new(),
            |command, _| {
                let rendered = render_command(command);
                commands.push(rendered.clone());
                if rendered.contains(" filter replace ") {
                    Err(LinuxEbpfSmokeError::CommandFailed(
                        "simulated attach failure".to_string(),
                    ))
                } else {
                    Ok(successful_output(""))
                }
            },
        )
        .unwrap_err();

        assert!(matches!(error, LinuxEbpfSmokeError::CommandFailed(_)));
        assert!(
            commands
                .iter()
                .any(|command| command.contains(" qdisc del "))
        );
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn failed_smoke_still_writes_transcript_and_removes_work_dir() {
        let work_dir = temp_work_dir("finalize-test-work").unwrap();
        let evidence_dir = temp_work_dir("finalize-test-evidence").unwrap();
        let transcript_path = evidence_dir.join("run.log");
        let result = finalize_run_result(
            work_dir.clone(),
            Some(&transcript_path),
            "compile failed\n".to_string(),
            Err(LinuxEbpfSmokeError::CommandFailed(
                "simulated failure".to_string(),
            )),
        );

        assert!(matches!(result, Err(LinuxEbpfSmokeError::CommandFailed(_))));
        assert_eq!(
            std::fs::read_to_string(transcript_path).unwrap(),
            "compile failed\n"
        );
        assert!(!work_dir.exists());
        std::fs::remove_dir_all(evidence_dir).unwrap();
    }
}
