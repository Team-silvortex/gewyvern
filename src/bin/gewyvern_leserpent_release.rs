use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};

use gewyvern::leserpent_account_config::{SILVORTEX_ISSUER_KEY, is_canonical_https_origin};
use gewyvern::native_binary::file_is_mach_o_arm64;
use ring::digest::{SHA256, digest};
use serde_json::json;

const BUNDLE_ID: &str = "org.gewyvern.leserpent";
const EXECUTABLE: &str = "Leserpent.Avalonia";
const DAEMON_EXECUTABLE: &str = "leserpentd";
const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_ENTITLEMENTS: &str = "assets/packaging/leserpent-macos.entitlements";

fn main() {
    match Options::parse(env::args().skip(1)).and_then(run) {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("Leserpent macOS release failed: {error}");
            process::exit(1);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Preflight,
    Sign,
    Notarize,
    Verify,
}

#[derive(Debug)]
struct Options {
    action: Action,
    app: PathBuf,
    identity: Option<String>,
    keychain_profile: Option<String>,
    entitlements: PathBuf,
    custom_entitlements: bool,
    allow_adhoc: bool,
    require_ready: bool,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args.peekable();
        let action = match args.next().as_deref() {
            Some("preflight") => Action::Preflight,
            Some("sign") => Action::Sign,
            Some("notarize") => Action::Notarize,
            Some("verify") => Action::Verify,
            Some("--help" | "-h") | None => return Err(usage().to_string()),
            Some(value) => return Err(format!("unknown action `{value}`\n{}", usage())),
        };
        let mut app = None;
        let mut identity = None;
        let mut keychain_profile = None;
        let mut entitlements = PathBuf::from(DEFAULT_ENTITLEMENTS);
        let mut custom_entitlements = false;
        let mut allow_adhoc = false;
        let mut require_ready = false;
        while let Some(argument) = args.next() {
            let value = match argument.as_str() {
                "--app" | "--identity" | "--keychain-profile" | "--entitlements" => args
                    .next()
                    .ok_or_else(|| format!("{argument} requires a value"))?,
                "--allow-adhoc" => {
                    allow_adhoc = true;
                    continue;
                }
                "--require-ready" => {
                    require_ready = true;
                    continue;
                }
                _ => return Err(format!("unknown argument `{argument}`")),
            };
            match argument.as_str() {
                "--app" => app = Some(PathBuf::from(value)),
                "--identity" => identity = Some(value),
                "--keychain-profile" => keychain_profile = Some(value),
                "--entitlements" => {
                    entitlements = PathBuf::from(value);
                    custom_entitlements = true;
                }
                _ => unreachable!(),
            }
        }
        let options = Self {
            action,
            app: app.ok_or_else(|| "--app is required".to_string())?,
            identity,
            keychain_profile,
            entitlements,
            custom_entitlements,
            allow_adhoc,
            require_ready,
        };
        options.validate()?;
        Ok(options)
    }

    fn validate(&self) -> Result<(), String> {
        match self.action {
            Action::Preflight => {
                if self.identity.is_some() || self.allow_adhoc {
                    return Err("preflight received an option for another action".to_string());
                }
            }
            Action::Sign => {
                let identity = self
                    .identity
                    .as_deref()
                    .ok_or_else(|| "sign requires --identity".to_string())?;
                if !identity.starts_with("Developer ID Application:") {
                    return Err(
                        "signing identity must be a Developer ID Application certificate"
                            .to_string(),
                    );
                }
                validate_opaque(identity, "identity")?;
                if self.keychain_profile.is_some() || self.allow_adhoc || self.require_ready {
                    return Err("sign received an option for another action".to_string());
                }
            }
            Action::Notarize => {
                let profile = self
                    .keychain_profile
                    .as_deref()
                    .ok_or_else(|| "notarize requires --keychain-profile".to_string())?;
                validate_opaque(profile, "keychain profile")?;
                if self.identity.is_some()
                    || self.allow_adhoc
                    || self.custom_entitlements
                    || self.require_ready
                {
                    return Err("notarize received an option for another action".to_string());
                }
            }
            Action::Verify => {
                if self.identity.is_some()
                    || self.keychain_profile.is_some()
                    || self.custom_entitlements
                    || self.require_ready
                {
                    return Err("verify received an option for another action".to_string());
                }
            }
        }
        Ok(())
    }
}

fn run(options: Options) -> Result<String, String> {
    if !cfg!(target_os = "macos") {
        return Err("macOS release operations require a macOS host".to_string());
    }
    validate_app(&options.app)?;
    match options.action {
        Action::Preflight => preflight(&options),
        Action::Sign => sign(&options),
        Action::Notarize => notarize(&options),
        Action::Verify => {
            verify_signature(&options.app, options.allow_adhoc)?;
            if !options.allow_adhoc {
                gatekeeper_assess(&options.app)?;
            }
            let formal_claims = formal_release_claims(options.allow_adhoc);
            Ok(format!(
                "Leserpent macOS release valid: action=verify, app={}, developer_id={}, gatekeeper={}, runtime_launch={}",
                options.app.display(),
                formal_claims,
                formal_claims,
                formal_claims
            ))
        }
    }
}

fn preflight(options: &Options) -> Result<String, String> {
    validate_regular_file(&options.entitlements, "entitlements")?;
    run_checked(
        Command::new("plutil")
            .arg("-lint")
            .arg(&options.entitlements),
        "entitlements plist validation",
    )?;
    let apple_tools = apple_release_tools();
    let identities = developer_id_identity_count()?;
    let profile_requested = options.keychain_profile.is_some();
    let profile_valid = match options.keychain_profile.as_deref() {
        Some(profile) => notary_profile_is_valid(profile),
        None => false,
    };
    let blockers = preflight_blockers(
        apple_tools.iter().all(|(_, ready)| *ready),
        identities,
        profile_requested,
        profile_valid,
    );
    let release_ready = blockers.is_empty();
    let executable = options.app.join("Contents/MacOS").join(EXECUTABLE);
    let daemon = options.app.join("Contents/MacOS").join(DAEMON_EXECUTABLE);
    let blocker_summary = blockers.join(",");
    let report = json!({
        "schema_version": 2,
        "proof": "leserpent-macos-release-preflight",
        "platform": "macos",
        "host_arch": env::consts::ARCH,
        "app": options.app.file_name().and_then(|value| value.to_str()).unwrap_or("Leserpent.app"),
        "version": PRODUCT_VERSION,
        "app_executable_sha256": file_sha256(&executable)?,
        "daemon_executable_sha256": file_sha256(&daemon)?,
        "entitlements_sha256": file_sha256(&options.entitlements)?,
        "apple_tools": apple_tools.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "developer_id_application_identities": identities,
        "notary_profile_requested": profile_requested,
        "notary_profile_valid": profile_valid,
        "release_ready": release_ready,
        "blockers": blockers,
        "result": if release_ready { "ready" } else { "blocked" },
    });
    let report = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    enforce_preflight_readiness(
        options.require_ready,
        release_ready,
        &blocker_summary,
        report,
    )
}

fn enforce_preflight_readiness(
    require_ready: bool,
    release_ready: bool,
    blocker_summary: &str,
    report: String,
) -> Result<String, String> {
    if require_ready && !release_ready {
        return Err(format!(
            "release preflight is blocked: {blocker_summary}; report={report}"
        ));
    }
    Ok(report)
}

fn apple_release_tools() -> Vec<(&'static str, bool)> {
    let mut tools = vec![
        ("codesign", Path::new("/usr/bin/codesign").is_file()),
        ("ditto", Path::new("/usr/bin/ditto").is_file()),
        ("plutil", Path::new("/usr/bin/plutil").is_file()),
        ("security", Path::new("/usr/bin/security").is_file()),
        ("spctl", Path::new("/usr/sbin/spctl").is_file()),
        ("xcrun", Path::new("/usr/bin/xcrun").is_file()),
    ];
    tools.push(("notarytool", xcrun_finds("notarytool")));
    tools.push(("stapler", xcrun_finds("stapler")));
    tools
}

fn xcrun_finds(tool: &str) -> bool {
    Command::new("xcrun")
        .args(["--find", tool])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn developer_id_identity_count() -> Result<usize, String> {
    let output = run_checked(
        Command::new("security").args(["find-identity", "-v", "-p", "codesigning"]),
        "Developer ID identity inventory",
    )?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains("Developer ID Application:"))
        .count())
}

fn notary_profile_is_valid(profile: &str) -> bool {
    validate_opaque(profile, "keychain profile").is_ok()
        && Command::new("xcrun")
            .args([
                "notarytool",
                "history",
                "--keychain-profile",
                profile,
                "--output-format",
                "json",
            ])
            .output()
            .is_ok_and(|output| output.status.success())
}

fn preflight_blockers(
    tools_ready: bool,
    identities: usize,
    profile_requested: bool,
    profile_valid: bool,
) -> Vec<&'static str> {
    let mut blockers = Vec::new();
    if !tools_ready {
        blockers.push("apple_release_tool_missing");
    }
    if identities == 0 {
        blockers.push("developer_id_application_identity_missing");
    }
    if !profile_requested {
        blockers.push("notary_keychain_profile_not_requested");
    } else if !profile_valid {
        blockers.push("notary_keychain_profile_unavailable");
    }
    blockers
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
    Ok(digest(&SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn formal_release_claims(allow_adhoc: bool) -> bool {
    !allow_adhoc
}

fn sign(options: &Options) -> Result<String, String> {
    let identity = options.identity.as_deref().expect("validated identity");
    validate_regular_file(&options.entitlements, "entitlements")?;
    run_checked(
        Command::new("plutil")
            .arg("-lint")
            .arg(&options.entitlements),
        "entitlements plist validation",
    )?;

    for payload in nested_native_payloads(&options.app)? {
        run_codesign(identity, &payload, None)?;
    }
    run_codesign(identity, &options.app, Some(&options.entitlements))?;
    verify_signature(&options.app, false)?;
    Ok(format!(
        "Leserpent macOS release valid: action=sign, app={}, hardened_runtime=true, secure_timestamp=true, developer_id=true",
        options.app.display()
    ))
}

fn notarize(options: &Options) -> Result<String, String> {
    verify_signature(&options.app, false)?;
    let profile = options
        .keychain_profile
        .as_deref()
        .expect("validated keychain profile");
    let archive = env::temp_dir().join(format!("leserpent-notarization-{}.zip", process::id()));
    if archive.exists() {
        return Err(format!(
            "temporary notarization archive already exists: {}",
            archive.display()
        ));
    }
    let result = (|| {
        run_checked(
            Command::new("ditto")
                .args(["-c", "-k", "--keepParent"])
                .arg(&options.app)
                .arg(&archive),
            "notarization archive creation",
        )?;
        let submission = run_checked(
            Command::new("xcrun")
                .args(["notarytool", "submit"])
                .arg(&archive)
                .args([
                    "--keychain-profile",
                    profile,
                    "--wait",
                    "--output-format",
                    "json",
                ]),
            "Apple notarization submission",
        )?;
        require_accepted(&submission.stdout)?;
        run_checked(
            Command::new("xcrun")
                .args(["stapler", "staple"])
                .arg(&options.app),
            "notarization ticket staple",
        )?;
        run_checked(
            Command::new("xcrun")
                .args(["stapler", "validate"])
                .arg(&options.app),
            "notarization ticket validation",
        )?;
        gatekeeper_assess(&options.app)
    })();
    let _ = fs::remove_file(&archive);
    result?;
    Ok(format!(
        "Leserpent macOS release valid: action=notarize, app={}, accepted=true, stapled=true, gatekeeper=true",
        options.app.display()
    ))
}

fn run_codesign(identity: &str, path: &Path, entitlements: Option<&Path>) -> Result<(), String> {
    let mut command = Command::new("codesign");
    command.args([
        "--force",
        "--sign",
        identity,
        "--options",
        "runtime",
        "--timestamp",
    ]);
    if let Some(entitlements) = entitlements {
        command.arg("--entitlements").arg(entitlements);
    }
    command.arg(path);
    run_checked(&mut command, &format!("code signing {}", path.display()))?;
    Ok(())
}

fn verify_signature(app: &Path, allow_adhoc: bool) -> Result<(), String> {
    run_checked(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict", "--verbose=2"])
            .arg(app),
        "strict code-signature verification",
    )?;
    let app_details = signature_details(app)?;
    validate_signature_policy(&app_details, "app bundle", allow_adhoc)?;
    let app_team = if allow_adhoc {
        None
    } else {
        Some(team_identifier(&app_details)?)
    };
    for payload in nested_native_payloads(app)? {
        run_checked(
            Command::new("codesign")
                .args(["--verify", "--strict", "--verbose=2"])
                .arg(&payload),
            &format!(
                "nested code-signature verification for {}",
                payload.display()
            ),
        )?;
        let details = signature_details(&payload)?;
        validate_signature_policy(&details, &payload.display().to_string(), allow_adhoc)?;
        if !allow_adhoc && Some(team_identifier(&details)?) != app_team {
            return Err(format!(
                "nested signature Team ID does not match the app bundle: {}",
                payload.display()
            ));
        }
    }
    Ok(())
}

fn signature_details(path: &Path) -> Result<String, String> {
    let output = run_checked(
        Command::new("codesign")
            .args(["--display", "--verbose=4"])
            .arg(path),
        &format!("code-signature inspection for {}", path.display()),
    )?;
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
}

fn validate_signature_policy(details: &str, label: &str, allow_adhoc: bool) -> Result<(), String> {
    if !has_hardened_runtime(details) {
        return Err(format!(
            "{label} signature does not enable Hardened Runtime"
        ));
    }
    if !allow_adhoc
        && (!details.contains("Authority=Developer ID Application:")
            || !has_secure_timestamp(details))
    {
        return Err(format!(
            "{label} signature is not a timestamped Developer ID Application signature"
        ));
    }
    Ok(())
}

fn has_secure_timestamp(details: &str) -> bool {
    details.lines().any(|line| {
        line.strip_prefix("Timestamp=")
            .is_some_and(|value| !value.is_empty() && value != "none")
    })
}

fn team_identifier(details: &str) -> Result<&str, String> {
    match details
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
    {
        Some(value) if !value.is_empty() && value != "not set" => Ok(value),
        _ => Err("Developer ID signature has no Team ID".to_string()),
    }
}

fn has_hardened_runtime(details: &str) -> bool {
    details
        .lines()
        .any(|line| line.starts_with("CodeDirectory ") && line.contains("runtime"))
}

fn gatekeeper_assess(app: &Path) -> Result<(), String> {
    run_checked(
        Command::new("spctl")
            .args(["--assess", "--type", "execute", "--verbose=4"])
            .arg(app),
        "Gatekeeper assessment",
    )?;
    Ok(())
}

fn validate_app(app: &Path) -> Result<(), String> {
    if app.extension().and_then(|value| value.to_str()) != Some("app") {
        return Err("--app must point to a .app bundle".to_string());
    }
    let metadata =
        fs::symlink_metadata(app).map_err(|error| format!("app bundle is unavailable: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("app bundle must be a non-symlink directory".to_string());
    }
    let plist = fs::read_to_string(app.join("Contents/Info.plist"))
        .map_err(|error| format!("Info.plist is unavailable: {error}"))?;
    validate_plist(&plist)?;
    validate_regular_file(
        &app.join("Contents/MacOS").join(EXECUTABLE),
        "main executable",
    )?;
    validate_regular_file(
        &app.join("Contents/MacOS").join(DAEMON_EXECUTABLE),
        "local orchestra daemon",
    )?;
    validate_executable_permission(
        &app.join("Contents/MacOS").join(EXECUTABLE),
        "main executable",
    )?;
    validate_executable_permission(
        &app.join("Contents/MacOS").join(DAEMON_EXECUTABLE),
        "local orchestra daemon",
    )?;
    validate_macos_payload(app)?;
    reject_symlinks(app)?;
    Ok(())
}

fn validate_plist(plist: &str) -> Result<(), String> {
    for (key, expected) in [
        ("CFBundleIdentifier", BUNDLE_ID),
        ("CFBundleExecutable", EXECUTABLE),
        ("CFBundlePackageType", "APPL"),
        ("CFBundleShortVersionString", PRODUCT_VERSION),
        ("CFBundleVersion", PRODUCT_VERSION),
    ] {
        let observed = plist_string_value(plist, key)?;
        if observed != expected {
            return Err(format!(
                "Info.plist {key} does not match the release contract"
            ));
        }
    }
    let issuer_marker = format!("<key>{SILVORTEX_ISSUER_KEY}</key>");
    if plist.contains(&issuer_marker) {
        let issuer = plist_string_value(plist, SILVORTEX_ISSUER_KEY)?;
        if !is_canonical_https_origin(issuer) {
            return Err(
                "Info.plist Team Silvortex issuer is not a canonical HTTPS origin".to_string(),
            );
        }
    }
    Ok(())
}

fn plist_string_value<'a>(plist: &'a str, key: &str) -> Result<&'a str, String> {
    let marker = format!("<key>{key}</key>");
    let mut matches = plist.match_indices(&marker);
    let (offset, _) = matches
        .next()
        .ok_or_else(|| format!("Info.plist is missing {key}"))?;
    if matches.next().is_some() {
        return Err(format!("Info.plist contains duplicate {key}"));
    }
    let tail = plist[offset + marker.len()..].trim_start();
    let tail = tail
        .strip_prefix("<string>")
        .ok_or_else(|| format!("Info.plist {key} must be a string"))?;
    let end = tail
        .find("</string>")
        .ok_or_else(|| format!("Info.plist {key} string is unterminated"))?;
    let value = &tail[..end];
    if value.contains(['<', '>']) {
        return Err(format!("Info.plist {key} contains nested markup"));
    }
    Ok(value)
}

fn validate_macos_payload(app: &Path) -> Result<(), String> {
    for entry in fs::read_dir(app.join("Contents/MacOS")).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        let is_library = entry
            .path()
            .extension()
            .is_some_and(|value| value == "dylib");
        if !file_type.is_file()
            || (entry.file_name() != EXECUTABLE
                && entry.file_name() != DAEMON_EXECUTABLE
                && !is_library)
        {
            return Err(format!(
                "app bundle contains an unsupported MacOS payload: {}",
                entry.path().display()
            ));
        }
        if !file_is_mach_o_arm64(&entry.path())
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?
        {
            return Err(format!(
                "app bundle contains a non-ARM64 Mach-O payload: {}",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn reject_symlinks(root: &Path) -> Result<(), String> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                return Err(format!(
                    "app bundle contains a forbidden symlink: {}",
                    entry.path().display()
                ));
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(())
}

fn nested_native_payloads(app: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = app.join("Contents/MacOS");
    let mut payloads = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| {
            format!(
                "failed to enumerate native payload under {}: {error}",
                directory.display()
            )
        })?;
        if entry.file_name() == EXECUTABLE {
            validate_regular_file(&entry.path(), "main executable")?;
            continue;
        }
        let path = entry.path();
        let is_daemon = entry.file_name() == DAEMON_EXECUTABLE;
        if !is_daemon && path.extension().is_none_or(|value| value != "dylib") {
            return Err(format!(
                "native signing snapshot contains an unsupported payload: {}",
                path.display()
            ));
        }
        let label = if is_daemon {
            "local orchestra daemon"
        } else {
            "native library"
        };
        validate_regular_file(&path, label)?;
        if is_daemon {
            validate_executable_permission(&path, label)?;
        }
        if !file_is_mach_o_arm64(&path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
        {
            return Err(format!(
                "native signing snapshot contains a non-ARM64 Mach-O payload: {}",
                path.display()
            ));
        }
        payloads.push(path);
    }
    if !payloads.iter().any(|path| {
        path.file_name()
            .is_some_and(|name| name == DAEMON_EXECUTABLE)
    }) {
        return Err("native signing snapshot is missing the local orchestra daemon".to_string());
    }
    payloads.sort();
    Ok(payloads)
}

fn validate_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_executable_permission(path: &Path, label: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {label} permissions: {error}"))?
        .permissions()
        .mode();
    if mode & 0o111 == 0 {
        return Err(format!("{label} is not executable"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_executable_permission(_path: &Path, _label: &str) -> Result<(), String> {
    Ok(())
}

fn validate_opaque(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('-')
        || value.chars().any(char::is_control)
    {
        return Err(format!("{label} is invalid"));
    }
    Ok(())
}

fn require_accepted(bytes: &[u8]) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "notarytool returned invalid JSON".to_string())?;
    if value.get("status").and_then(|value| value.as_str()) != Some("Accepted") {
        return Err("Apple notarization did not return Accepted".to_string());
    }
    Ok(())
}

fn run_checked(command: &mut Command, context: &str) -> Result<Output, String> {
    let output = command
        .output()
        .map_err(|error| format!("{context} could not start: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{context} failed: {}",
            detail.chars().take(2048).collect::<String>().trim()
        ));
    }
    Ok(output)
}

fn usage() -> &'static str {
    "usage:\n  gewyvern_leserpent_release preflight --app Leserpent.app [--keychain-profile PROFILE] [--entitlements FILE] [--require-ready]\n  gewyvern_leserpent_release sign --app Leserpent.app --identity 'Developer ID Application: ...' [--entitlements FILE]\n  gewyvern_leserpent_release notarize --app Leserpent.app --keychain-profile PROFILE\n  gewyvern_leserpent_release verify --app Leserpent.app [--allow-adhoc]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn parse(arguments: &[&str]) -> Result<Options, String> {
        Options::parse(arguments.iter().map(|value| value.to_string()))
    }

    fn valid_plist() -> String {
        format!(
            "<dict><key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>\
             <key>CFBundleExecutable</key><string>{EXECUTABLE}</string>\
             <key>CFBundlePackageType</key><string>APPL</string>\
             <key>CFBundleShortVersionString</key><string>{PRODUCT_VERSION}</string>\
             <key>CFBundleVersion</key><string>{PRODUCT_VERSION}</string></dict>"
        )
    }

    #[test]
    fn accepts_only_action_scoped_release_options() {
        assert!(parse(&["preflight", "--app", "Leserpent.app"]).is_ok());
        assert!(
            parse(&[
                "preflight",
                "--app",
                "Leserpent.app",
                "--keychain-profile",
                "leserpent-notary",
                "--require-ready"
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "sign",
                "--app",
                "Leserpent.app",
                "--identity",
                "Developer ID Application: Gewyvern (TEAMID)"
            ])
            .is_ok()
        );
        assert!(
            parse(&[
                "notarize",
                "--app",
                "Leserpent.app",
                "--keychain-profile",
                "leserpent-notary"
            ])
            .is_ok()
        );
        assert!(parse(&["verify", "--app", "Leserpent.app", "--allow-adhoc"]).is_ok());
        assert!(
            parse(&[
                "verify",
                "--app",
                "Leserpent.app",
                "--entitlements",
                "unexpected.plist"
            ])
            .is_err()
        );
        assert!(parse(&["verify", "--app", "Leserpent.app", "--require-ready"]).is_err());
        assert!(
            parse(&[
                "sign",
                "--app",
                "Leserpent.app",
                "--identity",
                "Apple Development: Example"
            ])
            .is_err()
        );
        assert!(
            parse(&[
                "notarize",
                "--app",
                "Leserpent.app",
                "--keychain-profile",
                "-unsafe"
            ])
            .is_err()
        );
    }

    #[test]
    fn accepts_only_explicit_notarization_success() {
        assert!(require_accepted(br#"{"status":"Accepted","id":"fixture"}"#).is_ok());
        assert!(require_accepted(br#"{"status":"Invalid","id":"fixture"}"#).is_err());
        assert!(require_accepted(b"not-json").is_err());
    }

    #[test]
    fn recognizes_formal_and_adhoc_hardened_runtime_flags() {
        assert!(has_hardened_runtime(
            "CodeDirectory v=20500 flags=0x10000(runtime) hashes=1+1"
        ));
        assert!(has_hardened_runtime(
            "CodeDirectory v=20500 flags=0x10002(adhoc,runtime) hashes=1+1"
        ));
        assert!(!has_hardened_runtime(
            "CodeDirectory v=20400 flags=0x2(adhoc) hashes=1+1"
        ));
    }

    #[test]
    fn signature_policy_requires_runtime_and_formal_identity_fields() {
        let formal = "CodeDirectory flags=runtime\nAuthority=Developer ID Application: Example\nTimestamp=Jul 20, 2026\nTeamIdentifier=TEAM123";
        assert!(validate_signature_policy(formal, "payload", false).is_ok());
        assert_eq!(team_identifier(formal).unwrap(), "TEAM123");
        assert!(validate_signature_policy(formal, "payload", true).is_ok());
        assert!(validate_signature_policy("CodeDirectory flags=adhoc", "payload", true).is_err());
        assert!(team_identifier("TeamIdentifier=not set").is_err());
        assert!(validate_signature_policy(
            "CodeDirectory flags=runtime\nAuthority=Developer ID Application: Example\nTimestamp=none",
            "payload",
            false
        )
        .is_err());
    }

    #[test]
    fn withholds_release_and_runtime_claims_from_adhoc_verification() {
        assert!(!formal_release_claims(true));
        assert!(formal_release_claims(false));
    }

    #[test]
    fn preflight_readiness_is_explicit_and_non_vacuous() {
        assert_eq!(
            preflight_blockers(true, 0, false, false),
            [
                "developer_id_application_identity_missing",
                "notary_keychain_profile_not_requested"
            ]
        );
        assert_eq!(
            preflight_blockers(true, 1, true, false),
            ["notary_keychain_profile_unavailable"]
        );
        assert!(preflight_blockers(true, 1, true, true).is_empty());
        let blocked_report = r#"{"release_ready":false}"#.to_string();
        assert_eq!(
            enforce_preflight_readiness(
                false,
                false,
                "developer_id_application_identity_missing",
                blocked_report.clone(),
            )
            .unwrap(),
            blocked_report
        );
        let error = enforce_preflight_readiness(
            true,
            false,
            "developer_id_application_identity_missing",
            r#"{"release_ready":false}"#.to_string(),
        )
        .unwrap_err();
        assert!(error.contains("release preflight is blocked"));
        assert!(error.contains("developer_id_application_identity_missing"));
        assert!(
            enforce_preflight_readiness(true, true, "", r#"{"release_ready":true}"#.to_string(),)
                .is_ok()
        );
    }

    #[test]
    fn plist_validation_binds_unique_identity_and_workspace_version_fields() {
        let valid = valid_plist();
        assert!(validate_plist(&valid).is_ok());
        let with_issuer = valid.replace(
            "</dict>",
            &format!(
                "<key>{SILVORTEX_ISSUER_KEY}</key>\
                 <string>https://id.example.invalid/</string></dict>"
            ),
        );
        assert!(validate_plist(&with_issuer).is_ok());
        assert!(validate_plist(&with_issuer.replace("https://", "http://")).is_err());
        assert!(
            validate_plist(
                &with_issuer.replace("https://id.example.invalid/", "https://foo&amp;bar/")
            )
            .is_err()
        );
        assert!(
            validate_plist(&with_issuer.replace(
                "https://id.example.invalid/",
                "https://id.example.invalid:443/"
            ))
            .is_err()
        );
        assert!(
            validate_plist(&with_issuer.replace(
                &format!("<key>{SILVORTEX_ISSUER_KEY}</key>"),
                &format!(
                    "<key>{SILVORTEX_ISSUER_KEY}</key><string>https://id.example.invalid/</string>\
                     <key>{SILVORTEX_ISSUER_KEY}</key>"
                ),
            ))
            .is_err()
        );

        let duplicate = valid.replace(
            "<key>CFBundleVersion</key>",
            "<key>CFBundleVersion</key><string>0</string><key>CFBundleVersion</key>",
        );
        assert!(validate_plist(&duplicate).is_err());
        assert!(
            validate_plist(&valid.replace(
                &format!("<string>{PRODUCT_VERSION}</string></dict>"),
                "<string>0.0.0</string></dict>",
            ))
            .is_err()
        );
        assert!(
            validate_plist(&valid.replace(
                "<key>CFBundlePackageType</key><string>APPL</string>",
                "<key>CFBundlePackageType</key><true/>",
            ))
            .is_err()
        );
    }

    #[test]
    fn app_validation_rejects_non_arm64_executables_and_libraries() {
        let root = env::temp_dir().join(format!(
            "gewyvern-leserpent-release-payload-{}",
            process::id()
        ));
        let app = root.join("Leserpent.app");
        let contents = app.join("Contents");
        let macos = contents.join("MacOS");
        fs::create_dir_all(&macos).unwrap();
        fs::write(contents.join("Info.plist"), valid_plist()).unwrap();
        let executable = macos.join(EXECUTABLE);
        fs::write(&executable, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let daemon = macos.join(DAEMON_EXECUTABLE);
        fs::write(&daemon, b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01daemon").unwrap();
        fs::set_permissions(&daemon, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(validate_app(&app).unwrap_err().contains("non-ARM64 Mach-O"));

        fs::write(&executable, b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01payload").unwrap();
        fs::write(macos.join("fake.dylib"), b"not-a-library").unwrap();
        assert!(validate_app(&app).unwrap_err().contains("non-ARM64 Mach-O"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signing_snapshot_rejects_unknown_payloads() {
        let root = env::temp_dir().join(format!(
            "gewyvern-leserpent-release-snapshot-{}",
            process::id()
        ));
        let app = root.join("Leserpent.app");
        let macos = app.join("Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        let payload = b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01payload";
        fs::write(macos.join(EXECUTABLE), payload).unwrap();
        fs::write(macos.join(DAEMON_EXECUTABLE), payload).unwrap();
        fs::set_permissions(
            macos.join(DAEMON_EXECUTABLE),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        fs::write(macos.join("native.dylib"), payload).unwrap();
        assert_eq!(nested_native_payloads(&app).unwrap().len(), 2);

        fs::write(macos.join("late-addition.txt"), b"unexpected").unwrap();
        assert!(nested_native_payloads(&app).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signing_snapshot_requires_an_executable_arm64_daemon() {
        let root = env::temp_dir().join(format!(
            "gewyvern-leserpent-release-daemon-snapshot-{}",
            process::id()
        ));
        let app = root.join("Leserpent.app");
        let macos = app.join("Contents/MacOS");
        fs::create_dir_all(&macos).unwrap();
        let payload = b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01payload";
        fs::write(macos.join(EXECUTABLE), payload).unwrap();
        assert!(
            nested_native_payloads(&app)
                .unwrap_err()
                .contains("missing")
        );

        let daemon = macos.join(DAEMON_EXECUTABLE);
        fs::write(&daemon, payload).unwrap();
        assert!(
            nested_native_payloads(&app)
                .unwrap_err()
                .contains("not executable")
        );
        fs::set_permissions(&daemon, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(nested_native_payloads(&app).unwrap(), vec![daemon]);
        fs::remove_dir_all(root).unwrap();
    }
}
