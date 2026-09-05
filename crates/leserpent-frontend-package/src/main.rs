use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use ring::digest::{Context, SHA256, digest};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use silvortex_bounded_io::open_bounded_regular_file_allow_empty;

const MANIFEST_SCHEMA: &str = "leserpent.frontend-package/v1";
const MAX_INPUT_FILES: usize = 128;
const MAX_ASSET_FILES: usize = 128;
const MAX_SCANNED_ENTRIES: usize = 512;
const MAX_DIRECTORY_DEPTH: usize = 16;
const MAX_INPUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ASSET_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 128 * 1024;
const MAX_PACKAGE_LOCK_BYTES: u64 = 8 * 1024 * 1024;
const MAX_INSTALLED_PACKAGE_BYTES: u64 = 256 * 1024;
const MAX_LOCK_BYTES: u64 = 256;
const LOCK_WAIT: Duration = Duration::from_secs(16 * 60);
const STALE_LOCK: Duration = Duration::from_secs(15 * 60);

fn main() {
    match run(env::args().skip(1)) {
        Ok(Some(message)) => println!("{message}"),
        Ok(None) => {}
        Err(error) => {
            eprintln!("frontend package failed: {error}");
            process::exit(1);
        }
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<Option<String>, String> {
    let options = Options::parse(args)?;
    if options.help {
        return Ok(Some(usage().to_string()));
    }
    let package = FrontendPackage::open(&options.package_root)?;

    if options.verify {
        let state = package.state()?;
        if package.read_manifest()?.as_ref() != Some(&state) {
            return Err("frontend package is stale; run `npm run package:frontend`".to_string());
        }
        return Ok(Some(format!(
            "frontend package verified: {} assets, {} bytes",
            state.asset_file_count, state.asset_bytes
        )));
    }

    let _lock = PackageLock::acquire(&package.root)?;
    let mut state = package.state()?;
    if !options.force && package.read_manifest()?.as_ref() == Some(&state) {
        return Ok(Some(format!(
            "frontend package up to date: {} assets, {} bytes",
            state.asset_file_count, state.asset_bytes
        )));
    }

    package.rebuild()?;
    state = package.state()?;
    package.write_manifest(&state)?;
    Ok(Some(format!(
        "frontend package rebuilt: {} assets, {} bytes",
        state.asset_file_count, state.asset_bytes
    )))
}

#[derive(Debug)]
struct Options {
    package_root: PathBuf,
    verify: bool,
    force: bool,
    help: bool,
}

impl Options {
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .map_err(|error| format!("failed to resolve repository root: {error}"))?;
        let mut package_root = repository_root.join("apps/leserpent");
        let mut verify = false;
        let mut force = false;
        let mut help = false;
        let mut package_root_set = false;
        let mut args = args.peekable();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--verify" if !verify => verify = true,
                "--force" if !force => force = true,
                "--package-root" if !package_root_set => {
                    package_root = PathBuf::from(
                        args.next()
                            .ok_or_else(|| "--package-root requires a path".to_string())?,
                    );
                    package_root_set = true;
                }
                "--help" | "-h" if !help => help = true,
                _ => {
                    return Err(format!(
                        "unknown or duplicate argument `{argument}`\n{}",
                        usage()
                    ));
                }
            }
        }
        if verify && force {
            return Err(format!(
                "--verify and --force are mutually exclusive\n{}",
                usage()
            ));
        }
        if help && (verify || force) {
            return Err(format!(
                "--help cannot be combined with an action\n{}",
                usage()
            ));
        }
        Ok(Self {
            package_root,
            verify,
            force,
            help,
        })
    }
}

fn usage() -> &'static str {
    "usage: leserpent-frontend-package [--verify|--force] [--package-root PATH]"
}

struct FrontendPackage {
    root: PathBuf,
    frontend_root: PathBuf,
    web_root: PathBuf,
    scripts_root: PathBuf,
    manifest_path: PathBuf,
}

impl FrontendPackage {
    fn open(root: &Path) -> Result<Self, String> {
        require_real_directory(root, "frontend package root")?;
        let root = root
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", root.display()))?;
        let frontend_root = root.join("src/Leserpent/frontend");
        let web_root = root.join("src/Leserpent/wwwroot");
        let scripts_root = root.join("scripts");
        require_real_directory(&frontend_root, "TypeScript source root")?;
        require_real_directory(&web_root, "web asset root")?;
        require_real_directory(&scripts_root, "frontend script root")?;
        Ok(Self {
            manifest_path: root.join("frontend-package-manifest.json"),
            root,
            frontend_root,
            web_root,
            scripts_root,
        })
    }

    fn state(&self) -> Result<FrontendPackageManifest, String> {
        let inputs = self.inventory(self.package_inputs()?, MAX_INPUT_BYTES)?;
        let assets = self.inventory(self.package_assets()?, MAX_ASSET_BYTES)?;
        Ok(FrontendPackageManifest {
            schema: MANIFEST_SCHEMA.to_string(),
            inputs_sha256: inputs.sha256,
            assets_sha256: assets.sha256,
            input_file_count: inputs.files.len(),
            asset_file_count: assets.files.len(),
            input_bytes: inputs.total_bytes,
            asset_bytes: assets.total_bytes,
            assets: assets.files,
        })
    }

    fn package_inputs(&self) -> Result<Vec<PathBuf>, String> {
        let mut scanned = 0;
        let mut files = self.collect_files(
            &self.frontend_root,
            &|path| path.extension() == Some(OsStr::new("ts")),
            MAX_INPUT_FILES,
            &mut scanned,
        )?;
        files.extend([
            self.root.join("package.json"),
            self.root.join("package-lock.json"),
            self.root.join("tsconfig.json"),
            self.scripts_root.join("build-language-packs.mjs"),
            self.scripts_root.join("check-language-pack-coverage.mjs"),
        ]);
        if files.len() > MAX_INPUT_FILES {
            return Err(format!(
                "frontend package input count exceeds {MAX_INPUT_FILES}"
            ));
        }
        self.sorted_paths(files)
    }

    fn package_assets(&self) -> Result<Vec<PathBuf>, String> {
        let mut scanned = 0;
        let files = self.collect_files(
            &self.web_root,
            &|path| !matches!(path.extension().and_then(OsStr::to_str), Some("br" | "gz")),
            MAX_ASSET_FILES,
            &mut scanned,
        )?;
        self.sorted_paths(files)
    }

    fn sorted_paths(&self, files: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
        let mut keyed = files
            .into_iter()
            .map(|path| self.relative_path(&path).map(|key| (key, path)))
            .collect::<Result<Vec<_>, _>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(keyed.into_iter().map(|(_, path)| path).collect())
    }

    fn collect_files(
        &self,
        root: &Path,
        predicate: &dyn Fn(&Path) -> bool,
        limit: usize,
        scanned: &mut usize,
    ) -> Result<Vec<PathBuf>, String> {
        fn visit(
            package: &FrontendPackage,
            directory: &Path,
            predicate: &dyn Fn(&Path) -> bool,
            limit: usize,
            depth: usize,
            scanned: &mut usize,
            files: &mut Vec<PathBuf>,
        ) -> Result<(), String> {
            if depth > MAX_DIRECTORY_DEPTH {
                return Err(format!(
                    "frontend package directory depth exceeds {MAX_DIRECTORY_DEPTH}"
                ));
            }
            let directory_entries = fs::read_dir(directory)
                .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
            let mut entries = Vec::new();
            for entry in directory_entries {
                *scanned = scanned
                    .checked_add(1)
                    .ok_or_else(|| "frontend package scan entry count overflowed".to_string())?;
                if *scanned > MAX_SCANNED_ENTRIES {
                    return Err(format!(
                        "frontend package scan exceeds {MAX_SCANNED_ENTRIES} entries"
                    ));
                }
                entries.push(
                    entry.map_err(|error| {
                        format!("failed to scan {}: {error}", directory.display())
                    })?,
                );
            }
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "frontend package rejects symlinked input: {}",
                        package.relative_path(&path)?
                    ));
                }
                if metadata.is_dir() {
                    visit(package, &path, predicate, limit, depth + 1, scanned, files)?;
                } else if !metadata.is_file() {
                    return Err(format!(
                        "frontend package rejects non-file input: {}",
                        package.relative_path(&path)?
                    ));
                } else if predicate(&path) {
                    files.push(path);
                    if files.len() > limit {
                        return Err(format!("frontend package file count exceeds {limit}"));
                    }
                }
            }
            Ok(())
        }

        require_real_directory(root, "frontend package inventory root")?;
        let mut files = Vec::new();
        visit(self, root, predicate, limit, 0, scanned, &mut files)?;
        Ok(files)
    }

    fn inventory(&self, files: Vec<PathBuf>, max_bytes: u64) -> Result<Inventory, String> {
        let mut total_bytes = 0_u64;
        let mut entries = Vec::with_capacity(files.len());
        for path in files {
            let relative_path = self.relative_path(&path)?;
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "frontend package input must be a real file: {relative_path}"
                ));
            }
            let remaining = max_bytes
                .checked_sub(total_bytes)
                .filter(|remaining| *remaining > 0)
                .ok_or_else(|| format!("frontend package bytes exceed {max_bytes}"))?;
            let bytes = read_bounded_file(&path, remaining)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            total_bytes = total_bytes
                .checked_add(bytes.len() as u64)
                .filter(|total| *total <= max_bytes)
                .ok_or_else(|| format!("frontend package bytes exceed {max_bytes}"))?;
            entries.push(AssetEntry {
                path: relative_path,
                bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
        }

        let mut aggregate = Context::new(&SHA256);
        for entry in &entries {
            aggregate.update(entry.path.as_bytes());
            aggregate.update(b"\0");
            aggregate.update(entry.sha256.as_bytes());
            aggregate.update(b"\0");
        }
        Ok(Inventory {
            sha256: hex(aggregate.finish().as_ref()),
            total_bytes,
            files: entries,
        })
    }

    fn relative_path(&self, path: &Path) -> Result<String, String> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| format!("frontend package path escapes its root: {}", path.display()))?;
        let value = relative
            .to_str()
            .ok_or_else(|| format!("frontend package path is not UTF-8: {}", path.display()))?
            .replace('\\', "/");
        if value.is_empty() || value == ".." || value.starts_with("../") {
            return Err(format!(
                "frontend package path escapes its root: {}",
                path.display()
            ));
        }
        Ok(value)
    }

    fn read_manifest(&self) -> Result<Option<FrontendPackageManifest>, String> {
        let bytes = match read_bounded_file(&self.manifest_path, MAX_MANIFEST_BYTES) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("failed to read frontend manifest: {error}")),
        };
        match serde_json::from_slice(&bytes) {
            Ok(manifest) => Ok(Some(manifest)),
            Err(_) => Ok(None),
        }
    }

    fn write_manifest(&self, state: &FrontendPackageManifest) -> Result<(), String> {
        if self.read_manifest()?.as_ref() == Some(state) {
            return Ok(());
        }
        let mut bytes = serde_json::to_vec_pretty(state)
            .map_err(|error| format!("failed to encode frontend manifest: {error}"))?;
        bytes.push(b'\n');
        let temporary = self.manifest_path.with_extension(format!(
            "json.tmp-{}-{}",
            process::id(),
            random_hex()?
        ));
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            set_private_mode(&mut options);
            let mut output = options
                .open(&temporary)
                .map_err(|error| format!("failed to create frontend manifest: {error}"))?;
            output
                .write_all(&bytes)
                .and_then(|_| output.sync_all())
                .map_err(|error| format!("failed to persist frontend manifest: {error}"))?;
            replace_file(&temporary, &self.manifest_path)
                .map_err(|error| format!("failed to replace frontend manifest: {error}"))?;
            sync_parent(&self.manifest_path)
                .map_err(|error| format!("failed to sync frontend manifest directory: {error}"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn rebuild(&self) -> Result<(), String> {
        self.run_node_script(
            &self.scripts_root.join("build-language-packs.mjs"),
            "language-pack build",
            Duration::from_secs(120),
        )?;
        let compiler = self.ensure_typescript()?;
        let mut typescript = Command::new(node_program());
        typescript
            .arg(compiler)
            .arg("-p")
            .arg(self.root.join("tsconfig.json"));
        self.run_command(typescript, "TypeScript build", Duration::from_secs(180))?;
        self.run_node_script(
            &self.scripts_root.join("check-language-pack-coverage.mjs"),
            "language-pack verification",
            Duration::from_secs(120),
        )
    }

    fn run_node_script(&self, path: &Path, label: &str, timeout: Duration) -> Result<(), String> {
        let mut command = Command::new(node_program());
        command.arg(path);
        self.run_command(command, label, timeout)
    }

    fn run_command(
        &self,
        mut command: Command,
        label: &str,
        timeout: Duration,
    ) -> Result<(), String> {
        command
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let mut child = command
            .spawn()
            .map_err(|error| format!("{label} failed to start: {error}"))?;
        let started = Instant::now();
        loop {
            match child
                .try_wait()
                .map_err(|error| format!("{label} failed while waiting: {error}"))?
            {
                Some(status) if status.success() => return Ok(()),
                Some(status) => return Err(format!("{label} failed with status {status}")),
                None if started.elapsed() < timeout => thread::sleep(Duration::from_millis(50)),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{label} timed out after {} seconds",
                        timeout.as_secs()
                    ));
                }
            }
        }
    }

    fn ensure_typescript(&self) -> Result<PathBuf, String> {
        let lock: serde_json::Value = serde_json::from_slice(
            &read_bounded_file(&self.root.join("package-lock.json"), MAX_PACKAGE_LOCK_BYTES)
                .map_err(|error| format!("failed to read package-lock.json: {error}"))?,
        )
        .map_err(|error| format!("failed to decode package-lock.json: {error}"))?;
        let locked_version = lock
            .pointer("/packages/node_modules~1typescript/version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "package-lock.json does not pin TypeScript".to_string())?;
        let compiler = self.root.join("node_modules/typescript/bin/tsc");
        let installed_package = self.root.join("node_modules/typescript/package.json");
        if regular_file(&compiler)
            && installed_version(&installed_package).as_deref() == Some(locked_version)
        {
            return Ok(compiler);
        }

        let mut install = Command::new(npm_program());
        install.args(["ci", "--ignore-scripts", "--no-audit", "--no-fund"]);
        self.run_command(
            install,
            "locked frontend dependency install",
            Duration::from_secs(300),
        )?;
        if !regular_file(&compiler)
            || installed_version(&installed_package).as_deref() != Some(locked_version)
        {
            return Err(format!(
                "locked frontend dependency install did not provide TypeScript {locked_version}"
            ));
        }
        Ok(compiler)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FrontendPackageManifest {
    schema: String,
    inputs_sha256: String,
    assets_sha256: String,
    input_file_count: usize,
    asset_file_count: usize,
    input_bytes: u64,
    asset_bytes: u64,
    assets: Vec<AssetEntry>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct AssetEntry {
    path: String,
    bytes: u64,
    sha256: String,
}

struct Inventory {
    sha256: String,
    total_bytes: u64,
    files: Vec<AssetEntry>,
}

struct PackageLock {
    file: Option<File>,
    path: PathBuf,
    token: String,
}

impl PackageLock {
    fn acquire(package_root: &Path) -> Result<Self, String> {
        let key = &sha256_hex(package_root.to_string_lossy().as_bytes())[..24];
        let path = env::temp_dir().join(format!("leserpent-frontend-package-{key}.lock"));
        let started = Instant::now();
        let token = format!("{}:{}", process::id(), random_hex()?);
        loop {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            set_private_mode(&mut options);
            match options.open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file
                        .write_all(token.as_bytes())
                        .and_then(|_| file.sync_all())
                    {
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(format!("failed to persist frontend package lock: {error}"));
                    }
                    return Ok(Self {
                        file: Some(file),
                        path,
                        token,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&path).map_err(|error| {
                        format!("failed to inspect frontend package lock: {error}")
                    })?;
                    if !metadata.is_file() || metadata.file_type().is_symlink() {
                        return Err("frontend package lock must be a real file".to_string());
                    }
                    let age = metadata
                        .modified()
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .unwrap_or_default();
                    if age > STALE_LOCK && metadata.len() <= MAX_LOCK_BYTES {
                        fs::remove_file(&path).map_err(|error| {
                            format!("failed to remove stale frontend package lock: {error}")
                        })?;
                        continue;
                    }
                    if started.elapsed() >= LOCK_WAIT {
                        return Err(format!(
                            "timed out waiting for frontend package lock {}",
                            path.display()
                        ));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    return Err(format!("failed to acquire frontend package lock: {error}"));
                }
            }
        }
    }
}

impl Drop for PackageLock {
    fn drop(&mut self) {
        drop(self.file.take());
        let owned = read_bounded_file(&self.path, MAX_LOCK_BYTES)
            .ok()
            .is_some_and(|bytes| bytes == self.token.as_bytes());
        if owned {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn require_real_directory(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "{label} must be a real directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}

fn installed_version(path: &Path) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(
        &read_bounded_file(path, MAX_INSTALLED_PACKAGE_BYTES).ok()?,
    )
    .ok()?
    .get("version")?
    .as_str()
    .map(str::to_string)
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut file = open_bounded_regular_file_allow_empty(path, max_bytes)?;
    let expected_bytes = file.metadata()?.len();
    let mut bytes = Vec::with_capacity(usize::try_from(expected_bytes).unwrap_or(0));
    file.read_to_end(&mut bytes)?;
    if bytes.len() as u64 != expected_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file changed while reading",
        ));
    }
    Ok(bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(digest(&SHA256, bytes).as_ref())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn random_hex() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "failed to generate frontend package owner token".to_string())?;
    Ok(hex(&bytes))
}

fn node_program() -> &'static str {
    if cfg!(windows) { "node.exe" } else { "node" }
}

fn npm_program() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

#[cfg(unix)]
fn set_private_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_private_mode(_: &mut OpenOptions) {}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "MoveFileExW"]
        fn move_file_ex_w(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        move_file_ex_w(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "leserpent-frontend-package-{label}-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must follow the epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn options_reject_conflicting_modes() {
        let error = Options::parse(["--verify".to_string(), "--force".to_string()].into_iter())
            .unwrap_err();
        assert!(error.contains("mutually exclusive"));
    }

    #[test]
    fn options_reject_duplicate_package_roots() {
        let error = Options::parse(
            [
                "--package-root".to_string(),
                "one".to_string(),
                "--package-root".to_string(),
                "two".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(error.contains("unknown or duplicate argument"));
    }

    #[test]
    fn checked_frontend_manifest_is_current() {
        let message = run(["--verify".to_string()].into_iter())
            .expect("checked frontend package must verify")
            .expect("verification must report a result");
        assert!(message.contains("frontend package verified"));
    }

    #[test]
    fn aggregate_hash_is_stable() {
        assert_eq!(
            sha256_hex(b"leserpent"),
            "b6ad287e4f95f377e38764e104298a06085a2daad6c8db67e4466535d65beaff"
        );
    }

    #[test]
    fn bounded_file_reader_rejects_oversized_input() {
        let path = temp_path("oversized");
        let file = File::create(&path).unwrap();
        file.set_len(257).unwrap();

        assert!(read_bounded_file(&path, 256).is_err());
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_reader_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_path("symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.json");
        let link = root.join("link.json");
        fs::write(&target, "{}").unwrap();
        symlink(&target, &link).unwrap();

        assert!(read_bounded_file(&link, 256).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
