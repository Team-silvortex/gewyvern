use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use serde_json::json;

use crate::native_binary::is_mach_o_arm64;

use super::command::{
    DOTNET_PROOF_TIMEOUT, PROOF_FIXTURE_TIMEOUT, TOOL_PROBE_TIMEOUT, ValidationError,
    ValidationReport, default_out_dir, repo_root, run_command_output_with_timeout,
};
use super::leserpent_accessibility::require_accessibility_proof;

const FIXTURES: &[&str] = &[
    "renderer-conformance-v1.json",
    "renderer-debugger-conformance-v1.json",
    "renderer-log-conformance-v1.json",
    "renderer-workspace-conformance-v1.json",
];
const PRESENTATION_FIXTURE: &str = "renderer-presentation-conformance-v1.json";
const PRESENTATION_MARKERS: &[&str] = &[
    "Avalonia action activation valid:",
    "presentation_activate=true",
    "native_click_exactly_once=true",
    "unavailable_action_rejected=true",
    "hidden_action_rejected=true",
    "non_action_rejected=true",
    "missing_action_rejected=true",
    "Avalonia focus retention valid:",
    "reopen_window=true",
    "reclose_window=true",
    "window_lifecycle_idempotent=true",
    "window_reopen_fresh_native_window=true",
    "window_semantic_tree_rematerialized=true",
];
const MAX_ARTIFACT_FILES: usize = 12;
const MAX_DEBUG_SYMBOL_ENTRIES: usize = 16;
const MAX_DEBUG_SYMBOL_FILES: usize = 8;
const MAX_DEBUG_SYMBOL_BYTES: u64 = 128 * 1024 * 1024;

pub fn run_leserpent_aot_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let target = HostTarget::current()?;
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-aot"));
    fs::create_dir_all(&out_dir)?;
    clear_previous_evidence(&out_dir)?;
    let artifact_dir = out_dir.join("artifact");
    let dotnet_artifacts = out_dir.join("dotnet-artifacts");
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir)?;
    }
    fs::create_dir_all(&artifact_dir)?;
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(&dotnet_artifacts)?;
    }

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
        TOOL_PROBE_TIMEOUT,
    )?;
    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("restore")
            .arg(&app)
            .args([
                "-p:PublishProfile=NativeAot",
                "-p:PublishAot=true",
                "--locked-mode",
            ])
            .arg("--artifacts-path")
            .arg(&dotnet_artifacts),
        &out_dir.join("restore.log"),
        "locked NativeAOT restore failed",
        DOTNET_PROOF_TIMEOUT,
    )?;
    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("publish")
            .arg(&app)
            .args(["-p:PublishProfile=NativeAot", "-r", target.rid])
            .arg("--no-restore")
            .arg("--artifacts-path")
            .arg(&dotnet_artifacts)
            .arg("-o")
            .arg(&artifact_dir),
        &out_dir.join("publish.log"),
        "NativeAOT publish failed",
        DOTNET_PROOF_TIMEOUT,
    )?;

    let executable = artifact_dir.join(target.executable_name);
    let executable_bytes = validate_native_executable(&executable, target.magic)?;
    let artifact_inventory = artifact_inventory(&artifact_dir, target.executable_name)?;
    if artifact_inventory.files.is_empty() || artifact_inventory.files.len() > MAX_ARTIFACT_FILES {
        return Err(ValidationError::new(format!(
            "NativeAOT artifact file count must be 1..={MAX_ARTIFACT_FILES}, got {}",
            artifact_inventory.files.len()
        )));
    }

    let fixtures_dir = root.join("apps/leserpent-avalonia/fixtures");
    for fixture in FIXTURES {
        let fixture_path = fixtures_dir.join(fixture);
        let log_path = out_dir.join(format!("fixture-{fixture}.log"));
        let output = run_fixture(
            target,
            &executable,
            &fixture_path,
            &log_path,
            "--verify-controls",
            "control fixture",
        )?;
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
    }
    let presentation_fixture_path = fixtures_dir.join(PRESENTATION_FIXTURE);
    let presentation_log_path = out_dir.join(format!("fixture-{PRESENTATION_FIXTURE}.log"));
    let presentation_output = run_fixture(
        target,
        &executable,
        &presentation_fixture_path,
        &presentation_log_path,
        "--verify-focus-retention",
        "presentation fixture",
    )?;
    let presentation_text = String::from_utf8_lossy(&presentation_output.stdout);
    for marker in PRESENTATION_MARKERS {
        if !presentation_text.contains(marker) {
            return Err(ValidationError::new(format!(
                "presentation fixture did not emit required marker `{marker}`"
            )));
        }
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
        "files": artifact_inventory.files,
        "debug_symbols": {
            "files": artifact_inventory.debug_symbol_files,
            "total_bytes": artifact_inventory.debug_symbol_bytes,
        },
        "subprocess_limits": {
            "tool_probe_seconds": TOOL_PROBE_TIMEOUT.as_secs(),
            "dotnet_seconds": DOTNET_PROOF_TIMEOUT.as_secs(),
            "fixture_seconds": PROOF_FIXTURE_TIMEOUT.as_secs(),
        },
        "fixtures": FIXTURES,
        "presentation_fixture": PRESENTATION_FIXTURE,
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
    evidence_files.push(format!("fixture-{PRESENTATION_FIXTURE}.log"));
    validate_evidence_files(&out_dir, &evidence_files)?;
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-aot",
            "files": evidence_files,
        }))?,
    )?;
    fs::remove_dir_all(&dotnet_artifacts)?;

    Ok(ValidationReport {
        name: format!("Leserpent NativeAOT validation ({})", target.rid),
        out_dir,
        checks: vec![
            "locked_restore".to_string(),
            "native_publish".to_string(),
            "native_executable_magic".to_string(),
            "bounded_artifact_manifest".to_string(),
            "strict_artifact_inventory".to_string(),
            "four_control_fixtures".to_string(),
            "native_presentation_fixture".to_string(),
            "native_action_activation".to_string(),
            "debugger_cancel_lifecycle".to_string(),
            "complete_evidence_index".to_string(),
            "isolated_dotnet_artifacts".to_string(),
            "bounded_dotnet_and_fixture_subprocesses".to_string(),
            "stale_evidence_invalidation".to_string(),
            "failed_fixture_log_retention".to_string(),
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

fn run_fixture(
    target: HostTarget,
    executable: &Path,
    fixture: &Path,
    log_path: &Path,
    mode: &str,
    label: &str,
) -> Result<Output, ValidationError> {
    let mut command = if target.needs_xvfb {
        let mut command = Command::new("xvfb-run");
        command.args(["-a", "-s", "-screen 0 1280x800x24"]);
        command.arg(executable);
        command
    } else {
        Command::new(executable)
    };
    command.arg(mode).arg(fixture);
    let dependency = if target.needs_xvfb {
        "; xvfb-run requires xvfb and xauth on the Linux host"
    } else {
        ""
    };
    let output = run_command_output_with_timeout(
        &mut command,
        PROOF_FIXTURE_TIMEOUT,
        &format!("{label} `{}`{dependency}", fixture.display()),
    )?;
    write_output(log_path, &output)?;
    if !output.status.success() {
        return Err(command_failure(
            &format!("{label} `{}` failed", fixture.display()),
            &output,
        ));
    }
    Ok(output)
}

fn clear_previous_evidence(out_dir: &Path) -> Result<(), ValidationError> {
    for name in [
        "environment.txt",
        "dotnet-version.log",
        "restore.log",
        "publish.log",
        "artifact-manifest.json",
        "evidence-index.json",
    ]
    .into_iter()
    .chain(FIXTURES.iter().copied())
    .map(|name| {
        if FIXTURES.contains(&name) {
            format!("fixture-{name}.log")
        } else {
            name.to_string()
        }
    })
    .chain(std::iter::once(format!(
        "fixture-{PRESENTATION_FIXTURE}.log"
    ))) {
        match fs::remove_file(out_dir.join(&name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ValidationError::new(format!(
                    "failed to remove stale NativeAOT evidence `{name}`: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn run_logged(
    command: &mut Command,
    log_path: &Path,
    context: &str,
    timeout: Duration,
) -> Result<Output, ValidationError> {
    let output = run_command_output_with_timeout(command, timeout, context)?;
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
        NativeMagic::MachO64 => is_mach_o_arm64(&bytes),
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

struct ArtifactInventory {
    files: Vec<String>,
    debug_symbol_files: Vec<String>,
    debug_symbol_bytes: u64,
}

fn artifact_inventory(
    dir: &Path,
    executable_name: &str,
) -> Result<ArtifactInventory, ValidationError> {
    let mut files = Vec::new();
    let mut debug_symbol_files = Vec::new();
    let mut debug_symbol_bytes = 0_u64;
    let debug_bundle_name = format!("{executable_name}.dSYM");
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            let name = entry.file_name().into_string().map_err(|_| {
                ValidationError::new("NativeAOT artifact inventory contains a non-UTF-8 filename")
            })?;
            files.push(name);
        } else if file_type.is_dir() && entry.file_name() == debug_bundle_name.as_str() {
            let debug_inventory = debug_symbol_inventory(dir, &entry.path())?;
            if debug_inventory.0.is_empty() {
                return Err(ValidationError::new(
                    "NativeAOT debug symbol bundle contains no files",
                ));
            }
            debug_symbol_files = debug_inventory.0;
            debug_symbol_bytes = debug_inventory.1;
        } else {
            return Err(ValidationError::new(format!(
                "NativeAOT artifact inventory contains a non-file entry: {}",
                entry.path().display()
            )));
        }
    }
    files.sort();
    Ok(ArtifactInventory {
        files,
        debug_symbol_files,
        debug_symbol_bytes,
    })
}

fn debug_symbol_inventory(
    root: &Path,
    bundle: &Path,
) -> Result<(Vec<String>, u64), ValidationError> {
    let mut pending = vec![bundle.to_path_buf()];
    let mut files = Vec::new();
    let mut total_bytes = 0_u64;
    let mut entries = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            entries += 1;
            if entries > MAX_DEBUG_SYMBOL_ENTRIES {
                return Err(ValidationError::new(format!(
                    "NativeAOT debug symbols exceed the {MAX_DEBUG_SYMBOL_ENTRIES}-entry limit"
                )));
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                return Err(ValidationError::new(format!(
                    "NativeAOT debug symbols contain a non-file entry: {}",
                    entry.path().display()
                )));
            }
            if files.len() >= MAX_DEBUG_SYMBOL_FILES {
                return Err(ValidationError::new(format!(
                    "NativeAOT debug symbols exceed the {MAX_DEBUG_SYMBOL_FILES}-file limit"
                )));
            }
            let metadata = entry.metadata()?;
            total_bytes = total_bytes.checked_add(metadata.len()).ok_or_else(|| {
                ValidationError::new("NativeAOT debug symbol byte count overflowed")
            })?;
            if total_bytes > MAX_DEBUG_SYMBOL_BYTES {
                return Err(ValidationError::new(format!(
                    "NativeAOT debug symbols exceed the {MAX_DEBUG_SYMBOL_BYTES}-byte limit"
                )));
            }
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|_| {
                ValidationError::new("NativeAOT debug symbol escaped the artifact root")
            })?;
            files.push(
                relative
                    .to_str()
                    .ok_or_else(|| {
                        ValidationError::new(
                            "NativeAOT debug symbol inventory contains a non-UTF-8 path",
                        )
                    })?
                    .to_string(),
            );
        }
    }
    files.sort();
    Ok((files, total_bytes))
}

fn validate_evidence_files(root: &Path, files: &[String]) -> Result<(), ValidationError> {
    let mut unique = BTreeSet::new();
    for file in files {
        if !unique.insert(file) {
            return Err(ValidationError::new(format!(
                "NativeAOT evidence index contains a duplicate entry: {file}"
            )));
        }
        let path = root.join(file);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            ValidationError::new(format!(
                "NativeAOT evidence file is unavailable: {}: {error}",
                path.display()
            ))
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ValidationError::new(format!(
                "NativeAOT evidence entry is not a regular file: {}",
                path.display()
            )));
        }
    }
    Ok(())
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

    #[test]
    fn artifact_inventory_rejects_entries_it_cannot_account_for() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-aot-inventory-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Leserpent.Avalonia"), b"artifact").unwrap();
        fs::create_dir(root.join("unaccounted")).unwrap();
        assert!(artifact_inventory(&root, "Leserpent.Avalonia").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn artifact_inventory_accounts_for_bounded_debug_symbols() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-aot-debug-inventory-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let debug = root.join("Leserpent.Avalonia.dSYM/Contents/Resources/DWARF");
        fs::create_dir_all(&debug).unwrap();
        fs::write(root.join("Leserpent.Avalonia"), b"artifact").unwrap();
        fs::write(debug.join("Leserpent.Avalonia"), b"debug-symbols").unwrap();

        let inventory = artifact_inventory(&root, "Leserpent.Avalonia").unwrap();
        assert_eq!(inventory.files, ["Leserpent.Avalonia"]);
        assert_eq!(
            inventory.debug_symbol_files,
            ["Leserpent.Avalonia.dSYM/Contents/Resources/DWARF/Leserpent.Avalonia"]
        );
        assert_eq!(inventory.debug_symbol_bytes, 13);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn evidence_index_requires_unique_regular_files() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-aot-evidence-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("proof.log"), b"proof").unwrap();
        assert!(validate_evidence_files(&root, &["proof.log".to_string()]).is_ok());
        assert!(
            validate_evidence_files(&root, &["proof.log".to_string(), "proof.log".to_string()])
                .is_err()
        );
        assert!(validate_evidence_files(&root, &["missing.log".to_string()]).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
