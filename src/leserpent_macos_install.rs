use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

use ring::digest::{SHA256, digest};
use serde::Serialize;

use crate::native_binary::file_is_mach_o_arm64;

const APP_NAME: &str = "Leserpent.app";
const EXECUTABLE: &str = "Leserpent.Avalonia";
const MAX_BUNDLE_FILES: usize = 256;
const MAX_BUNDLE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallAction {
    Install,
    Rollback,
    Status,
}

#[derive(Debug)]
pub struct InstallOptions {
    pub action: InstallAction,
    pub app: Option<PathBuf>,
    pub root: PathBuf,
    pub launcher: PathBuf,
    pub keep_releases: usize,
}

impl InstallOptions {
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args.peekable();
        let action = match args.next().as_deref() {
            Some("install") => InstallAction::Install,
            Some("rollback") => InstallAction::Rollback,
            Some("status") => InstallAction::Status,
            Some("--help" | "-h") | None => return Err(usage().to_string()),
            Some(value) => return Err(format!("unknown action `{value}`\n{}", usage())),
        };
        let home = env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                "HOME is required unless --root and --launcher are supplied".to_string()
            })?;
        let mut app = None;
        let mut root = home.join("Library/Application Support/Leserpent/Installer");
        let mut launcher = home.join("Applications").join(APP_NAME);
        let mut keep_releases = 3usize;
        while let Some(argument) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("{argument} requires a value"))?;
            match argument.as_str() {
                "--app" => app = Some(PathBuf::from(value)),
                "--root" => root = PathBuf::from(value),
                "--launcher" => launcher = PathBuf::from(value),
                "--keep-releases" => {
                    keep_releases = value
                        .parse()
                        .map_err(|_| "--keep-releases must be an integer".to_string())?;
                }
                _ => return Err(format!("unknown option `{argument}`\n{}", usage())),
            }
        }
        if action == InstallAction::Install && app.is_none() {
            return Err("install requires --app".to_string());
        }
        if action != InstallAction::Install && app.is_some() {
            return Err("--app is valid only for install".to_string());
        }
        if keep_releases < 2 {
            return Err("--keep-releases must be at least 2".to_string());
        }
        if !root.is_absolute() || !launcher.is_absolute() {
            return Err("--root and --launcher must be absolute paths".to_string());
        }
        if !safe_absolute_path(&root) || !safe_absolute_path(&launcher) {
            return Err("--root and --launcher cannot contain parent traversal".to_string());
        }
        if launcher.file_name().and_then(|value| value.to_str()) != Some(APP_NAME) {
            return Err(format!("--launcher must end in {APP_NAME}"));
        }
        Ok(Self {
            action,
            app,
            root,
            launcher,
            keep_releases,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct InstallReport {
    action: &'static str,
    current: Option<String>,
    previous: Option<String>,
    launcher: String,
    result: &'static str,
}

impl InstallReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("fixed install report must serialize")
    }
}

pub fn execute(options: &InstallOptions) -> Result<InstallReport, String> {
    if env::consts::OS != "macos" {
        return Err("this installer supports macOS only".to_string());
    }
    execute_on_supported_host(options)
}

fn execute_on_supported_host(options: &InstallOptions) -> Result<InstallReport, String> {
    match options.action {
        InstallAction::Install => install(options)?,
        InstallAction::Rollback => rollback(options)?,
        InstallAction::Status => validate_status(options)?,
    }
    report(options)
}

fn install(options: &InstallOptions) -> Result<(), String> {
    let source = options.app.as_deref().expect("install app was parsed");
    let source = fs::canonicalize(source).map_err(|error| error.to_string())?;
    if source.starts_with(&options.root) {
        return Err("source application cannot be inside the install root".to_string());
    }
    let identity = validate_bundle(&source)?;
    fs::create_dir_all(options.root.join("releases")).map_err(|error| error.to_string())?;
    require_directory(&options.root, "install root")?;
    require_directory(&options.root.join("releases"), "release root")?;
    validate_launcher(options)?;
    let old_current = release_link(&options.root, "current")?;
    let old_previous = release_link(&options.root, "previous")?;
    if old_current.is_none() && old_previous.is_some() {
        return Err("installer state has previous without current".to_string());
    }
    let release_id = format!("{}-{}", identity.version, identity.executable_hash);
    let release_target = PathBuf::from("releases").join(&release_id);
    let release_dir = options.root.join(&release_target);
    let installed_app = release_dir.join(APP_NAME);
    if release_dir.exists() {
        let installed = validate_bundle(&installed_app)?;
        if installed != identity {
            return Err("existing release identity does not match its release id".to_string());
        }
    } else {
        let pending = options.root.join(format!(
            ".release-{release_id}.pending-{}",
            std::process::id()
        ));
        if pending.exists() {
            return Err("pending release path already exists".to_string());
        }
        fs::create_dir(&pending).map_err(|error| error.to_string())?;
        let copied = copy_bundle(&source, &pending.join(APP_NAME))
            .and_then(|_| validate_bundle(&pending.join(APP_NAME)).map(|_| ()))
            .and_then(|_| fs::rename(&pending, &release_dir).map_err(|error| error.to_string()));
        if let Err(error) = copied {
            let _ = fs::remove_dir_all(&pending);
            return Err(error);
        }
    }
    if old_current.as_ref() == Some(&release_target) {
        ensure_launcher(options)?;
        prune_releases(options)?;
        return Ok(());
    }

    if let Some(current) = &old_current {
        replace_link(&options.root, "previous", current)?;
    }
    if let Err(error) = replace_link(&options.root, "current", &release_target)
        .and_then(|_| ensure_launcher(options))
    {
        restore_link(&options.root, "current", old_current.as_deref());
        restore_link(&options.root, "previous", old_previous.as_deref());
        return Err(error);
    }
    prune_releases(options)
}

fn rollback(options: &InstallOptions) -> Result<(), String> {
    validate_launcher(options)?;
    let current = release_link(&options.root, "current")?
        .ok_or_else(|| "cannot rollback: current release link is missing".to_string())?;
    let previous = release_link(&options.root, "previous")?
        .ok_or_else(|| "cannot rollback: previous release link is missing".to_string())?;
    if current == previous {
        return Err("cannot rollback: current and previous releases are identical".to_string());
    }
    replace_link(&options.root, "previous", &current)?;
    if let Err(error) =
        replace_link(&options.root, "current", &previous).and_then(|_| ensure_launcher(options))
    {
        restore_link(&options.root, "current", Some(&current));
        restore_link(&options.root, "previous", Some(&previous));
        return Err(error);
    }
    Ok(())
}

fn report(options: &InstallOptions) -> Result<InstallReport, String> {
    let current = release_link(&options.root, "current")?;
    let previous = release_link(&options.root, "previous")?;
    Ok(InstallReport {
        action: match options.action {
            InstallAction::Install => "install",
            InstallAction::Rollback => "rollback",
            InstallAction::Status => "status",
        },
        current: current.map(|path| path.to_string_lossy().into_owned()),
        previous: previous.map(|path| path.to_string_lossy().into_owned()),
        launcher: options.launcher.display().to_string(),
        result: "passed",
    })
}

#[derive(Debug, PartialEq, Eq)]
struct BundleIdentity {
    version: String,
    executable_hash: String,
}

fn validate_bundle(app: &Path) -> Result<BundleIdentity, String> {
    require_directory(app, "application bundle")?;
    let (files, bytes) = inspect_tree(app)?;
    if files == 0 || files > MAX_BUNDLE_FILES || bytes > MAX_BUNDLE_BYTES {
        return Err("application bundle exceeds its file or byte limit".to_string());
    }
    let executable = app.join("Contents/MacOS").join(EXECUTABLE);
    require_file(&executable, "application executable")?;
    if !file_is_mach_o_arm64(&executable)
        .map_err(|error| format!("failed to inspect application executable: {error}"))?
    {
        return Err("application executable is not an arm64 Mach-O".to_string());
    }
    require_file(
        &app.join("Contents/Resources/leserpent.icns"),
        "application icon",
    )?;
    let plist = fs::read_to_string(app.join("Contents/Info.plist"))
        .map_err(|error| format!("Info.plist is unavailable: {error}"))?;
    if plist_string(&plist, "CFBundleIdentifier")? != "org.gewyvern.leserpent" {
        return Err("Info.plist bundle identifier is invalid".to_string());
    }
    if plist_string(&plist, "CFBundleExecutable")? != EXECUTABLE {
        return Err("Info.plist executable is invalid".to_string());
    }
    let version = plist_string(&plist, "CFBundleShortVersionString")?;
    if plist_string(&plist, "CFBundleVersion")? != version || !safe_version(&version) {
        return Err("Info.plist versions are invalid or inconsistent".to_string());
    }
    let executable_bytes = fs::read(&executable).map_err(|error| error.to_string())?;
    let hash = digest(&SHA256, &executable_bytes);
    let executable_hash = hash
        .as_ref()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(BundleIdentity {
        version,
        executable_hash,
    })
}

fn inspect_tree(root: &Path) -> Result<(usize, u64), String> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = 0usize;
    let mut bytes = 0u64;
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "application bundle contains a symbolic link: {}",
                    entry.path().display()
                ));
            }
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                files += 1;
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "application bundle byte count overflowed".to_string())?;
            } else {
                return Err("application bundle contains a special file".to_string());
            }
            if files > MAX_BUNDLE_FILES || bytes > MAX_BUNDLE_BYTES {
                return Err("application bundle exceeds its file or byte limit".to_string());
            }
        }
    }
    Ok((files, bytes))
}

fn copy_bundle(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            copy_bundle(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?;
            sanitize_permissions(&destination_path, &metadata)?;
        } else {
            return Err("application bundle changed during copy".to_string());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn sanitize_permissions(path: &Path, source: &fs::Metadata) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = source.permissions().mode() & !0o022;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn sanitize_permissions(_path: &Path, _source: &fs::Metadata) -> Result<(), String> {
    Ok(())
}

fn release_link(root: &Path, name: &str) -> Result<Option<PathBuf>, String> {
    let link = root.join(name);
    let metadata = match fs::symlink_metadata(&link) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return Err(format!("{name} release link is not a symbolic link"));
    }
    let target = fs::read_link(&link).map_err(|error| error.to_string())?;
    let parts = target.components().collect::<Vec<_>>();
    if parts.len() != 2
        || parts[0] != Component::Normal("releases".as_ref())
        || !matches!(parts[1], Component::Normal(value) if safe_release_id(value.to_string_lossy().as_ref()))
        || !root.join(&target).is_dir()
    {
        return Err(format!("{name} release link is missing or unsafe"));
    }
    let identity = validate_bundle(&root.join(&target).join(APP_NAME))?;
    let expected_id = format!("{}-{}", identity.version, identity.executable_hash);
    if target.file_name().and_then(|value| value.to_str()) != Some(expected_id.as_str()) {
        return Err(format!(
            "{name} release identity does not match its directory"
        ));
    }
    Ok(Some(target))
}

#[cfg(unix)]
fn replace_link(root: &Path, name: &str, target: &Path) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    let pending = root.join(format!(".{name}.pending-{}", std::process::id()));
    if fs::symlink_metadata(&pending).is_ok() {
        return Err(format!("pending {name} link already exists"));
    }
    symlink(target, &pending).map_err(|error| error.to_string())?;
    if let Err(error) = fs::rename(&pending, root.join(name)) {
        let _ = fs::remove_file(&pending);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(not(unix))]
fn replace_link(_root: &Path, _name: &str, _target: &Path) -> Result<(), String> {
    Err("symbolic-link installation requires Unix".to_string())
}

fn restore_link(root: &Path, name: &str, target: Option<&Path>) {
    if let Some(target) = target {
        let _ = replace_link(root, name, target);
    } else if fs::symlink_metadata(root.join(name))
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        let _ = fs::remove_file(root.join(name));
    }
}

fn validate_launcher(options: &InstallOptions) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(&options.launcher) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.file_type().is_symlink() {
        return Err("launcher exists and is not a managed symbolic link".to_string());
    }
    let expected = options.root.join("current").join(APP_NAME);
    if fs::read_link(&options.launcher).map_err(|error| error.to_string())? != expected {
        return Err("launcher symbolic link has an unexpected target".to_string());
    }
    Ok(())
}

fn validate_status(options: &InstallOptions) -> Result<(), String> {
    release_link(&options.root, "current")?
        .ok_or_else(|| "current release link is missing".to_string())?;
    validate_launcher(options)?;
    if !options.launcher.is_symlink() {
        return Err("managed application launcher is missing".to_string());
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_launcher(options: &InstallOptions) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    validate_launcher(options)?;
    let parent = options
        .launcher
        .parent()
        .ok_or_else(|| "launcher has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let expected = options.root.join("current").join(APP_NAME);
    if options.launcher.is_symlink() {
        return Ok(());
    }
    let pending = parent.join(format!(".Leserpent.app.pending-{}", std::process::id()));
    symlink(expected, &pending).map_err(|error| error.to_string())?;
    fs::rename(&pending, &options.launcher).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn ensure_launcher(_options: &InstallOptions) -> Result<(), String> {
    Err("application launcher requires Unix".to_string())
}

fn prune_releases(options: &InstallOptions) -> Result<(), String> {
    let current = release_link(&options.root, "current")?;
    let previous = release_link(&options.root, "previous")?;
    let protected = [current, previous]
        .into_iter()
        .flatten()
        .filter_map(|path| path.file_name().map(|value| value.to_owned()))
        .collect::<BTreeSet<_>>();
    let releases = options.root.join("releases");
    let mut entries = Vec::new();
    for entry in fs::read_dir(&releases).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let name = entry.file_name();
        if !safe_release_id(name.to_string_lossy().as_ref()) {
            return Err("release directory contains an unsafe name".to_string());
        }
        require_directory(&path, "retained release")?;
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .map_err(|error| error.to_string())?;
        entries.push((modified, path));
    }
    entries.sort_by(|left, right| right.cmp(left));
    for (_, path) in entries.into_iter().skip(options.keep_releases) {
        if path
            .file_name()
            .is_some_and(|name| !protected.contains(name))
        {
            fs::remove_dir_all(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn plist_string(plist: &str, key: &str) -> Result<String, String> {
    let marker = format!("<key>{key}</key>");
    if plist.matches(&marker).count() != 1 {
        return Err(format!("Info.plist must contain exactly one {key}"));
    }
    let value = plist
        .split_once(&marker)
        .map(|(_, rest)| rest.trim_start())
        .and_then(|rest| rest.strip_prefix("<string>"))
        .and_then(|rest| rest.split_once("</string>").map(|(value, _)| value))
        .ok_or_else(|| format!("Info.plist {key} must be a string"))?;
    if value.contains('<') || value.contains('>') || value.is_empty() {
        return Err(format!("Info.plist {key} contains invalid markup"));
    }
    Ok(value.to_string())
}

fn safe_version(version: &str) -> bool {
    let segments = version.split('.').collect::<Vec<_>>();
    (1..=3).contains(&segments.len())
        && segments
            .iter()
            .all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_digit()))
}

fn safe_release_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn safe_absolute_path(path: &Path) -> bool {
    path.components()
        .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn require_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a non-symlink directory"));
    }
    Ok(())
}

fn require_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    Ok(())
}

fn usage() -> &'static str {
    "usage:\n  gewyvern_leserpent_install install --app Leserpent.app [--root DIR] [--launcher PATH] [--keep-releases N]\n  gewyvern_leserpent_install rollback [--root DIR] [--launcher PATH]\n  gewyvern_leserpent_install status [--root DIR] [--launcher PATH]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn fixture_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "leserpent-macos-install-{label}-{}",
            std::process::id()
        ))
    }

    fn fixture_app(path: &Path, version: &str, marker: u8) {
        let executable_dir = path.join("Contents/MacOS");
        fs::create_dir_all(&executable_dir).unwrap();
        fs::create_dir_all(path.join("Contents/Resources")).unwrap();
        let mut executable = b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01".to_vec();
        executable.push(marker);
        let executable_path = executable_dir.join(EXECUTABLE);
        fs::write(&executable_path, executable).unwrap();
        fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(path.join("Contents/Resources/leserpent.icns"), b"icon").unwrap();
        fs::write(
            path.join("Contents/Info.plist"),
            format!(
                "<plist><dict><key>CFBundleIdentifier</key><string>org.gewyvern.leserpent</string><key>CFBundleExecutable</key><string>{EXECUTABLE}</string><key>CFBundleShortVersionString</key><string>{version}</string><key>CFBundleVersion</key><string>{version}</string></dict></plist>"
            ),
        )
        .unwrap();
    }

    fn options(root: &Path, app: Option<PathBuf>, action: InstallAction) -> InstallOptions {
        InstallOptions {
            action,
            app,
            root: root.join("data/Installer"),
            launcher: root.join("Applications/Leserpent.app"),
            keep_releases: 3,
        }
    }

    #[test]
    fn install_upgrade_and_rollback_preserve_external_state() {
        let root = fixture_root("roundtrip");
        let first = root.join("first.app");
        let second = root.join("second.app");
        fixture_app(&first, "1.4.0", 1);
        fixture_app(&second, "1.4.1", 2);
        fs::create_dir_all(root.join("data")).unwrap();
        fs::write(root.join("data/profile.json"), b"preserved").unwrap();

        execute_on_supported_host(&options(&root, Some(first), InstallAction::Install)).unwrap();
        let first_target = release_link(&root.join("data/Installer"), "current")
            .unwrap()
            .unwrap();
        execute_on_supported_host(&options(&root, Some(second), InstallAction::Install)).unwrap();
        let second_target = release_link(&root.join("data/Installer"), "current")
            .unwrap()
            .unwrap();
        assert_ne!(first_target, second_target);
        assert_eq!(
            release_link(&root.join("data/Installer"), "previous").unwrap(),
            Some(first_target.clone())
        );
        execute_on_supported_host(&options(&root, None, InstallAction::Rollback)).unwrap();
        assert_eq!(
            release_link(&root.join("data/Installer"), "current").unwrap(),
            Some(first_target)
        );
        assert_eq!(
            fs::read(root.join("data/profile.json")).unwrap(),
            b"preserved"
        );
        assert!(root.join("Applications/Leserpent.app").is_symlink());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unsafe_release_and_bundle_links() {
        let root = fixture_root("unsafe");
        let app = root.join("source.app");
        fixture_app(&app, "1.4.0", 1);
        symlink("/tmp", app.join("Contents/Resources/escape")).unwrap();
        assert!(
            execute_on_supported_host(&options(&root, Some(app), InstallAction::Install))
                .unwrap_err()
                .contains("symbolic link")
        );

        let install_root = root.join("data/Installer");
        fs::create_dir_all(&install_root).unwrap();
        symlink("../../outside", install_root.join("current")).unwrap();
        assert!(
            release_link(&install_root, "current")
                .unwrap_err()
                .contains("unsafe")
        );
        fs::remove_file(install_root.join("current")).unwrap();
        fs::create_dir(install_root.join("releases")).unwrap();
        symlink("/tmp", install_root.join("releases/unsafe-release")).unwrap();
        assert!(
            prune_releases(&options(&root, None, InstallAction::Status))
                .unwrap_err()
                .contains("non-symlink")
        );
        fs::remove_file(install_root.join("releases/unsafe-release")).unwrap();
        fixture_app(
            &install_root.join("releases/wrong-release/Leserpent.app"),
            "1.4.0",
            1,
        );
        symlink("releases/wrong-release", install_root.join("current")).unwrap();
        assert!(
            release_link(&install_root, "current")
                .unwrap_err()
                .contains("identity")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn options_are_bounded_and_action_specific() {
        assert!(
            InstallOptions::parse(
                [
                    "install",
                    "--app",
                    "/tmp/Leserpent.app",
                    "--keep-releases",
                    "1"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_err()
        );
        assert!(
            InstallOptions::parse(
                ["rollback", "--app", "/tmp/Leserpent.app"]
                    .into_iter()
                    .map(str::to_string)
            )
            .is_err()
        );
    }
}
