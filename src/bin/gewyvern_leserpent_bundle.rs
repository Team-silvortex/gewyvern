use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use gewyvern::native_binary::file_is_mach_o_arm64;

const EXECUTABLE: &str = "Leserpent.Avalonia";
const DAEMON_EXECUTABLE: &str = "leserpentd";
const MAX_PUBLISH_FILES: usize = 32;
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(test)]
const PRODUCT_VERSION_FOR_TEST: &str = env!("CARGO_PKG_VERSION");

fn main() {
    match Options::parse(env::args().skip(1))
        .and_then(|options| create_bundle(&options).map(|_| options))
    {
        Ok(options) => println!(
            "Leserpent app bundle valid: path={}, version={}, executable={}, token_files=false",
            options.output.display(),
            options.version,
            EXECUTABLE
        ),
        Err(error) => {
            eprintln!("Leserpent app bundle failed: {error}");
            process::exit(1);
        }
    }
}

#[derive(Debug)]
struct Options {
    publish_dir: PathBuf,
    output: PathBuf,
    icon: PathBuf,
    daemon: Option<PathBuf>,
    version: String,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut publish_dir = None;
        let mut output = None;
        let mut icon = PathBuf::from("assets/branding/leserpent-icon.icns");
        let mut daemon = None;
        let mut version = env!("CARGO_PKG_VERSION").to_string();
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            let value = match argument.as_str() {
                "--publish-dir" | "--output" | "--icon" | "--version" | "--daemon" => {
                    args.next()
                        .ok_or_else(|| format!("{argument} requires a value"))?
                }
                "--help" | "-h" => return Err(usage().to_string()),
                _ => return Err(format!("unknown argument `{argument}`\n{}", usage())),
            };
            match argument.as_str() {
                "--publish-dir" => publish_dir = Some(PathBuf::from(value)),
                "--output" => output = Some(PathBuf::from(value)),
                "--icon" => icon = PathBuf::from(value),
                "--daemon" => daemon = Some(PathBuf::from(value)),
                "--version" => version = value,
                _ => unreachable!(),
            }
        }
        let options = Self {
            publish_dir: publish_dir.ok_or_else(|| "--publish-dir is required".to_string())?,
            output: output.ok_or_else(|| "--output is required".to_string())?,
            icon,
            daemon,
            version,
        };
        options.validate()?;
        Ok(options)
    }

    fn validate(&self) -> Result<(), String> {
        if self.output.extension().and_then(|value| value.to_str()) != Some("app") {
            return Err("--output must end in .app".to_string());
        }
        if let Some(daemon) = self.daemon.as_deref() {
            require_file(daemon, "configured daemon", None)?;
        }
        let segments = self.version.split('.').collect::<Vec<_>>();
        if !(1..=3).contains(&segments.len())
            || segments.iter().any(|segment| {
                segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err("--version must contain one to three numeric components".to_string());
        }
        Ok(())
    }
}

fn create_bundle(options: &Options) -> Result<(), String> {
    require_directory(&options.publish_dir, "publish directory")?;
    require_file(&options.icon, "icon", Some(MAX_ICON_BYTES))?;
    if options.output.exists() {
        return Err(format!(
            "output already exists; refusing to replace {}",
            options.output.display()
        ));
    }

    let mut files = publish_files(&options.publish_dir)?;
    files.push(resolve_daemon_payload(
        &options.publish_dir,
        options.daemon.as_deref(),
    )?);
    files.sort();
    files.dedup();
    if !files
        .iter()
        .any(|path| path.file_name().is_some_and(|name| name == EXECUTABLE))
    {
        return Err(format!("publish directory does not contain {EXECUTABLE}"));
    }
    for path in &files {
        require_mach_o_arm64(path)?;
    }

    let contents = options.output.join("Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos).map_err(|error| error.to_string())?;
    fs::create_dir_all(&resources).map_err(|error| error.to_string())?;
    let result = (|| {
        for source in files {
            let name = source
                .file_name()
                .ok_or_else(|| "publish file has no name".to_string())?;
            fs::copy(&source, macos.join(name)).map_err(|error| error.to_string())?;
        }
        fs::copy(&options.icon, resources.join("leserpent.icns"))
            .map_err(|error| error.to_string())?;
        fs::write(contents.join("Info.plist"), info_plist(&options.version))
            .map_err(|error| error.to_string())?;
        verify_bundle(&options.output, &options.version)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&options.output);
    }
    result
}

fn require_mach_o_arm64(path: &Path) -> Result<(), String> {
    if !file_is_mach_o_arm64(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?
    {
        return Err(format!(
            "bundle payload is not a 64-bit ARM Mach-O file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn publish_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir()
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name == format!("{EXECUTABLE}.dSYM"))
        {
            continue;
        }
        if file_type.is_file() && entry.path().extension().is_some_and(|value| value == "pdb") {
            continue;
        }
        if file_type.is_symlink() || !file_type.is_file() {
            return Err(format!(
                "publish directory must contain regular files only: {}",
                entry.path().display()
            ));
        }
        let name = entry.file_name();
        let is_daemon = name == DAEMON_EXECUTABLE;
        if name != EXECUTABLE
            && !is_daemon
            && entry
                .path()
                .extension()
                .is_none_or(|value| value != "dylib")
        {
            return Err(format!(
                "publish directory contains an unsupported bundle file: {}",
                entry.path().display()
            ));
        }
        files.push(entry.path());
        if files.len() > MAX_PUBLISH_FILES {
            return Err(format!(
                "publish directory exceeds the {MAX_PUBLISH_FILES}-file limit"
            ));
        }
    }
    files.sort();
    Ok(files)
}

fn resolve_daemon_payload(
    publish_directory: &Path,
    configured: Option<&Path>,
) -> Result<PathBuf, String> {
    let candidates = if let Some(configured_daemon) = configured {
        vec![configured_daemon.to_path_buf()]
    } else {
        discover_daemon_payload_candidates(publish_directory)
    };

    for candidate in candidates {
        if candidate.exists() {
            require_file(&candidate, "daemon payload", None)?;
            if file_is_mach_o_arm64(&candidate)
                .map_err(|error| format!("failed to inspect {}: {error}", candidate.display()))?
            {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "a 64-bit ARM Mach-O {DAEMON_EXECUTABLE} payload is required; build it first or pass --daemon FILE"
    ))
}

fn discover_daemon_payload_candidates(publish_directory: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![publish_directory.join(DAEMON_EXECUTABLE)];

    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut target_roots = vec![
        manifest_root.join("target"),
        manifest_root
            .join("crates")
            .join("leserpentd")
            .join("target"),
    ];

    let mut triples = Vec::new();
    if let Ok(explicit) = std::env::var("LESERPENT_DAEMON_TARGET")
        && !explicit.trim().is_empty()
    {
        triples.push(explicit);
    }

    let host_triple = match std::env::consts::ARCH {
        "aarch64" => Some("aarch64-apple-darwin"),
        "x86_64" => Some("x86_64-apple-darwin"),
        _ => None,
    };
    if let Some(host_triple) = host_triple
        && !triples.iter().any(|value| value == host_triple)
    {
        triples.push(host_triple.to_string());
    }

    for target_root in &mut target_roots {
        candidates.push(target_root.join("release").join(DAEMON_EXECUTABLE));
        candidates.push(target_root.join("debug").join(DAEMON_EXECUTABLE));
        for triple in &triples {
            candidates.push(
                target_root
                    .join(triple)
                    .join("release")
                    .join(DAEMON_EXECUTABLE),
            );
            candidates.push(
                target_root
                    .join(triple)
                    .join("debug")
                    .join(DAEMON_EXECUTABLE),
            );
        }
    }

    candidates
}

fn require_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!("{label} must be a non-symlink directory"));
    }
    Ok(())
}

fn require_file(path: &Path, label: &str, max_bytes: Option<u64>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("{label} {} is unavailable: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    if max_bytes.is_some_and(|limit| metadata.len() > limit) {
        return Err(format!("{label} exceeds its size limit"));
    }
    Ok(())
}

fn verify_bundle(bundle: &Path, version: &str) -> Result<(), String> {
    let executable = bundle.join("Contents/MacOS").join(EXECUTABLE);
    let daemon = bundle.join("Contents/MacOS").join(DAEMON_EXECUTABLE);
    require_file(&executable, "bundled executable", None)?;
    require_file(&daemon, "bundled daemon", None)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for (path, label) in [(&executable, "executable"), (&daemon, "daemon")] {
            if fs::metadata(path)
                .map_err(|error| error.to_string())?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(format!("bundled {label} has no execute permission"));
            }
        }
    }
    require_file(
        &bundle.join("Contents/Resources/leserpent.icns"),
        "bundled icon",
        Some(MAX_ICON_BYTES),
    )?;
    let plist = fs::read_to_string(bundle.join("Contents/Info.plist"))
        .map_err(|error| error.to_string())?;
    if plist != info_plist(version) {
        return Err("generated Info.plist failed its exact metadata contract".to_string());
    }
    Ok(())
}

fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>Leserpent</string>
  <key>CFBundleExecutable</key>
  <string>Leserpent.Avalonia</string>
  <key>CFBundleIconFile</key>
  <string>leserpent.icns</string>
  <key>CFBundleIdentifier</key>
  <string>org.gewyvern.leserpent</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>Leserpent</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>{version}</string>
  <key>CFBundleVersion</key>
  <string>{version}</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key>
  <string>12.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
"#
    )
}

fn usage() -> &'static str {
    "usage: gewyvern_leserpent_bundle --publish-dir DIR --output Leserpent.app [--daemon FILE] [--icon FILE] [--version X.Y.Z]"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gewyvern-leserpent-bundle-{label}-{}",
            process::id()
        ))
    }

    fn mach_o_arm64_fixture() -> &'static [u8] {
        b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01payload"
    }

    #[test]
    fn options_require_safe_bundle_identity() {
        assert!(
            Options::parse(
                ["--publish-dir", "publish", "--output", "Leserpent.app"]
                    .into_iter()
                    .map(str::to_string)
            )
            .is_ok()
        );
        assert!(
            Options::parse(
                [
                    "--publish-dir",
                    "publish",
                    "--output",
                    "Leserpent",
                    "--version",
                    "1.2.0"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_err()
        );
        assert!(
            Options::parse(
                [
                    "--publish-dir",
                    "publish",
                    "--output",
                    "Leserpent.app",
                    "--version",
                    "1.2-beta"
                ]
                .into_iter()
                .map(str::to_string)
            )
            .is_err()
        );
    }

    #[test]
    fn plist_has_stable_product_metadata() {
        let plist = info_plist("1.2.0");
        assert!(plist.contains("org.gewyvern.leserpent"));
        assert!(plist.contains("public.app-category.developer-tools"));
        assert_eq!(plist.matches("<string>1.2.0</string>").count(), 2);
        assert!(!plist.contains("token"));
    }

    #[test]
    fn bundle_verification_rejects_ambiguous_or_mismatched_plist_metadata() {
        let root = fixture_root("plist-metadata");
        let publish = root.join("publish");
        let output = root.join("Leserpent.app");
        let icon = root.join("leserpent.icns");
        fs::create_dir_all(&publish).unwrap();
        let executable = publish.join(EXECUTABLE);
        fs::write(&executable, mach_o_arm64_fixture()).unwrap();
        fs::write(publish.join(DAEMON_EXECUTABLE), mach_o_arm64_fixture()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(
                publish.join(DAEMON_EXECUTABLE),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        fs::write(&icon, b"icns-fixture").unwrap();
        let options = Options {
            publish_dir: publish,
            output: output.clone(),
            icon,
            daemon: None,
            version: "1.2.0".to_string(),
        };
        create_bundle(&options).unwrap();

        let plist_path = output.join("Contents/Info.plist");
        let ambiguous = info_plist("1.2.0").replace(
            "<key>CFBundleVersion</key>",
            "<key>CFBundleVersion</key>\n  <key>CFBundleVersion</key>",
        );
        fs::write(&plist_path, ambiguous).unwrap();
        assert!(verify_bundle(&output, "1.2.0").is_err());

        fs::write(&plist_path, info_plist("1.2.1")).unwrap();
        assert!(verify_bundle(&output, "1.2.0").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundle_creation_rejects_non_arm64_payloads_before_writing_output() {
        let root = fixture_root("non-arm64");
        let publish = root.join("publish");
        let output = root.join("Leserpent.app");
        let icon = root.join("leserpent.icns");
        fs::create_dir_all(&publish).unwrap();
        fs::write(publish.join(EXECUTABLE), b"#!/bin/sh\nexit 0\n").unwrap();
        fs::write(publish.join(DAEMON_EXECUTABLE), mach_o_arm64_fixture()).unwrap();
        fs::write(&icon, b"icns-fixture").unwrap();
        let options = Options {
            publish_dir: publish,
            output: output.clone(),
            icon,
            daemon: None,
            version: PRODUCT_VERSION_FOR_TEST.to_string(),
        };

        assert!(create_bundle(&options).unwrap_err().contains("ARM Mach-O"));
        assert!(!output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundle_creation_requires_the_rust_daemon_payload() {
        let root = fixture_root("missing-daemon");
        let publish = root.join("publish");
        let output = root.join("Leserpent.app");
        let icon = root.join("leserpent.icns");
        fs::create_dir_all(&publish).unwrap();
        fs::write(publish.join(EXECUTABLE), mach_o_arm64_fixture()).unwrap();
        fs::write(&icon, b"icns-fixture").unwrap();
        let options = Options {
            publish_dir: publish,
            output: output.clone(),
            icon,
            daemon: Some(root.join("absent-leserpentd")),
            version: PRODUCT_VERSION_FOR_TEST.to_string(),
        };

        assert!(
            options
                .validate()
                .unwrap_err()
                .contains("configured daemon")
        );
        assert!(!output.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn creates_complete_bundle_and_refuses_replacement() {
        let root = fixture_root("complete");
        let publish = root.join("publish");
        let output = root.join("Leserpent.app");
        let icon = root.join("leserpent.icns");
        fs::create_dir_all(&publish).unwrap();
        let executable = publish.join(EXECUTABLE);
        fs::write(&executable, mach_o_arm64_fixture()).unwrap();
        fs::write(publish.join(DAEMON_EXECUTABLE), mach_o_arm64_fixture()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(
                publish.join(DAEMON_EXECUTABLE),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        fs::write(
            publish.join("libHarfBuzzSharp.dylib"),
            mach_o_arm64_fixture(),
        )
        .unwrap();
        fs::write(publish.join("Leserpent.RemoteClient.pdb"), b"debug-symbols").unwrap();
        fs::create_dir(publish.join(format!("{EXECUTABLE}.dSYM"))).unwrap();
        fs::write(&icon, b"icns-fixture").unwrap();
        let options = Options {
            publish_dir: publish,
            output: output.clone(),
            icon,
            daemon: None,
            version: "1.2.0".to_string(),
        };

        create_bundle(&options).unwrap();
        assert!(output.join("Contents/MacOS/Leserpent.Avalonia").is_file());
        assert!(
            output
                .join("Contents/MacOS/libHarfBuzzSharp.dylib")
                .is_file()
        );
        assert!(output.join("Contents/MacOS/leserpentd").is_file());
        assert!(output.join("Contents/Resources/leserpent.icns").is_file());
        assert!(
            create_bundle(&options)
                .unwrap_err()
                .contains("refusing to replace")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
