use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use ring::digest::{Context, SHA256, digest};

mod version;

const CONTROL_BUNDLE_RID: &str = "linux-x64";
const CONTROL_BUNDLE_MANIFEST: &str = "bundle-manifest.toml";
const CONTROL_BUNDLE_SUMS: &str = "SHA256SUMS";
const MAX_CONTROL_BUNDLE_FILES: usize = 4_096;
const MAX_CONTROL_BUNDLE_BYTES: u64 = 1024 * 1024 * 1024;

const USAGE: &str = r#"Usage:
  cargo dev doctor
  cargo dev version check
  cargo dev version set VERSION [--dry-run]
  cargo dev check [--scope core|control|desktop|all] [--restore] [--dry-run]
  cargo dev build [--scope core|control|desktop|all] [--release] [--restore] [--dry-run]
  cargo dev package linux [--format layout|deb|rpm|all] [--skip-build] [--out-dir PATH] [--dry-run]
  cargo dev package control [--output DIR] [--dry-run]
  cargo dev package desktop [--output APP] [--silvortex-issuer URL] [--identity ID --notary-profile PROFILE] [--dry-run]
  cargo dev deploy control [--output DIR] [--reuse] [--no-start] [--keep-releases N] [--dry-run]
  cargo dev deploy desktop [--output APP] [--silvortex-issuer URL] [--identity ID --notary-profile PROFILE] [--launch] [--dry-run]

The native workflow keeps compiler caches intact, reports stage timings, and
uses the checked package, bundle, and atomic installer boundaries."#;

fn main() {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
    {
        println!("{USAGE}");
        return;
    }
    let started = Instant::now();
    let result = Workflow::parse(arguments).and_then(|workflow| execute(workflow, repo_root()));
    match result {
        Ok(outcome) => eprintln!(
            "workflow complete: action={}, elapsed={:.3}s{}",
            outcome.action,
            started.elapsed().as_secs_f64(),
            outcome
                .artifact
                .as_deref()
                .map(|path| format!(", artifact={}", path.display()))
                .unwrap_or_default()
        ),
        Err(error) => {
            eprintln!("workflow failed: {error}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildScope {
    Core,
    Control,
    Desktop,
    All,
}

impl BuildScope {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "core" => Ok(Self::Core),
            "control" => Ok(Self::Control),
            "desktop" => Ok(Self::Desktop),
            "all" => Ok(Self::All),
            _ => Err(format!(
                "--scope must be core, control, desktop, or all; got `{value}`"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxPackageFormat {
    Layout,
    Deb,
    Rpm,
    All,
}

impl LinuxPackageFormat {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "layout" => Ok(Self::Layout),
            "deb" => Ok(Self::Deb),
            "rpm" => Ok(Self::Rpm),
            "all" => Ok(Self::All),
            _ => Err(format!(
                "--format must be layout, deb, rpm, or all; got `{value}`"
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::All => "all",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BuildOptions {
    scope: BuildScope,
    release: bool,
    restore: bool,
    dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct LinuxPackageOptions {
    format: LinuxPackageFormat,
    skip_build: bool,
    out_dir: Option<PathBuf>,
    dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct ControlOptions {
    output: PathBuf,
    install: bool,
    reuse: bool,
    no_start: bool,
    keep_releases: u16,
    dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct AppleReleaseOptions {
    identity: String,
    notary_profile: String,
}

#[derive(Debug, PartialEq, Eq)]
struct DesktopOptions {
    output: PathBuf,
    silvortex_issuer: Option<String>,
    apple_release: Option<AppleReleaseOptions>,
    install: bool,
    launch: bool,
    dry_run: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Workflow {
    Doctor,
    Version(version::VersionAction),
    Check(BuildOptions),
    Build(BuildOptions),
    PackageLinux(LinuxPackageOptions),
    Control(ControlOptions),
    Desktop(DesktopOptions),
}

impl Workflow {
    fn parse(arguments: Vec<String>) -> Result<Self, String> {
        let mut arguments = arguments.into_iter();
        match arguments.next().as_deref() {
            Some("doctor") => {
                reject_trailing(arguments)?;
                Ok(Self::Doctor)
            }
            Some("version") => version::VersionAction::parse(arguments).map(Self::Version),
            Some("check") => parse_check(arguments).map(Self::Check),
            Some("build") => parse_build(arguments).map(Self::Build),
            Some("package") => match arguments.next().as_deref() {
                Some("linux") => parse_linux_package(arguments).map(Self::PackageLinux),
                Some("control") => parse_control(arguments, false).map(Self::Control),
                Some("desktop") => parse_desktop(arguments, false).map(Self::Desktop),
                Some(value) => Err(format!("unknown package target `{value}`\n{USAGE}")),
                None => Err(format!(
                    "package requires linux, control, or desktop\n{USAGE}"
                )),
            },
            Some("deploy") => match arguments.next().as_deref() {
                Some("control") => parse_control(arguments, true).map(Self::Control),
                Some("desktop") => parse_desktop(arguments, true).map(Self::Desktop),
                Some(value) => Err(format!("unknown deploy target `{value}`\n{USAGE}")),
                None => Err(format!("deploy requires control or desktop\n{USAGE}")),
            },
            Some(value) => Err(format!("unknown workflow `{value}`\n{USAGE}")),
            None => Err(USAGE.to_string()),
        }
    }
}

fn parse_control(
    arguments: impl Iterator<Item = String>,
    install: bool,
) -> Result<ControlOptions, String> {
    let mut output = PathBuf::from("artifacts/leserpent/linux-x64");
    let mut output_seen = false;
    let mut reuse = false;
    let mut no_start = false;
    let mut keep_releases = 3_u16;
    let mut keep_releases_seen = false;
    let mut dry_run = false;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" if !output_seen => {
                output = PathBuf::from(next_value(&mut arguments, "--output")?);
                output_seen = true;
            }
            "--reuse" if install && !reuse => reuse = true,
            "--no-start" if install && !no_start => no_start = true,
            "--keep-releases" if install && !keep_releases_seen => {
                let value = next_value(&mut arguments, "--keep-releases")?;
                keep_releases = value.parse().map_err(|_| {
                    "--keep-releases must be an integer from 2 through 64".to_string()
                })?;
                if !(2..=64).contains(&keep_releases) {
                    return Err("--keep-releases must be an integer from 2 through 64".to_string());
                }
                keep_releases_seen = true;
            }
            "--dry-run" if !dry_run => dry_run = true,
            _ => return Err(format!("unknown or repeated control option `{argument}`")),
        }
    }
    if output.as_os_str().is_empty() || output == Path::new(".") || output == Path::new("..") {
        return Err("--output must identify a bundle directory".to_string());
    }
    Ok(ControlOptions {
        output,
        install,
        reuse,
        no_start,
        keep_releases,
        dry_run,
    })
}

fn parse_check(arguments: impl Iterator<Item = String>) -> Result<BuildOptions, String> {
    let options = parse_build(arguments)?;
    if options.release {
        return Err("check does not accept --release; use build --release".to_string());
    }
    Ok(options)
}

fn parse_build(arguments: impl Iterator<Item = String>) -> Result<BuildOptions, String> {
    let mut scope = BuildScope::All;
    let mut scope_seen = false;
    let mut release = false;
    let mut restore = false;
    let mut dry_run = false;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--scope" if !scope_seen => {
                scope = BuildScope::parse(&next_value(&mut arguments, "--scope")?)?;
                scope_seen = true;
            }
            "--release" if !release => release = true,
            "--restore" if !restore => restore = true,
            "--dry-run" if !dry_run => dry_run = true,
            _ => return Err(format!("unknown or repeated build option `{argument}`")),
        }
    }
    Ok(BuildOptions {
        scope,
        release,
        restore,
        dry_run,
    })
}

fn parse_linux_package(
    arguments: impl Iterator<Item = String>,
) -> Result<LinuxPackageOptions, String> {
    let mut format = LinuxPackageFormat::All;
    let mut format_seen = false;
    let mut skip_build = false;
    let mut out_dir = None;
    let mut dry_run = false;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--format" if !format_seen => {
                format = LinuxPackageFormat::parse(&next_value(&mut arguments, "--format")?)?;
                format_seen = true;
            }
            "--skip-build" if !skip_build => skip_build = true,
            "--out-dir" if out_dir.is_none() => {
                out_dir = Some(PathBuf::from(next_value(&mut arguments, "--out-dir")?));
            }
            "--dry-run" if !dry_run => dry_run = true,
            _ => return Err(format!("unknown or repeated package option `{argument}`")),
        }
    }
    Ok(LinuxPackageOptions {
        format,
        skip_build,
        out_dir,
        dry_run,
    })
}

fn parse_desktop(
    arguments: impl Iterator<Item = String>,
    install: bool,
) -> Result<DesktopOptions, String> {
    let mut output = PathBuf::from("artifacts/leserpent-avalonia/Leserpent.app");
    let mut output_seen = false;
    let mut silvortex_issuer = None;
    let mut identity = None;
    let mut notary_profile = None;
    let mut launch = false;
    let mut dry_run = false;
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--output" if !output_seen => {
                output = PathBuf::from(next_value(&mut arguments, "--output")?);
                output_seen = true;
            }
            "--silvortex-issuer" if silvortex_issuer.is_none() => {
                silvortex_issuer = Some(next_value(&mut arguments, "--silvortex-issuer")?);
            }
            "--identity" if identity.is_none() => {
                identity = Some(next_value(&mut arguments, "--identity")?);
            }
            "--notary-profile" if notary_profile.is_none() => {
                notary_profile = Some(next_value(&mut arguments, "--notary-profile")?);
            }
            "--launch" if install && !launch => launch = true,
            "--dry-run" if !dry_run => dry_run = true,
            _ => return Err(format!("unknown or repeated desktop option `{argument}`")),
        }
    }
    if output.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err("--output must end in .app".to_string());
    }
    if let Some(issuer) = silvortex_issuer.as_deref()
        && (!issuer.starts_with("https://") || !issuer.ends_with('/'))
    {
        return Err("--silvortex-issuer must be a canonical HTTPS origin ending in /".to_string());
    }
    let apple_release = match (identity, notary_profile) {
        (None, None) => None,
        (Some(identity), Some(notary_profile)) => {
            validate_apple_release_value(&identity, "--identity")?;
            validate_apple_release_value(&notary_profile, "--notary-profile")?;
            if !identity.starts_with("Developer ID Application:") {
                return Err(
                    "--identity must name a Developer ID Application certificate".to_string(),
                );
            }
            Some(AppleReleaseOptions {
                identity,
                notary_profile,
            })
        }
        _ => {
            return Err(
                "--identity and --notary-profile must be supplied together for an Apple release"
                    .to_string(),
            );
        }
    };
    Ok(DesktopOptions {
        output,
        silvortex_issuer,
        apple_release,
        install,
        launch,
        dry_run,
    })
}

fn validate_apple_release_value(value: &str, option: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('-')
        || value.chars().any(char::is_control)
    {
        return Err(format!("{option} is invalid"));
    }
    Ok(())
}

fn next_value(
    arguments: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<String, String> {
    arguments
        .next()
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or_else(|| format!("{option} requires a value"))
}

fn reject_trailing(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    match arguments.next() {
        Some(argument) => Err(format!("unexpected argument `{argument}`")),
        None => Ok(()),
    }
}

struct WorkflowOutcome {
    action: &'static str,
    artifact: Option<PathBuf>,
}

fn execute(workflow: Workflow, root: PathBuf) -> Result<WorkflowOutcome, String> {
    match workflow {
        Workflow::Doctor => {
            doctor(&root)?;
            Ok(WorkflowOutcome {
                action: "doctor",
                artifact: None,
            })
        }
        Workflow::Version(action) => {
            let label = action.label();
            version::execute(&root, action)?;
            Ok(WorkflowOutcome {
                action: label,
                artifact: None,
            })
        }
        Workflow::Check(options) => {
            run_parallel(check_specs(&root, &options), options.dry_run)?;
            Ok(WorkflowOutcome {
                action: "check",
                artifact: None,
            })
        }
        Workflow::Build(options) => {
            run_parallel(build_specs(&root, &options), options.dry_run)?;
            Ok(WorkflowOutcome {
                action: "build",
                artifact: None,
            })
        }
        Workflow::PackageLinux(options) => {
            let artifact = package_linux(&root, &options)?;
            Ok(WorkflowOutcome {
                action: "package-linux",
                artifact: Some(artifact),
            })
        }
        Workflow::Control(options) => {
            let action = if options.install {
                "deploy-control"
            } else {
                "package-control"
            };
            let artifact = control_pipeline(&root, &options)?;
            Ok(WorkflowOutcome {
                action,
                artifact: Some(artifact),
            })
        }
        Workflow::Desktop(options) => {
            let action = if options.install {
                "deploy-desktop"
            } else {
                "package-desktop"
            };
            let artifact = desktop_pipeline(&root, &options)?;
            Ok(WorkflowOutcome {
                action,
                artifact: Some(artifact),
            })
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("developer workflow crate must live under crates/")
        .to_path_buf()
}

#[derive(Clone, Debug)]
struct ProcessSpec {
    label: &'static str,
    program: OsString,
    arguments: Vec<OsString>,
    current_dir: PathBuf,
}

impl ProcessSpec {
    fn new(
        label: &'static str,
        program: impl Into<OsString>,
        arguments: impl IntoIterator<Item = impl Into<OsString>>,
        current_dir: &Path,
    ) -> Self {
        Self {
            label,
            program: program.into(),
            arguments: arguments.into_iter().map(Into::into).collect(),
            current_dir: current_dir.to_path_buf(),
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.arguments)
            .current_dir(&self.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        command
    }

    fn rendered(&self) -> String {
        std::iter::once(&self.program)
            .chain(self.arguments.iter())
            .map(|value| quote_argument(&value.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn build_specs(root: &Path, options: &BuildOptions) -> Vec<ProcessSpec> {
    compile_specs(root, options, false)
}

fn check_specs(root: &Path, options: &BuildOptions) -> Vec<ProcessSpec> {
    compile_specs(root, options, true)
}

fn compile_specs(root: &Path, options: &BuildOptions, check_only: bool) -> Vec<ProcessSpec> {
    let mut specs = Vec::new();
    if matches!(options.scope, BuildScope::Core | BuildScope::All) {
        let mut arguments = vec![
            if check_only { "check" } else { "build" },
            "--locked",
            "--workspace",
        ];
        if options.release {
            arguments.push("--release");
        }
        specs.push(ProcessSpec::new(
            if check_only {
                "rust-workspace-check"
            } else {
                "rust-workspace"
            },
            "cargo",
            arguments,
            root,
        ));
    }
    if matches!(options.scope, BuildScope::Control | BuildScope::All) {
        let mut arguments = vec![
            "build",
            "apps/leserpent/src/Leserpent/Leserpent.csproj",
            "--nologo",
            "--verbosity",
            "minimal",
        ];
        add_dotnet_restore_mode(
            &mut arguments,
            root,
            &["apps/leserpent/src/Leserpent/Leserpent.csproj"],
            options.restore,
        );
        if options.release {
            arguments.extend(["-c", "Release"]);
        }
        specs.push(ProcessSpec::new(
            "leserpent-control",
            "dotnet",
            arguments,
            root,
        ));
    }
    if matches!(options.scope, BuildScope::Desktop | BuildScope::All) {
        let mut arguments = vec![
            "build",
            "apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj",
            "--nologo",
            "--verbosity",
            "minimal",
        ];
        add_dotnet_restore_mode(
            &mut arguments,
            root,
            &[
                "apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj",
                "apps/leserpent-avalonia/src/Leserpent.RemoteClient/Leserpent.RemoteClient.csproj",
                "apps/leserpent-avalonia/src/Leserpent.RendererCore/Leserpent.RendererCore.csproj",
            ],
            options.restore,
        );
        if options.release {
            arguments.extend(["-c", "Release"]);
        }
        specs.push(ProcessSpec::new(
            "leserpent-desktop",
            "dotnet",
            arguments,
            root,
        ));
    }
    specs
}

fn add_dotnet_restore_mode(
    arguments: &mut Vec<&str>,
    root: &Path,
    projects: &[&str],
    force_restore: bool,
) {
    if !force_restore && dotnet_restore_is_fresh(root, projects) {
        arguments.push("--no-restore");
    } else {
        arguments.push("-p:RestoreLockedMode=true");
    }
}

fn dotnet_restore_is_fresh(root: &Path, projects: &[&str]) -> bool {
    let shared_inputs = [
        root.join("Directory.Build.props"),
        root.join("Directory.Build.targets"),
        root.join("global.json"),
        root.join("NuGet.Config"),
    ];
    projects.iter().all(|project| {
        let project = root.join(project);
        let Some(project_dir) = project.parent().map(Path::to_path_buf) else {
            return false;
        };
        let assets = project_dir.join("obj/project.assets.json");
        let Ok(assets_modified) = fs::metadata(&assets).and_then(|value| value.modified()) else {
            return false;
        };
        let local_inputs = [
            project,
            project_dir.join("packages.lock.json"),
            project_dir.join("packages.development.lock.json"),
        ];
        shared_inputs
            .iter()
            .chain(local_inputs.iter())
            .filter(|path| path.is_file())
            .all(|path| {
                fs::metadata(path)
                    .and_then(|value| value.modified())
                    .is_ok_and(|modified| modified <= assets_modified)
            })
    })
}

fn run_parallel(specs: Vec<ProcessSpec>, dry_run: bool) -> Result<(), String> {
    if specs.is_empty() {
        return Err("workflow contains no build stages".to_string());
    }
    if dry_run {
        for spec in specs {
            eprintln!("[dry-run:{}] {}", spec.label, spec.rendered());
        }
        return Ok(());
    }

    let started = Instant::now();
    let mut children: Vec<(ProcessSpec, Child, Instant)> = Vec::new();
    for spec in specs {
        eprintln!("[start:{}] {}", spec.label, spec.rendered());
        match spec.command().spawn() {
            Ok(child) => children.push((spec, child, Instant::now())),
            Err(error) => {
                for (_, child, _) in &mut children {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(format!("failed to start {}: {error}", spec.label));
            }
        }
    }

    let stage_count = children.len();
    let (sender, receiver) = mpsc::channel();
    for (spec, mut child, stage_started) in children {
        let sender = sender.clone();
        let label = spec.label;
        thread::spawn(move || {
            let result = child
                .wait()
                .map(|status| (spec, status, stage_started.elapsed()))
                .map_err(|error| format!("failed to wait for {label}: {error}"));
            let _ = sender.send(result);
        });
    }
    drop(sender);
    let mut failures = Vec::new();
    for _ in 0..stage_count {
        let (spec, status, elapsed) = receiver
            .recv()
            .map_err(|error| format!("workflow stage monitor failed: {error}"))??;
        eprintln!(
            "[finish:{}] status={}, elapsed={:.3}s",
            spec.label,
            status,
            elapsed.as_secs_f64()
        );
        if !status.success() {
            failures.push(format!("{} ({status})", spec.label));
        }
    }
    eprintln!(
        "[finish:parallel] stages={}, elapsed={:.3}s",
        stage_count,
        started.elapsed().as_secs_f64()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("workflow stages failed: {}", failures.join(", ")))
    }
}

fn run_one(spec: ProcessSpec, dry_run: bool) -> Result<(), String> {
    if dry_run {
        eprintln!("[dry-run:{}] {}", spec.label, spec.rendered());
        return Ok(());
    }
    eprintln!("[start:{}] {}", spec.label, spec.rendered());
    let started = Instant::now();
    let status = spec
        .command()
        .status()
        .map_err(|error| format!("failed to start {}: {error}", spec.label))?;
    eprintln!(
        "[finish:{}] status={}, elapsed={:.3}s",
        spec.label,
        status,
        started.elapsed().as_secs_f64()
    );
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed with {status}", spec.label))
    }
}

fn package_linux(root: &Path, options: &LinuxPackageOptions) -> Result<PathBuf, String> {
    let mut arguments = vec![OsString::from("scripts/packaging/build_packages.sh")];
    match options.format {
        LinuxPackageFormat::Layout => arguments.push(OsString::from("--layout-only")),
        format => {
            arguments.push(OsString::from("--format"));
            arguments.push(OsString::from(format.as_str()));
        }
    }
    if options.skip_build {
        arguments.push(OsString::from("--skip-build"));
    }
    if let Some(out_dir) = options.out_dir.as_deref() {
        arguments.push(OsString::from("--out-dir"));
        arguments.push(out_dir.as_os_str().to_owned());
    }
    run_one(
        ProcessSpec::new("linux-package", "bash", arguments, root),
        options.dry_run,
    )?;
    Ok(resolve_from_root(
        root,
        options
            .out_dir
            .as_deref()
            .unwrap_or_else(|| Path::new("target/packages")),
    ))
}

fn control_pipeline(root: &Path, options: &ControlOptions) -> Result<PathBuf, String> {
    let output = resolve_from_root(root, &options.output);
    if options.reuse {
        if !options.dry_run {
            let identity = validate_control_bundle(&output, env!("CARGO_PKG_VERSION"))?;
            eprintln!("[reuse:control-bundle] identity={identity}");
        }
    } else {
        package_control_bundle(root, options, &output)?;
    }
    if options.install {
        install_control_bundle(root, options, &output)?;
    }
    Ok(output)
}

fn package_control_bundle(
    root: &Path,
    options: &ControlOptions,
    output: &Path,
) -> Result<(), String> {
    let supported_host = env::consts::OS == "linux" && env::consts::ARCH == "x86_64";
    if !options.dry_run && !supported_host {
        return Err("control package/deploy currently requires a Linux x86_64 host".to_string());
    }
    let project = root.join("apps/leserpent/src/Leserpent/Leserpent.csproj");
    require_file(&project, "Leserpent control project", options.dry_run)?;
    for (path, label) in [
        (
            root.join("apps/leserpent/deploy/linux/install.sh"),
            "Leserpent Linux installer",
        ),
        (
            root.join("apps/leserpent/deploy/linux/leserpent.service"),
            "Leserpent systemd unit",
        ),
        (
            root.join("apps/leserpent/deploy/linux/leserpent.env.example"),
            "Leserpent environment template",
        ),
    ] {
        require_file(&path, label, options.dry_run)?;
    }

    let managed_root = root.join("target/dev-workflow/control");
    let dotnet_artifacts = managed_root.join("dotnet-artifacts");
    let pending = adjacent_temporary_path(output, "pending")?;
    if !options.dry_run {
        preflight_managed_output(
            output,
            &root.join("artifacts/leserpent/linux-x64"),
            &pending,
            "control bundle",
        )?;
        fs::create_dir_all(&managed_root).map_err(|error| error.to_string())?;
    }
    let _lock = if options.dry_run {
        None
    } else {
        Some(DirectoryLock::acquire(&managed_root.join("pipeline.lock"))?)
    };
    let target_root = cargo_target_root(root);
    let release_dir = target_root.join("release");
    let bridge = release_dir.join("leserpent-compat-bridge");
    let daemon = release_dir.join("leserpentd");
    run_parallel(
        vec![
            control_restore_spec(root, &dotnet_artifacts),
            control_native_payloads_spec(root),
        ],
        options.dry_run,
    )?;

    let mut pending_guard = None;
    if !options.dry_run {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        pending_guard = Some(PendingDirectory::new(&pending));
    }
    run_one(
        control_publish_spec(root, &project, &dotnet_artifacts, &pending),
        options.dry_run,
    )?;
    if options.dry_run {
        eprintln!(
            "[dry-run:control-bundle-finalize] copy Rust payloads, write {}, verify, and atomically publish {}",
            CONTROL_BUNDLE_SUMS,
            output.display()
        );
        return Ok(());
    }

    copy_regular_executable(&bridge, &pending.join("leserpent-compat-bridge"))?;
    copy_regular_executable(&daemon, &pending.join("leserpentd"))?;
    let deploy_source = root.join("apps/leserpent/deploy/linux");
    let deploy_output = pending.join("deploy");
    fs::create_dir_all(&deploy_output)
        .map_err(|error| format!("failed to create control deploy directory: {error}"))?;
    copy_regular_executable(
        &deploy_source.join("install.sh"),
        &deploy_output.join("install.sh"),
    )?;
    copy_regular_file(
        &deploy_source.join("leserpent.service"),
        &deploy_output.join("leserpent.service"),
    )?;
    copy_regular_file(
        &deploy_source.join("leserpent.env.example"),
        &deploy_output.join("leserpent.env.example"),
    )?;
    write_control_bundle_metadata(&pending, env!("CARGO_PKG_VERSION"))?;
    let identity = validate_control_bundle(&pending, env!("CARGO_PKG_VERSION"))?;
    atomic_replace_directory(&pending, output)?;
    pending_guard
        .as_mut()
        .expect("real control workflow owns a pending guard")
        .disarm();
    eprintln!("[publish:control-bundle] identity={identity}");
    Ok(())
}

fn control_restore_spec(root: &Path, artifacts: &Path) -> ProcessSpec {
    ProcessSpec::new(
        "control-aot-restore",
        "dotnet",
        [
            OsString::from("restore"),
            OsString::from("apps/leserpent/src/Leserpent/Leserpent.csproj"),
            OsString::from("-p:PublishProfile=native-aot"),
            OsString::from("-p:PublishAot=true"),
            OsString::from("-p:RuntimeIdentifier=linux-x64"),
            OsString::from("--locked-mode"),
            OsString::from("--artifacts-path"),
            artifacts.as_os_str().to_owned(),
        ],
        root,
    )
}

fn control_native_payloads_spec(root: &Path) -> ProcessSpec {
    ProcessSpec::new(
        "control-native-payloads",
        "cargo",
        [
            "build",
            "--locked",
            "--release",
            "-p",
            "leserpent-protocol",
            "-p",
            "leserpentd",
            "--bin",
            "leserpent-compat-bridge",
            "--bin",
            "leserpentd",
            "--features",
            "leserpentd/native-ssh",
        ],
        root,
    )
}

fn control_publish_spec(
    root: &Path,
    project: &Path,
    artifacts: &Path,
    output: &Path,
) -> ProcessSpec {
    ProcessSpec::new(
        "control-aot-publish",
        "dotnet",
        [
            OsString::from("publish"),
            project.as_os_str().to_owned(),
            OsString::from("-p:PublishProfile=native-aot"),
            OsString::from("-p:PublishAot=true"),
            OsString::from("-p:RuntimeIdentifier=linux-x64"),
            OsString::from("-p:SkipRustCompatibilityBridge=true"),
            OsString::from("--no-restore"),
            OsString::from("--artifacts-path"),
            artifacts.as_os_str().to_owned(),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ],
        root,
    )
}

fn install_control_bundle(
    root: &Path,
    options: &ControlOptions,
    output: &Path,
) -> Result<(), String> {
    if !options.dry_run {
        validate_control_bundle(output, env!("CARGO_PKG_VERSION"))?;
    }
    let installer = output.join("deploy/install.sh");
    let mut installer_arguments = vec![
        installer.as_os_str().to_owned(),
        OsString::from("--source"),
        output.as_os_str().to_owned(),
        OsString::from("--keep-releases"),
        OsString::from(options.keep_releases.to_string()),
    ];
    if options.no_start {
        installer_arguments.push(OsString::from("--no-start"));
    }
    let spec = if effective_user_is_root() {
        ProcessSpec::new("control-install", "bash", installer_arguments, root)
    } else {
        let mut arguments = vec![
            OsString::from("--non-interactive"),
            OsString::from("--"),
            OsString::from("bash"),
        ];
        arguments.extend(installer_arguments);
        ProcessSpec::new("control-install", "sudo", arguments, root)
    };
    run_one(spec, options.dry_run)
}

fn effective_user_is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"0\n")
}

#[derive(Debug)]
struct ControlBundleFile {
    path: String,
    bytes: u64,
    sha256: String,
}

fn write_control_bundle_metadata(bundle: &Path, version: &str) -> Result<String, String> {
    for name in [CONTROL_BUNDLE_MANIFEST, CONTROL_BUNDLE_SUMS] {
        match fs::symlink_metadata(bundle.join(name)) {
            Ok(_) => return Err(format!("control bundle metadata already exists: {name}")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "failed to inspect control bundle metadata: {error}"
                ));
            }
        }
    }
    let mut files =
        collect_control_bundle_files(bundle, &[CONTROL_BUNDLE_MANIFEST, CONTROL_BUNDLE_SUMS])?;
    if files.len() >= MAX_CONTROL_BUNDLE_FILES {
        return Err(format!(
            "control bundle leaves no inventory slot for {CONTROL_BUNDLE_MANIFEST}"
        ));
    }
    let payload_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or_else(|| "control bundle byte count overflowed".to_string())
    })?;
    let payload_files = files.len();
    let manifest_path = bundle.join(CONTROL_BUNDLE_MANIFEST);
    fs::write(
        &manifest_path,
        control_bundle_manifest(version, payload_files, payload_bytes),
    )
    .map_err(|error| format!("failed to write control bundle manifest: {error}"))?;

    let manifest_metadata = fs::symlink_metadata(&manifest_path)
        .map_err(|error| format!("failed to inspect control bundle manifest: {error}"))?;
    files.push(ControlBundleFile {
        path: CONTROL_BUNDLE_MANIFEST.to_string(),
        bytes: manifest_metadata.len(),
        sha256: sha256_file(&manifest_path)?,
    });
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let sums = control_bundle_sums(&files);
    fs::write(bundle.join(CONTROL_BUNDLE_SUMS), &sums)
        .map_err(|error| format!("failed to write control bundle checksums: {error}"))?;
    Ok(sha256_bytes(sums.as_bytes()))
}

fn validate_control_bundle(bundle: &Path, version: &str) -> Result<String, String> {
    require_directory(bundle, "control bundle")?;
    for path in [
        "Leserpent",
        "leserpent-compat-bridge",
        "leserpentd",
        "libe_sqlite3.so",
        "wwwroot/index.html",
        "deploy/install.sh",
        "deploy/leserpent.service",
        "deploy/leserpent.env.example",
        CONTROL_BUNDLE_MANIFEST,
        CONTROL_BUNDLE_SUMS,
    ] {
        require_file(&bundle.join(path), path, false)?;
    }
    require_directory(&bundle.join("wwwroot"), "control bundle wwwroot")?;
    for executable in ["Leserpent", "leserpent-compat-bridge", "leserpentd"] {
        require_elf_x86_64(&bundle.join(executable), executable)?;
        require_executable(&bundle.join(executable), executable)?;
    }
    require_elf_x86_64(&bundle.join("libe_sqlite3.so"), "libe_sqlite3.so")?;
    require_executable(&bundle.join("deploy/install.sh"), "deploy/install.sh")?;

    let files = collect_control_bundle_files(bundle, &[CONTROL_BUNDLE_SUMS])?;
    let payload = files
        .iter()
        .filter(|file| file.path != CONTROL_BUNDLE_MANIFEST)
        .collect::<Vec<_>>();
    let payload_bytes = payload.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or_else(|| "control bundle byte count overflowed".to_string())
    })?;
    let manifest = read_bounded_text(&bundle.join(CONTROL_BUNDLE_MANIFEST), 16 * 1024)?;
    let expected_manifest = control_bundle_manifest(version, payload.len(), payload_bytes);
    if manifest != expected_manifest {
        return Err("control bundle manifest does not match its payload".to_string());
    }

    let sums = read_bounded_text(&bundle.join(CONTROL_BUNDLE_SUMS), 1024 * 1024)?;
    if sums != control_bundle_sums(&files) {
        return Err("control bundle checksum inventory does not match its files".to_string());
    }
    Ok(sha256_bytes(sums.as_bytes()))
}

fn control_bundle_manifest(version: &str, payload_files: usize, payload_bytes: u64) -> String {
    format!(
        "schema_version = 1\nproduct = \"leserpent-control\"\nversion = \"{version}\"\nrid = \"{CONTROL_BUNDLE_RID}\"\nhash_algorithm = \"sha256\"\ninventory = \"{CONTROL_BUNDLE_SUMS}\"\npayload_files = {payload_files}\npayload_bytes = {payload_bytes}\n"
    )
}

fn control_bundle_sums(files: &[ControlBundleFile]) -> String {
    let mut body = String::new();
    for file in files {
        body.push_str(&file.sha256);
        body.push_str("  ");
        body.push_str(&file.path);
        body.push('\n');
    }
    body
}

fn collect_control_bundle_files(
    bundle: &Path,
    excluded: &[&str],
) -> Result<Vec<ControlBundleFile>, String> {
    require_directory(bundle, "control bundle")?;
    let mut pending = vec![bundle.to_path_buf()];
    let mut files = Vec::new();
    let mut total_bytes = 0_u64;
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| format!("failed to read control bundle: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read control bundle entry: {error}"))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "control bundle must not contain symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                pending.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err(format!(
                    "control bundle must contain only regular files and directories: {}",
                    path.display()
                ));
            }
            let relative = portable_bundle_path(bundle, &path)?;
            if excluded.iter().any(|candidate| relative == *candidate) {
                continue;
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| "control bundle byte count overflowed".to_string())?;
            if total_bytes > MAX_CONTROL_BUNDLE_BYTES {
                return Err(format!(
                    "control bundle exceeds the {}-byte limit",
                    MAX_CONTROL_BUNDLE_BYTES
                ));
            }
            files.push(ControlBundleFile {
                path: relative,
                bytes: metadata.len(),
                sha256: sha256_file(&path)?,
            });
            if files.len() > MAX_CONTROL_BUNDLE_FILES {
                return Err(format!(
                    "control bundle exceeds the {MAX_CONTROL_BUNDLE_FILES}-file limit"
                ));
            }
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn portable_bundle_path(bundle: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(bundle)
        .map_err(|_| "control bundle path escaped its root".to_string())?;
    let value = relative
        .to_str()
        .ok_or_else(|| "control bundle paths must be UTF-8".to_string())?
        .replace(std::path::MAIN_SEPARATOR, "/");
    if value.is_empty()
        || value.starts_with('/')
        || value.starts_with('-')
        || value.contains("//")
        || value.split('/').any(|component| {
            component.is_empty()
                || matches!(component, "." | "..")
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
    {
        return Err(format!("control bundle path is not portable: {value}"));
    }
    Ok(value)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
    let mut context = Context::new(&SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        context.update(&buffer[..read]);
    }
    Ok(hex(context.finish().as_ref()))
}

fn sha256_bytes(value: &[u8]) -> String {
    hex(digest(&SHA256, value).as_ref())
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_bounded_text(path: &Path, limit: u64) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > limit {
        return Err(format!(
            "bundle metadata is not a bounded regular file: {}",
            path.display()
        ));
    }
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn require_elf_x86_64(path: &Path, label: &str) -> Result<(), String> {
    let mut header = [0_u8; 20];
    File::open(path)
        .and_then(|mut file| std::io::Read::read_exact(&mut file, &mut header))
        .map_err(|error| format!("failed to inspect {label}: {error}"))?;
    if header[..4] != *b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || u16::from_le_bytes([header[18], header[19]]) != 62
    {
        return Err(format!("{label} is not a 64-bit x86 ELF payload"));
    }
    Ok(())
}

fn require_executable(path: &Path, label: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(path)
            .map_err(|error| format!("failed to inspect {label}: {error}"))?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err(format!("{label} is not executable"));
        }
    }
    Ok(())
}

fn copy_regular_executable(source: &Path, destination: &Path) -> Result<(), String> {
    require_regular_control_source(source)?;
    require_executable(source, "native control payload")?;
    copy_control_source(source, destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("failed to set native payload permissions: {error}"))?;
    }
    Ok(())
}

fn copy_regular_file(source: &Path, destination: &Path) -> Result<(), String> {
    require_regular_control_source(source)?;
    copy_control_source(source, destination)
}

fn require_regular_control_source(source: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("failed to inspect {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "control bundle source must be a regular non-symlink file: {}",
            source.display()
        ));
    }
    Ok(())
}

fn copy_control_source(source: &Path, destination: &Path) -> Result<(), String> {
    fs::copy(source, destination).map_err(|error| {
        format!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn desktop_pipeline(root: &Path, options: &DesktopOptions) -> Result<PathBuf, String> {
    let supported_host = env::consts::OS == "macos" && env::consts::ARCH == "aarch64";
    if !options.dry_run && !supported_host {
        return Err("desktop package/deploy currently requires a macOS arm64 host".to_string());
    }
    require_file(
        &root.join("apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj"),
        "Avalonia project",
        options.dry_run,
    )?;
    require_file(
        &root.join("assets/branding/leserpent-icon.icns"),
        "Leserpent icon",
        options.dry_run,
    )?;
    require_file(
        Path::new("/usr/bin/codesign"),
        "macOS code-signing tool",
        options.dry_run,
    )?;

    let managed_root = root.join("target/dev-workflow/desktop");
    let dotnet_artifacts = managed_root.join("dotnet-artifacts");
    let publish_dir = managed_root.join("publish/osx-arm64");
    let output = resolve_from_root(root, &options.output);
    let pending = adjacent_temporary_path(&output, "pending")?;
    if !options.dry_run {
        preflight_desktop_output(root, &output, &pending)?;
    }
    let target_root = cargo_target_root(root);
    let release_dir = target_root.join("release");
    let bundler = release_dir.join("gewyvern_leserpent_bundle");
    let installer = release_dir.join("gewyvern_leserpent_install");
    let release_tool = release_dir.join("gewyvern_leserpent_release");
    let daemon = release_dir.join("leserpentd");
    let _lock = if options.dry_run {
        None
    } else {
        fs::create_dir_all(&managed_root).map_err(|error| error.to_string())?;
        Some(DirectoryLock::acquire(&managed_root.join("pipeline.lock"))?)
    };

    let project = "apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj";
    let restore = ProcessSpec::new(
        "desktop-aot-restore",
        "dotnet",
        [
            OsString::from("restore"),
            OsString::from(project),
            OsString::from("-p:PublishProfile=NativeAot"),
            OsString::from("-p:PublishAot=true"),
            OsString::from("-p:RuntimeIdentifier=osx-arm64"),
            OsString::from("--locked-mode"),
            OsString::from("--artifacts-path"),
            dotnet_artifacts.as_os_str().to_owned(),
        ],
        root,
    );
    let native_tools = desktop_native_tools_spec(root, options.apple_release.is_some());
    run_parallel(vec![restore, native_tools], options.dry_run)?;

    if !options.dry_run {
        reset_managed_directory(&publish_dir, &managed_root)?;
    }
    run_one(
        ProcessSpec::new(
            "desktop-aot-publish",
            "dotnet",
            [
                OsString::from("publish"),
                OsString::from(project),
                OsString::from("-p:PublishProfile=NativeAot"),
                OsString::from("-p:PublishAot=true"),
                OsString::from("-p:RuntimeIdentifier=osx-arm64"),
                OsString::from("--no-restore"),
                OsString::from("--artifacts-path"),
                dotnet_artifacts.as_os_str().to_owned(),
                OsString::from("-o"),
                publish_dir.as_os_str().to_owned(),
            ],
            root,
        ),
        options.dry_run,
    )?;

    let mut pending_guard = None;
    if !options.dry_run {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        pending_guard = Some(PendingDirectory::new(&pending));
    }
    let mut bundle_arguments = vec![
        OsString::from("--publish-dir"),
        publish_dir.as_os_str().to_owned(),
        OsString::from("--daemon"),
        daemon.as_os_str().to_owned(),
        OsString::from("--output"),
        pending.as_os_str().to_owned(),
    ];
    if let Some(issuer) = options.silvortex_issuer.as_deref() {
        bundle_arguments.push(OsString::from("--silvortex-issuer"));
        bundle_arguments.push(OsString::from(issuer));
    }
    run_one(
        ProcessSpec::new(
            "desktop-bundle",
            bundler.as_os_str(),
            bundle_arguments,
            root,
        ),
        options.dry_run,
    )?;
    for spec in desktop_signing_specs(root, options, &pending, &release_tool) {
        run_one(spec, options.dry_run)?;
    }
    if !options.dry_run {
        atomic_replace_directory(&pending, &output)?;
        pending_guard
            .as_mut()
            .expect("real desktop workflow owns a pending guard")
            .disarm();
    }

    if options.install {
        run_one(
            ProcessSpec::new(
                "desktop-install",
                installer.as_os_str(),
                [
                    OsString::from("install"),
                    OsString::from("--app"),
                    output.as_os_str().to_owned(),
                ],
                root,
            ),
            options.dry_run,
        )?;
        if options.launch {
            let home = env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "HOME is required for --launch".to_string())?;
            run_one(
                ProcessSpec::new(
                    "desktop-launch",
                    "open",
                    [PathBuf::from(home).join("Applications/Leserpent.app")],
                    root,
                ),
                options.dry_run,
            )?;
        }
    }
    Ok(output)
}

fn desktop_native_tools_spec(root: &Path, apple_release: bool) -> ProcessSpec {
    let mut arguments = vec![
        OsString::from("build"),
        OsString::from("--locked"),
        OsString::from("--release"),
        OsString::from("-p"),
        OsString::from("gewyvern"),
        OsString::from("--bin"),
        OsString::from("gewyvern_leserpent_bundle"),
        OsString::from("--bin"),
        OsString::from("gewyvern_leserpent_install"),
    ];
    if apple_release {
        arguments.push(OsString::from("--bin"));
        arguments.push(OsString::from("gewyvern_leserpent_release"));
    }
    arguments.extend([
        OsString::from("-p"),
        OsString::from("leserpentd"),
        OsString::from("--bin"),
        OsString::from("leserpentd"),
        OsString::from("--features"),
        OsString::from("leserpentd/native-ssh"),
    ]);
    ProcessSpec::new("desktop-native-tools", "cargo", arguments, root)
}

fn desktop_signing_specs(
    root: &Path,
    options: &DesktopOptions,
    pending: &Path,
    release_tool: &Path,
) -> Vec<ProcessSpec> {
    let Some(release) = options.apple_release.as_ref() else {
        return vec![
            ProcessSpec::new(
                "desktop-adhoc-sign",
                "/usr/bin/codesign",
                [
                    OsString::from("--force"),
                    OsString::from("--deep"),
                    OsString::from("--sign"),
                    OsString::from("-"),
                    OsString::from("--timestamp=none"),
                    pending.as_os_str().to_owned(),
                ],
                root,
            ),
            ProcessSpec::new(
                "desktop-signature-verify",
                "/usr/bin/codesign",
                [
                    OsString::from("--verify"),
                    OsString::from("--deep"),
                    OsString::from("--strict"),
                    OsString::from("--verbose=2"),
                    pending.as_os_str().to_owned(),
                ],
                root,
            ),
        ];
    };

    let app = pending.as_os_str().to_owned();
    vec![
        ProcessSpec::new(
            "desktop-apple-release-preflight",
            release_tool.as_os_str(),
            [
                OsString::from("preflight"),
                OsString::from("--app"),
                app.clone(),
                OsString::from("--keychain-profile"),
                OsString::from(&release.notary_profile),
                OsString::from("--require-ready"),
            ],
            root,
        ),
        ProcessSpec::new(
            "desktop-developer-id-sign",
            release_tool.as_os_str(),
            [
                OsString::from("sign"),
                OsString::from("--app"),
                app.clone(),
                OsString::from("--identity"),
                OsString::from(&release.identity),
            ],
            root,
        ),
        ProcessSpec::new(
            "desktop-apple-notarize",
            release_tool.as_os_str(),
            [
                OsString::from("notarize"),
                OsString::from("--app"),
                app.clone(),
                OsString::from("--keychain-profile"),
                OsString::from(&release.notary_profile),
            ],
            root,
        ),
        ProcessSpec::new(
            "desktop-apple-release-verify",
            release_tool.as_os_str(),
            [OsString::from("verify"), OsString::from("--app"), app],
            root,
        ),
    ]
}

fn doctor(root: &Path) -> Result<(), String> {
    let probes = [
        ("cargo", true, command_works("cargo", "--version")),
        ("rustc", true, command_works("rustc", "--version")),
        ("dotnet", true, command_works("dotnet", "--version")),
        ("bash", false, command_works("bash", "--version")),
        ("python3", false, command_works("python3", "--version")),
        ("flock", false, command_works("flock", "--version")),
        ("codesign", false, Path::new("/usr/bin/codesign").is_file()),
        ("dpkg-deb", false, command_works("dpkg-deb", "--version")),
        ("rpmbuild", false, command_works("rpmbuild", "--version")),
    ];
    let mut missing = Vec::new();
    for (name, required, available) in probes {
        eprintln!(
            "tool={name} status={} requirement={}",
            if available { "ready" } else { "missing" },
            if required {
                "required"
            } else {
                "workflow-specific"
            }
        );
        if required && !available {
            missing.push(name);
        }
    }
    for (label, path) in [
        (
            "avalonia-project",
            root.join("apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj"),
        ),
        (
            "leserpent-icon",
            root.join("assets/branding/leserpent-icon.icns"),
        ),
        (
            "linux-packager",
            root.join("scripts/packaging/build_packages.sh"),
        ),
    ] {
        let available = path.is_file();
        eprintln!(
            "input={label} status={} path={}",
            if available { "ready" } else { "missing" },
            path.display()
        );
        if !available {
            missing.push(label);
        }
    }
    if command_works("flock", "--version") {
        eprintln!("package_lock=flock");
    } else {
        eprintln!("package_lock=portable-directory-fallback");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "required workflow inputs are missing: {}",
            missing.join(", ")
        ))
    }
}

fn command_works(program: &str, version_argument: &str) -> bool {
    Command::new(program)
        .arg(version_argument)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn require_file(path: &Path, label: &str, dry_run: bool) -> Result<(), String> {
    if dry_run || path.is_file() {
        Ok(())
    } else {
        Err(format!("{label} is missing: {}", path.display()))
    }
}

fn require_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        Err(format!("{label} must be a non-symlink directory"))
    } else {
        Ok(())
    }
}

fn resolve_from_root(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn cargo_target_root(root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| resolve_from_root(root, &path))
        .unwrap_or_else(|| root.join("target"))
}

fn reset_managed_directory(path: &Path, managed_root: &Path) -> Result<(), String> {
    if !path.starts_with(managed_root) || path == managed_root {
        return Err(format!(
            "refusing to reset unmanaged directory: {}",
            path.display()
        ));
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "managed output must be a non-symlink directory: {}",
                path.display()
            ));
        }
        fs::remove_dir_all(path).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(path).map_err(|error| error.to_string())
}

fn adjacent_temporary_path(output: &Path, role: &str) -> Result<PathBuf, String> {
    let parent = output
        .parent()
        .ok_or_else(|| "bundle output must have a parent directory".to_string())?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "bundle output must have a UTF-8 filename".to_string())?;
    if name.is_empty() || name == "." || name == ".." {
        return Err("bundle output must have a safe filename".to_string());
    }
    let pending_name = match (
        output.file_stem().and_then(|value| value.to_str()),
        output.extension().and_then(|value| value.to_str()),
    ) {
        (Some(stem), Some(extension)) if !stem.is_empty() && !extension.is_empty() => {
            format!(".{stem}.{role}-{}.{extension}", std::process::id())
        }
        _ => format!(".{name}.{role}-{}", std::process::id()),
    };
    Ok(parent.join(pending_name))
}

fn preflight_desktop_output(root: &Path, output: &Path, pending: &Path) -> Result<(), String> {
    preflight_managed_output(
        output,
        &root.join("artifacts/leserpent-avalonia/Leserpent.app"),
        pending,
        "desktop bundle",
    )
}

fn preflight_managed_output(
    output: &Path,
    managed_output: &Path,
    pending: &Path,
    label: &str,
) -> Result<(), String> {
    match fs::symlink_metadata(pending) {
        Ok(_) => {
            return Err(format!(
                "temporary {label} path already exists: {}",
                pending.display()
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("failed to inspect temporary {label} path: {error}")),
    }
    let existing = match fs::symlink_metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("failed to inspect {label} output: {error}")),
    };
    if existing.file_type().is_symlink() || !existing.is_dir() {
        return Err(format!(
            "existing {label} output must be a non-symlink directory: {}",
            output.display()
        ));
    }

    let output_identity = fs::canonicalize(output)
        .map_err(|error| format!("failed to resolve existing {label} output: {error}"))?;
    let managed_identity = match fs::canonicalize(managed_output) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "refusing to replace existing custom {label} output: {}; move it first or use the default managed output",
                output.display()
            ));
        }
        Err(error) => return Err(format!("failed to resolve managed {label} output: {error}")),
    };
    if output_identity != managed_identity {
        return Err(format!(
            "refusing to replace existing custom {label} output: {}; move it first or use the default managed output",
            output.display()
        ));
    }
    Ok(())
}

fn atomic_replace_directory(pending: &Path, output: &Path) -> Result<(), String> {
    let pending_metadata = fs::symlink_metadata(pending)
        .map_err(|error| format!("failed to inspect pending bundle: {error}"))?;
    if pending_metadata.file_type().is_symlink() || !pending_metadata.is_dir() {
        return Err("pending bundle must be a non-symlink directory".to_string());
    }
    let existing = match fs::symlink_metadata(output) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.to_string()),
    };
    let Some(existing) = existing else {
        return fs::rename(pending, output).map_err(|error| error.to_string());
    };
    if existing.file_type().is_symlink() || !existing.is_dir() {
        return Err(format!(
            "existing bundle output must be a non-symlink directory: {}",
            output.display()
        ));
    }
    let backup = adjacent_temporary_path(output, "previous")?;
    if fs::symlink_metadata(&backup).is_ok() {
        return Err(format!(
            "temporary bundle backup already exists: {}",
            backup.display()
        ));
    }
    fs::rename(output, &backup).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(pending, output) {
        let _ = fs::rename(&backup, output);
        return Err(format!("failed to publish new bundle: {error}"));
    }
    fs::remove_dir_all(&backup)
        .map_err(|error| format!("new bundle published but old artifact cleanup failed: {error}"))
}

struct DirectoryLock {
    path: PathBuf,
}

struct PendingDirectory {
    path: PathBuf,
    armed: bool,
}

impl PendingDirectory {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingDirectory {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                let _ = fs::remove_file(&self.path);
            }
            Ok(_) => {
                let _ = fs::remove_dir_all(&self.path);
            }
            Err(_) => {}
        }
    }
}

impl DirectoryLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        fs::create_dir(path).map_err(|error| {
            format!(
                "workflow lock is unavailable at {}: {error}; remove it only after confirming no workflow is active",
                path.display()
            )
        })?;
        if let Err(error) = fs::write(path.join("owner.txt"), std::process::id().to_string()) {
            let _ = fs::remove_dir(path);
            return Err(format!("failed to record workflow lock owner: {error}"));
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl Drop for DirectoryLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.path.join("owner.txt"));
        let _ = fs::remove_dir(&self.path);
    }
}

fn quote_argument(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_./:=+".contains(&byte))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, FileTimes};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn build_defaults_to_parallel_all_debug_workflow() {
        let Workflow::Build(options) = Workflow::parse(vec!["build".into()]).unwrap() else {
            panic!("expected build workflow");
        };
        assert_eq!(options.scope, BuildScope::All);
        assert!(!options.release);
        assert!(!options.restore);
        let specs = build_specs(Path::new("/repo"), &options);
        assert_eq!(specs.len(), 3);
        assert!(
            specs[0]
                .rendered()
                .contains("cargo build --locked --workspace")
        );
        assert!(
            specs[1]
                .rendered()
                .contains("apps/leserpent/src/Leserpent/Leserpent.csproj")
        );
        assert!(!specs[1].rendered().contains("Leserpent.SecurityTests"));
        assert!(specs[1].rendered().contains("RestoreLockedMode=true"));
        assert!(specs[2].rendered().contains("Leserpent.Avalonia.csproj"));
    }

    #[test]
    fn check_defaults_to_unlinked_parallel_workflow() {
        let Workflow::Check(options) = Workflow::parse(vec!["check".into()]).unwrap() else {
            panic!("expected check workflow");
        };
        assert_eq!(options.scope, BuildScope::All);
        assert!(!options.release);
        assert!(!options.restore);
        let specs = check_specs(Path::new("/repo"), &options);
        assert_eq!(specs.len(), 3);
        assert!(
            specs[0]
                .rendered()
                .contains("cargo check --locked --workspace")
        );
        assert!(specs[1].rendered().starts_with("dotnet build"));
        assert!(specs[2].rendered().starts_with("dotnet build"));
        assert!(Workflow::parse(vec!["check".into(), "--release".into()]).is_err());
    }

    #[test]
    fn parser_keeps_linux_control_and_desktop_routes_explicit() {
        let linux = Workflow::parse(vec![
            "package".into(),
            "linux".into(),
            "--format".into(),
            "layout".into(),
            "--skip-build".into(),
        ])
        .unwrap();
        assert_eq!(
            linux,
            Workflow::PackageLinux(LinuxPackageOptions {
                format: LinuxPackageFormat::Layout,
                skip_build: true,
                out_dir: None,
                dry_run: false,
            })
        );

        let control = Workflow::parse(vec!["package".into(), "control".into()]).unwrap();
        assert_eq!(
            control,
            Workflow::Control(ControlOptions {
                output: PathBuf::from("artifacts/leserpent/linux-x64"),
                install: false,
                reuse: false,
                no_start: false,
                keep_releases: 3,
                dry_run: false,
            })
        );
        let control = Workflow::parse(vec![
            "deploy".into(),
            "control".into(),
            "--reuse".into(),
            "--no-start".into(),
            "--keep-releases".into(),
            "5".into(),
        ])
        .unwrap();
        assert_eq!(
            control,
            Workflow::Control(ControlOptions {
                output: PathBuf::from("artifacts/leserpent/linux-x64"),
                install: true,
                reuse: true,
                no_start: true,
                keep_releases: 5,
                dry_run: false,
            })
        );
        assert!(
            Workflow::parse(vec!["package".into(), "control".into(), "--reuse".into()]).is_err()
        );

        let desktop =
            Workflow::parse(vec!["deploy".into(), "desktop".into(), "--launch".into()]).unwrap();
        let Workflow::Desktop(desktop) = desktop else {
            panic!("expected desktop workflow");
        };
        assert!(desktop.install);
        assert!(desktop.launch);
        assert!(desktop.apple_release.is_none());
        assert_eq!(
            desktop.output,
            PathBuf::from("artifacts/leserpent-avalonia/Leserpent.app")
        );
        assert!(
            Workflow::parse(vec!["package".into(), "desktop".into(), "--launch".into()]).is_err()
        );
    }

    #[test]
    fn atomic_bundle_replacement_publishes_complete_new_output() {
        let root = env::temp_dir().join(format!("gewyvern-dev-bundle-test-{}", std::process::id()));
        let output = root.join("Leserpent.app");
        let pending = adjacent_temporary_path(&output, "pending").unwrap();
        assert_eq!(
            pending.extension().and_then(|value| value.to_str()),
            Some("app")
        );
        let control_pending = adjacent_temporary_path(&root.join("linux-x64"), "pending").unwrap();
        assert!(
            control_pending
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.starts_with(".linux-x64.pending-"))
        );
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("old"), b"old").unwrap();
        fs::create_dir_all(&pending).unwrap();
        fs::write(pending.join("new"), b"new").unwrap();

        atomic_replace_directory(&pending, &output).unwrap();
        assert!(output.join("new").is_file());
        assert!(!output.join("old").exists());
        assert!(!pending.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_options_reject_non_app_outputs_and_non_https_issuers() {
        assert!(
            Workflow::parse(vec![
                "package".into(),
                "desktop".into(),
                "--output".into(),
                "artifact".into(),
            ])
            .is_err()
        );
        assert!(
            Workflow::parse(vec![
                "package".into(),
                "desktop".into(),
                "--silvortex-issuer".into(),
                "http://example.test/".into(),
            ])
            .is_err()
        );
    }

    #[test]
    fn desktop_apple_release_options_are_explicit_and_atomic() {
        let workflow = Workflow::parse(vec![
            "package".into(),
            "desktop".into(),
            "--identity".into(),
            "Developer ID Application: Team Silvortex (TEAM123)".into(),
            "--notary-profile".into(),
            "leserpent-notary".into(),
        ])
        .unwrap();
        let Workflow::Desktop(options) = workflow else {
            panic!("expected desktop workflow");
        };
        assert_eq!(
            options.apple_release,
            Some(AppleReleaseOptions {
                identity: "Developer ID Application: Team Silvortex (TEAM123)".into(),
                notary_profile: "leserpent-notary".into(),
            })
        );
        assert!(
            Workflow::parse(vec![
                "package".into(),
                "desktop".into(),
                "--identity".into(),
                "Developer ID Application: Team Silvortex (TEAM123)".into(),
            ])
            .unwrap_err()
            .contains("must be supplied together")
        );
        assert!(
            Workflow::parse(vec![
                "package".into(),
                "desktop".into(),
                "--identity".into(),
                "Apple Development: Team Silvortex (TEAM123)".into(),
                "--notary-profile".into(),
                "leserpent-notary".into(),
            ])
            .unwrap_err()
            .contains("Developer ID Application")
        );
    }

    #[test]
    fn desktop_apple_release_pipeline_uses_the_strict_native_gate() {
        let options = DesktopOptions {
            output: PathBuf::from("Leserpent.app"),
            silvortex_issuer: None,
            apple_release: Some(AppleReleaseOptions {
                identity: "Developer ID Application: Team Silvortex (TEAM123)".into(),
                notary_profile: "leserpent-notary".into(),
            }),
            install: false,
            launch: false,
            dry_run: true,
        };
        let specs = desktop_signing_specs(
            Path::new("/repo"),
            &options,
            Path::new("/repo/.Leserpent.pending.app"),
            Path::new("/repo/target/release/gewyvern_leserpent_release"),
        );
        assert_eq!(
            specs.iter().map(|spec| spec.label).collect::<Vec<_>>(),
            [
                "desktop-apple-release-preflight",
                "desktop-developer-id-sign",
                "desktop-apple-notarize",
                "desktop-apple-release-verify",
            ]
        );
        assert!(specs[0].rendered().contains("--require-ready"));
        assert!(specs[1].rendered().contains("Developer ID Application"));
        assert!(specs[2].rendered().contains("--keychain-profile"));
        assert!(
            !specs
                .iter()
                .any(|spec| spec.rendered().contains("--allow-adhoc"))
        );

        let release_build = desktop_native_tools_spec(Path::new("/repo"), true).rendered();
        assert!(release_build.contains("--bin gewyvern_leserpent_release"));
        let local_build = desktop_native_tools_spec(Path::new("/repo"), false).rendered();
        assert!(!local_build.contains("gewyvern_leserpent_release"));

        let local = DesktopOptions {
            output: PathBuf::from("Leserpent.app"),
            silvortex_issuer: None,
            apple_release: None,
            install: false,
            launch: false,
            dry_run: true,
        };
        let local_specs = desktop_signing_specs(
            Path::new("/repo"),
            &local,
            Path::new("/repo/.Leserpent.pending.app"),
            Path::new("/repo/target/release/gewyvern_leserpent_release"),
        );
        assert_eq!(
            local_specs
                .iter()
                .map(|spec| spec.label)
                .collect::<Vec<_>>(),
            ["desktop-adhoc-sign", "desktop-signature-verify"]
        );
    }

    #[test]
    fn dotnet_restore_freshness_tracks_project_inputs() {
        let root =
            env::temp_dir().join(format!("gewyvern-dev-restore-test-{}", std::process::id()));
        let project_dir = root.join("src/App");
        let project = project_dir.join("App.csproj");
        let lock = project_dir.join("packages.lock.json");
        let assets = project_dir.join("obj/project.assets.json");
        let nuget_config = root.join("NuGet.Config");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(assets.parent().unwrap()).unwrap();
        fs::write(&project, b"<Project />").unwrap();
        fs::write(&lock, b"{}").unwrap();
        fs::write(&assets, b"{}").unwrap();
        fs::write(&nuget_config, b"<configuration />").unwrap();

        let input_time = UNIX_EPOCH + Duration::from_secs(10);
        let assets_time = UNIX_EPOCH + Duration::from_secs(20);
        for input in [&project, &lock, &nuget_config] {
            File::options()
                .write(true)
                .open(input)
                .unwrap()
                .set_times(FileTimes::new().set_modified(input_time))
                .unwrap();
        }
        File::options()
            .write(true)
            .open(&assets)
            .unwrap()
            .set_times(FileTimes::new().set_modified(assets_time))
            .unwrap();
        assert!(dotnet_restore_is_fresh(&root, &["src/App/App.csproj"]));

        File::options()
            .write(true)
            .open(&nuget_config)
            .unwrap()
            .set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(30)))
            .unwrap();
        assert!(!dotnet_restore_is_fresh(&root, &["src/App/App.csproj"]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_desktop_stage_cleans_its_pending_bundle() {
        let root =
            env::temp_dir().join(format!("gewyvern-dev-pending-test-{}", std::process::id()));
        let pending = root.join(".Leserpent.pending-test.app");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&pending).unwrap();
        {
            let _guard = PendingDirectory::new(&pending);
        }
        assert!(!pending.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_output_preflight_replaces_only_the_managed_existing_bundle() {
        let root = env::temp_dir().join(format!("gewyvern-dev-output-test-{}", std::process::id()));
        let managed = root.join("artifacts/leserpent-avalonia/Leserpent.app");
        let custom = root.join("custom/Leserpent.app");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&managed).unwrap();
        fs::create_dir_all(&custom).unwrap();

        let managed_pending = adjacent_temporary_path(&managed, "pending").unwrap();
        preflight_desktop_output(&root, &managed, &managed_pending).unwrap();
        let custom_pending = adjacent_temporary_path(&custom, "pending").unwrap();
        assert!(
            preflight_desktop_output(&root, &custom, &custom_pending)
                .unwrap_err()
                .contains("refusing to replace existing custom desktop bundle output")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_pipeline_specs_restore_and_build_once_before_publish() {
        let root = Path::new("/repo");
        let artifacts = Path::new("/repo/target/dev-workflow/control/dotnet-artifacts");
        let restore = control_restore_spec(root, artifacts).rendered();
        assert!(restore.contains("dotnet restore"));
        assert!(restore.contains("--locked-mode"));
        assert!(restore.contains("PublishProfile=native-aot"));
        assert!(restore.contains("RuntimeIdentifier=linux-x64"));

        let native = control_native_payloads_spec(root).rendered();
        assert_eq!(native.matches("cargo build").count(), 1);
        assert!(native.contains("-p leserpent-protocol -p leserpentd"));
        assert!(native.contains("--bin leserpent-compat-bridge --bin leserpentd"));
        assert!(native.contains("leserpentd/native-ssh"));

        let publish = control_publish_spec(
            root,
            Path::new("/repo/apps/leserpent/src/Leserpent/Leserpent.csproj"),
            artifacts,
            Path::new("/repo/artifacts/leserpent/.linux-x64.pending"),
        )
        .rendered();
        assert!(publish.contains("dotnet publish"));
        assert!(publish.contains("PublishProfile=native-aot"));
        assert!(publish.contains("SkipRustCompatibilityBridge=true"));
        assert!(publish.contains("--no-restore"));
    }

    #[cfg(unix)]
    #[test]
    fn control_bundle_metadata_rejects_tampering_and_inventory_drift() {
        use std::os::unix::fs::PermissionsExt;

        let root = env::temp_dir().join(format!(
            "gewyvern-dev-control-bundle-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("deploy")).unwrap();
        fs::create_dir_all(root.join("wwwroot")).unwrap();

        let mut elf = vec![0_u8; 64];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18..20].copy_from_slice(&62_u16.to_le_bytes());
        for executable in ["Leserpent", "leserpent-compat-bridge", "leserpentd"] {
            let path = root.join(executable);
            fs::write(&path, &elf).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(root.join("libe_sqlite3.so"), &elf).unwrap();
        fs::write(root.join("wwwroot/index.html"), b"control").unwrap();
        fs::write(root.join("deploy/leserpent.service"), b"service").unwrap();
        fs::write(root.join("deploy/leserpent.env.example"), b"env").unwrap();
        let installer = root.join("deploy/install.sh");
        fs::write(&installer, b"#!/usr/bin/env bash\n").unwrap();
        fs::set_permissions(&installer, fs::Permissions::from_mode(0o755)).unwrap();

        let identity = write_control_bundle_metadata(&root, "1.20.0").unwrap();
        assert_eq!(identity.len(), 64);
        assert_eq!(validate_control_bundle(&root, "1.20.0").unwrap(), identity);

        fs::write(root.join("wwwroot/index.html"), b"changed").unwrap();
        assert!(
            validate_control_bundle(&root, "1.20.0")
                .unwrap_err()
                .contains("checksum inventory does not match its files")
        );
        fs::write(root.join("wwwroot/index.html"), b"control").unwrap();
        assert_eq!(validate_control_bundle(&root, "1.20.0").unwrap(), identity);

        fs::write(root.join("wwwroot/untracked.js"), b"extra").unwrap();
        assert!(
            validate_control_bundle(&root, "1.20.0")
                .unwrap_err()
                .contains("manifest does not match its payload")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
