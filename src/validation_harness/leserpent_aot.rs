use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};
use super::leserpent_accessibility::require_accessibility_proof;

const FIXTURES: &[&str] = &[
    "renderer-conformance-v1.json",
    "renderer-debugger-conformance-v1.json",
    "renderer-log-conformance-v1.json",
    "renderer-workspace-conformance-v1.json",
];
const MAX_ARTIFACT_FILES: usize = 12;

pub fn run_leserpent_aot_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let target = HostTarget::current()?;
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-aot"));
    let artifact_dir = out_dir.join("artifact");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    fs::create_dir_all(&artifact_dir)?;

    let root = repo_root();
    let app = root.join("apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj");
    if !app.is_file() {
        return Err(ValidationError::new(format!(
            "Leserpent Avalonia project not found: {}",
            app.display()
        )));
    }

    let dotnet_version = run_logged(
        Command::new("dotnet").arg("--version"),
        &out_dir.join("dotnet-version.log"),
        "failed to query dotnet SDK",
    )?;
    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("restore")
            .arg(&app)
            .args(["-p:PublishProfile=NativeAot", "--locked-mode"]),
        &out_dir.join("restore.log"),
        "locked NativeAOT restore failed",
    )?;
    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("publish")
            .arg(&app)
            .args(["-p:PublishProfile=NativeAot", "-r", target.rid])
            .arg("--no-restore")
            .arg("-o")
            .arg(&artifact_dir),
        &out_dir.join("publish.log"),
        "NativeAOT publish failed",
    )?;

    let executable = artifact_dir.join(target.executable_name);
    let executable_bytes = validate_native_executable(&executable, target.magic)?;
    let artifact_files = artifact_files(&artifact_dir)?;
    if artifact_files.is_empty() || artifact_files.len() > MAX_ARTIFACT_FILES {
        return Err(ValidationError::new(format!(
            "NativeAOT artifact file count must be 1..={MAX_ARTIFACT_FILES}, got {}",
            artifact_files.len()
        )));
    }

    let fixtures_dir = root.join("apps/leserpent-avalonia/fixtures");
    for fixture in FIXTURES {
        let fixture_path = fixtures_dir.join(fixture);
        let output = run_control_fixture(target, &executable, &fixture_path)?;
        let text = String::from_utf8_lossy(&output.stdout);
        if !text.contains("Avalonia controls valid:") {
            return Err(ValidationError::new(format!(
                "fixture `{fixture}` did not emit the Avalonia control proof marker"
            )));
        }
        require_accessibility_proof(&text, fixture)?;
        if *fixture == "renderer-debugger-conformance-v1.json"
            && !(text.contains("initial_debugger_cancel_buttons=1")
                && text.contains("remaining_debugger_cancel_buttons=0"))
        {
            return Err(ValidationError::new(
                "debugger fixture did not prove cancel-control lifecycle 1 -> 0",
            ));
        }
        write_output(&out_dir.join(format!("fixture-{fixture}.log")), &output)?;
    }

    let dotnet_version = first_stdout_line(&dotnet_version);
    fs::write(
        out_dir.join("environment.txt"),
        format!(
            "os={}\narch={}\nrid={}\ndotnet_sdk={}\n",
            env::consts::OS,
            env::consts::ARCH,
            target.rid,
            dotnet_version
        ),
    )?;
    let manifest = json!({
        "schema_version": 1,
        "rid": target.rid,
        "executable": target.executable_name,
        "executable_bytes": executable_bytes,
        "files": artifact_files,
        "fixtures": FIXTURES,
    });
    fs::write(
        out_dir.join("artifact-manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    let mut evidence_files = vec![
        "environment.txt".to_string(),
        "dotnet-version.log".to_string(),
        "restore.log".to_string(),
        "publish.log".to_string(),
        "artifact-manifest.json".to_string(),
    ];
    evidence_files.extend(
        FIXTURES
            .iter()
            .map(|fixture| format!("fixture-{fixture}.log")),
    );
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-aot",
            "files": evidence_files,
        }))?,
    )?;

    Ok(ValidationReport {
        name: format!("Leserpent NativeAOT validation ({})", target.rid),
        out_dir,
        checks: vec![
            "locked_restore".to_string(),
            "native_publish".to_string(),
            "native_executable_magic".to_string(),
            "bounded_artifact_manifest".to_string(),
            "four_control_fixtures".to_string(),
            "debugger_cancel_lifecycle".to_string(),
        ],
    })
}

#[derive(Clone, Copy)]
struct HostTarget {
    rid: &'static str,
    executable_name: &'static str,
    magic: NativeMagic,
    needs_xvfb: bool,
}

impl HostTarget {
    fn current() -> Result<Self, ValidationError> {
        host_target(env::consts::OS, env::consts::ARCH).ok_or_else(|| {
            ValidationError::new(format!(
                "Leserpent NativeAOT validation does not support host {}-{}",
                env::consts::OS,
                env::consts::ARCH
            ))
        })
    }
}

#[derive(Clone, Copy)]
enum NativeMagic {
    Elf,
    MachO64,
}

fn host_target(os: &str, arch: &str) -> Option<HostTarget> {
    match (os, arch) {
        ("linux", "x86_64") => Some(HostTarget {
            rid: "linux-x64",
            executable_name: "Leserpent.Avalonia",
            magic: NativeMagic::Elf,
            needs_xvfb: true,
        }),
        ("macos", "aarch64") => Some(HostTarget {
            rid: "osx-arm64",
            executable_name: "Leserpent.Avalonia",
            magic: NativeMagic::MachO64,
            needs_xvfb: false,
        }),
        _ => None,
    }
}

fn run_control_fixture(
    target: HostTarget,
    executable: &Path,
    fixture: &Path,
) -> Result<Output, ValidationError> {
    let mut command = if target.needs_xvfb {
        let mut command = Command::new("xvfb-run");
        command.args(["-a", "-s", "-screen 0 1280x800x24"]);
        command.arg(executable);
        command
    } else {
        Command::new(executable)
    };
    let output = command
        .args(["--verify-controls"])
        .arg(fixture)
        .output()
        .map_err(|err| {
            let dependency = if target.needs_xvfb {
                "; install xvfb and xauth on the Linux host"
            } else {
                ""
            };
            ValidationError::new(format!(
                "failed to execute control fixture `{}`: {err}{dependency}",
                fixture.display()
            ))
        })?;
    if !output.status.success() {
        return Err(command_failure(
            &format!("control fixture `{}` failed", fixture.display()),
            &output,
        ));
    }
    Ok(output)
}

fn run_logged(
    command: &mut Command,
    log_path: &Path,
    context: &str,
) -> Result<Output, ValidationError> {
    let output = command
        .output()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    write_output(log_path, &output)?;
    if !output.status.success() {
        return Err(command_failure(context, &output));
    }
    Ok(output)
}

fn write_output(path: &Path, output: &Output) -> Result<(), ValidationError> {
    let mut transcript = Vec::new();
    transcript.extend_from_slice(b"stdout:\n");
    transcript.extend_from_slice(&output.stdout);
    transcript.extend_from_slice(b"\n\nstderr:\n");
    transcript.extend_from_slice(&output.stderr);
    fs::write(path, transcript)?;
    Ok(())
}

fn command_failure(context: &str, output: &Output) -> ValidationError {
    ValidationError::new(format!(
        "{context}: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn validate_native_executable(path: &Path, expected: NativeMagic) -> Result<u64, ValidationError> {
    let bytes = fs::read(path).map_err(|err| {
        ValidationError::new(format!(
            "failed to read native executable {}: {err}",
            path.display()
        ))
    })?;
    let valid = match expected {
        NativeMagic::Elf => bytes.starts_with(b"\x7fELF"),
        NativeMagic::MachO64 => bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe]),
    };
    if !valid {
        return Err(ValidationError::new(format!(
            "published executable has the wrong native file signature: {}",
            path.display()
        )));
    }
    u64::try_from(bytes.len())
        .map_err(|_| ValidationError::new("native executable size does not fit in u64"))
}

fn artifact_files(dir: &Path) -> Result<Vec<String>, ValidationError> {
    let mut files = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(|kind| kind.is_file())
                .map(|_| entry.file_name().to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn first_stdout_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("unknown")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_proven_native_host_targets() {
        assert_eq!(host_target("linux", "x86_64").unwrap().rid, "linux-x64");
        assert_eq!(host_target("macos", "aarch64").unwrap().rid, "osx-arm64");
        assert!(host_target("windows", "x86_64").is_none());
        assert!(host_target("linux", "aarch64").is_none());
    }

    #[test]
    fn validates_native_file_signatures() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-aot-magic-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&root).unwrap();
        let elf = root.join("elf");
        fs::write(&elf, b"\x7fELFpayload").unwrap();
        assert_eq!(
            validate_native_executable(&elf, NativeMagic::Elf).unwrap(),
            11
        );
        assert!(validate_native_executable(&elf, NativeMagic::MachO64).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
