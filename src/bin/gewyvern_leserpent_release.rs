use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Output};

use gewyvern::native_binary::file_is_mach_o_arm64;

const BUNDLE_ID: &str = "org.gewyvern.leserpent";
const EXECUTABLE: &str = "Leserpent.Avalonia";
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
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = args.peekable();
        let action = match args.next().as_deref() {
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
        while let Some(argument) = args.next() {
            let value = match argument.as_str() {
                "--app" | "--identity" | "--keychain-profile" | "--entitlements" => args
                    .next()
                    .ok_or_else(|| format!("{argument} requires a value"))?,
                "--allow-adhoc" => {
                    allow_adhoc = true;
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
        };
        options.validate()?;
        Ok(options)
    }

    fn validate(&self) -> Result<(), String> {
        match self.action {
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
                if self.keychain_profile.is_some() || self.allow_adhoc {
                    return Err("sign received an option for another action".to_string());
                }
            }
            Action::Notarize => {
                let profile = self
                    .keychain_profile
                    .as_deref()
                    .ok_or_else(|| "notarize requires --keychain-profile".to_string())?;
                validate_opaque(profile, "keychain profile")?;
                if self.identity.is_some() || self.allow_adhoc || self.custom_entitlements {
                    return Err("notarize received an option for another action".to_string());
                }
            }
            Action::Verify => {
                if self.identity.is_some()
                    || self.keychain_profile.is_some()
                    || self.custom_entitlements
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

    for library in native_libraries(&options.app)? {
        run_codesign(identity, &library, None)?;
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
    let details = run_checked(
        Command::new("codesign")
            .args(["--display", "--verbose=4"])
            .arg(app),
        "code-signature inspection",
    )?;
    let details = String::from_utf8_lossy(&details.stderr);
    if !has_hardened_runtime(&details) {
        return Err("signature does not enable Hardened Runtime".to_string());
    }
    if !allow_adhoc
        && (!details.contains("Authority=Developer ID Application:")
            || !details.contains("Timestamp="))
    {
        return Err(
            "signature is not a timestamped Developer ID Application signature".to_string(),
        );
    }
    Ok(())
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
        if !file_type.is_file() || (entry.file_name() != EXECUTABLE && !is_library) {
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

fn native_libraries(app: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = app.join("Contents/MacOS");
    let mut libraries = Vec::new();
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
        if path.extension().is_none_or(|value| value != "dylib") {
            return Err(format!(
                "native signing snapshot contains an unsupported payload: {}",
                path.display()
            ));
        }
        validate_regular_file(&path, "native library")?;
        if !file_is_mach_o_arm64(&path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
        {
            return Err(format!(
                "native signing snapshot contains a non-ARM64 Mach-O library: {}",
                path.display()
            ));
        }
        libraries.push(path);
    }
    libraries.sort();
    Ok(libraries)
}

fn validate_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
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
    "usage:\n  gewyvern_leserpent_release sign --app Leserpent.app --identity 'Developer ID Application: ...' [--entitlements FILE]\n  gewyvern_leserpent_release notarize --app Leserpent.app --keychain-profile PROFILE\n  gewyvern_leserpent_release verify --app Leserpent.app [--allow-adhoc]"
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn withholds_release_and_runtime_claims_from_adhoc_verification() {
        assert!(!formal_release_claims(true));
        assert!(formal_release_claims(false));
    }

    #[test]
    fn plist_validation_binds_unique_identity_and_workspace_version_fields() {
        let valid = valid_plist();
        assert!(validate_plist(&valid).is_ok());

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
        fs::write(macos.join("native.dylib"), payload).unwrap();
        assert_eq!(native_libraries(&app).unwrap().len(), 1);

        fs::write(macos.join("late-addition.txt"), b"unexpected").unwrap();
        assert!(native_libraries(&app).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
