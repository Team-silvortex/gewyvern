use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const CRATES: &[&str] = &["leselang-syntax", "leselang-runtime-core", "leselang-hir"];
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
const MAX_FILES: usize = 1024;
const CONSUMER: &str = "leselang-native-source-consumer";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    output: PathBuf,
    verify: bool,
    dry_run: bool,
}

impl Options {
    pub(crate) fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut output = None;
        let mut verify = false;
        let mut dry_run = false;
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--output" if output.is_none() => {
                    output = Some(PathBuf::from(super::next_value(
                        &mut arguments,
                        "--output",
                    )?));
                }
                "--verify" if !verify => verify = true,
                "--dry-run" if !dry_run => dry_run = true,
                _ => return Err(format!("unknown or repeated Leselang option `{argument}`")),
            }
        }
        let output = output.unwrap_or_else(|| PathBuf::from("target/leselang-native-source"));
        if output.as_os_str().is_empty()
            || output.file_name().is_none()
            || output
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("--output must name a fresh directory without dot components".into());
        }
        Ok(Self {
            output,
            verify,
            dry_run,
        })
    }
}

struct Entry {
    bytes: Vec<u8>,
    source: Option<String>,
}

#[derive(Default)]
struct Plan {
    entries: BTreeMap<PathBuf, Entry>,
    bytes: usize,
}

impl Plan {
    fn add(&mut self, path: &Path, bytes: Vec<u8>, source: Option<String>) -> Result<(), String> {
        safe_relative(path)?;
        if self.entries.contains_key(path) {
            return Err(format!("duplicate export path {}", path.display()));
        }
        let total = self
            .bytes
            .checked_add(bytes.len())
            .ok_or("export size overflow")?;
        if bytes.len() as u64 > MAX_FILE_BYTES
            || total > MAX_TOTAL_BYTES
            || self.entries.len() >= MAX_FILES
        {
            return Err("Leselang source export exceeds its bounded inventory".into());
        }
        self.entries.insert(path.into(), Entry { bytes, source });
        self.bytes = total;
        Ok(())
    }

    fn generated(&mut self, path: &str, text: String) -> Result<(), String> {
        self.add(Path::new(path), text.into_bytes(), None)
    }

    fn copy(&mut self, root: &Path, source: &Path, output: &Path) -> Result<(), String> {
        let bytes = read_regular(root, source)?;
        self.add(output, bytes, Some(portable(source)?))
    }

    fn inventory(&self, version: &str, verified: bool) -> Result<Value, String> {
        let files = self
            .entries
            .iter()
            .map(|(path, entry)| {
                Ok(json!({
                    "path": portable(path)?,
                    "bytes": entry.bytes.len(),
                    "sha256": super::sha256_bytes(&entry.bytes),
                    "source": entry.source,
                }))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(json!({
            "schema_version": 1,
            "kind": "leselang-native-source-checkpoint",
            "version": version,
            "verified": verified,
            "complete_vm": false,
            "full_extraction_gate": false,
            "files": files,
        }))
    }
}

fn safe_relative(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("unsafe relative source path {}", path.display()));
    }
    portable(path).map(|_| ())
}

fn portable(path: &Path) -> Result<String, String> {
    path.components()
        .map(|component| {
            let Component::Normal(name) = component else {
                return Err("path is not relative".into());
            };
            let name = name.to_str().ok_or("source name is not UTF-8")?;
            if name.is_empty()
                || name
                    .bytes()
                    .any(|b| !b.is_ascii_alphanumeric() && !b"-_.".contains(&b))
            {
                return Err("source name is not portable".into());
            }
            Ok(name)
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|parts| parts.join("/"))
}

fn read_regular(root: &Path, relative: &Path) -> Result<Vec<u8>, String> {
    safe_relative(relative)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|e| format!("inspect {}: {e}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("source symlink refused: {}", current.display()));
        }
    }
    let metadata =
        fs::metadata(&current).map_err(|e| format!("inspect {}: {e}", current.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "source is not a bounded regular file: {}",
            current.display()
        ));
    }
    let bytes = fs::read(&current).map_err(|e| format!("read {}: {e}", current.display()))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err("source grew beyond its bound".into());
    }
    Ok(bytes)
}

fn read_text(root: &Path, relative: &Path) -> Result<String, String> {
    String::from_utf8(read_regular(root, relative)?)
        .map_err(|_| format!("source is not UTF-8: {}", relative.display()))
}

fn regular_children(root: &Path, relative: &Path) -> Result<Vec<PathBuf>, String> {
    safe_relative(relative)?;
    let directory = root.join(relative);
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|e| format!("inspect {}: {e}", directory.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "source directory symlink/non-directory refused: {}",
            directory.display()
        ));
    }
    let mut children = fs::read_dir(&directory)
        .map_err(|e| format!("read directory {}: {e}", directory.display()))?
        .map(|entry| {
            entry
                .map(|e| relative.join(e.file_name()))
                .map_err(|e| e.to_string())
        })
        .take(MAX_FILES + 1)
        .collect::<Result<Vec<_>, _>>()?;
    if children.len() > MAX_FILES {
        return Err("source directory exceeds its inventory bound".into());
    }
    children.sort();
    Ok(children)
}

fn copy_rust_tree(root: &Path, relative: &Path, plan: &mut Plan) -> Result<(), String> {
    if relative.components().count() > 32 {
        return Err("source directory nesting exceeds its bound".into());
    }
    for child in regular_children(root, relative)? {
        let metadata = fs::symlink_metadata(root.join(&child)).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("source symlink refused: {}", child.display()));
        }
        if metadata.is_dir() {
            copy_rust_tree(root, &child, plan)?;
        } else if metadata.is_file() && child.extension().is_some_and(|e| e == "rs") {
            plan.copy(root, &child, &child)?;
        } else {
            return Err(format!(
                "untracked source asset refused: {}",
                child.display()
            ));
        }
    }
    Ok(())
}

fn module_names(source: &str) -> Result<Vec<&str>, String> {
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(name) = line
            .strip_prefix("pub mod ")
            .or_else(|| line.strip_prefix("mod "))
            .and_then(|s| s.strip_suffix(';'))
        else {
            continue;
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            || !names.insert(name)
        {
            return Err("unsupported or duplicate top-level language module".into());
        }
    }
    Ok(names.into_iter().collect())
}

fn copy_crate(root: &Path, name: &str, plan: &mut Plan) -> Result<(), String> {
    let base = PathBuf::from(format!("crates/{name}"));
    let library = base.join("src/lib.rs");
    let source = read_text(root, &library)?;
    plan.copy(root, &library, &library)?;
    for module in module_names(&source)? {
        let file = base.join(format!("src/{module}.rs"));
        plan.copy(root, &file, &file)?;
        let directory = base.join(format!("src/{module}"));
        if root
            .join(&directory)
            .try_exists()
            .map_err(|e| e.to_string())?
        {
            copy_rust_tree(root, &directory, plan)?;
        }
    }
    for test in regular_children(root, &base.join("tests"))? {
        if test.extension().is_none_or(|e| e != "rs") {
            return Err("untracked native test asset".into());
        }
        let text = read_text(root, &test)?;
        // Product-only suites stay in the reference repository; mixed suites
        // retain their original cfg guards and all native tests unchanged.
        if name == "leselang-hir" && text.starts_with("#![cfg(feature = \"leserpent-profile\")]\n")
        {
            continue;
        }
        plan.copy(root, &test, &test)?;
    }
    if name == "leselang-runtime-core" {
        plan.copy(root, &base.join("README.md"), &base.join("README.md"))?;
    }
    Ok(())
}

struct RegistryLock {
    blocks: BTreeMap<String, String>,
    serde: String,
    serde_json: String,
}

fn assignment(text: &str, key: &str) -> Result<String, String> {
    let prefix = format!("{key} = ");
    let mut matches = text.lines().filter_map(|line| line.strip_prefix(&prefix));
    let value = matches
        .next()
        .ok_or_else(|| format!("lock entry missing {key}"))?;
    if matches.next().is_some() {
        return Err(format!("duplicate lock {key}"));
    }
    serde_json::from_str(value).map_err(|_| format!("lock {key} is not a quoted string"))
}

fn registry_lock(source: &str) -> Result<RegistryLock, String> {
    if !source.lines().any(|line| line == "version = 4") {
        return Err("source export requires lock format 4".into());
    }
    let mut available = BTreeMap::<String, Vec<String>>::new();
    for block in source.split("[[package]]\n").skip(1) {
        let name = assignment(block, "name")?;
        available
            .entry(name)
            .or_default()
            .push(block.trim().to_string());
    }
    let mut pending = vec!["serde".to_string(), "serde_json".to_string()];
    let mut blocks = BTreeMap::new();
    while let Some(name) = pending.pop() {
        if blocks.contains_key(&name) {
            continue;
        }
        let choices = available
            .get(&name)
            .ok_or_else(|| format!("locked registry dependency missing: {name}"))?;
        if choices.len() != 1 {
            return Err(format!("ambiguous locked registry dependency: {name}"));
        }
        let block = &choices[0];
        let source = assignment(block, "source")?;
        if !source.starts_with("registry+") || assignment(block, "checksum")?.len() != 64 {
            return Err("language registry closure is not checksummed".into());
        }
        if let Some((_, dependencies)) = block.split_once("dependencies = [\n") {
            let dependencies = dependencies
                .split_once("\n]")
                .ok_or("unterminated locked dependencies")?
                .0;
            for line in dependencies.lines() {
                let dependency: String = serde_json::from_str(line.trim().trim_end_matches(','))
                    .map_err(|_| "unsupported locked dependency")?;
                if dependency.contains(char::is_whitespace) {
                    return Err(
                        "version-qualified language lock dependencies need explicit review".into(),
                    );
                }
                pending.push(dependency);
            }
        }
        blocks.insert(name, block.clone());
    }
    let serde = assignment(&blocks["serde"], "version")?;
    let serde_json = assignment(&blocks["serde_json"], "version")?;
    Ok(RegistryLock {
        blocks,
        serde,
        serde_json,
    })
}

fn crate_manifest(
    source: &str,
    native_hir: bool,
    version: &str,
    registry: &RegistryLock,
) -> Result<String, String> {
    let mut output = String::new();
    let mut features = false;
    let mut removed_contract = false;
    for line in source.lines() {
        if line.starts_with('[') {
            features = native_hir && line == "[features]";
        }
        if features {
            if !line.is_empty()
                && line != "[features]"
                && line != "default = [\"leserpent-profile\"]"
                && line != "leserpent-profile = [\"dep:leselang-host-contract\"]"
            {
                return Err("native HIR feature boundary changed; review source export".into());
            }
            continue;
        }
        if native_hir
            && line
                == "leselang-host-contract = { path = \"../leselang-host-contract\", optional = true }"
        {
            removed_contract = true;
            continue;
        }
        let mapped = match line {
            "version.workspace = true" => format!("version = {version:?}\npublish = false"),
            "edition.workspace = true" => "edition = \"2024\"".into(),
            "license.workspace = true" => "license = \"MIT\"".into(),
            "serde.workspace = true" => format!(
                "serde = {{ version = \"={}\", features = [\"derive\"] }}",
                registry.serde
            ),
            "serde_json.workspace = true" => format!("serde_json = \"={}\"", registry.serde_json),
            _ => line.to_string(),
        };
        if mapped.contains(".workspace") || mapped.contains("leselang-host-contract") {
            return Err("unsupported source manifest inheritance or product dependency".into());
        }
        output.push_str(&mapped);
        output.push('\n');
    }
    if native_hir {
        if !removed_contract {
            return Err("optional reference contract manifest boundary changed".into());
        }
        // Recognize the original unreachable reference guards without creating
        // an enableable Cargo feature or rewriting shared production/test code.
        output.push_str("\n[lints.rust]\nunexpected_cfgs = { level = \"deny\", check-cfg = ['cfg(feature, values(\"leserpent-profile\"))'] }\n");
    }
    Ok(output)
}

fn lock_text(registry: &RegistryLock, version: &str, consumer: bool) -> String {
    let mut blocks = registry.blocks.clone();
    for name in CRATES {
        let mut names = if *name == "leselang-hir" {
            vec!["leselang-runtime-core", "leselang-syntax", "serde"]
        } else {
            vec!["serde"]
        };
        if !consumer {
            names.push("serde_json");
        }
        let dependencies = names
            .into_iter()
            .map(|name| format!(" {name:?},\n"))
            .collect::<String>();
        blocks.insert(
            (*name).into(),
            format!("name = {name:?}\nversion = {version:?}\ndependencies = [\n{dependencies}]"),
        );
    }
    if consumer {
        blocks.insert(CONSUMER.into(), format!("name = {CONSUMER:?}\nversion = {version:?}\ndependencies = [\n \"leselang-hir\",\n \"leselang-runtime-core\",\n \"leselang-syntax\",\n \"serde\",\n \"serde_json\",\n]"));
    }
    let mut output = "# This file is generated from the source repository's locked registry closure.\nversion = 4\n".to_string();
    for block in blocks.values() {
        output.push_str(&format!("\n[[package]]\n{block}\n"));
    }
    output
}

fn prepare(root: &Path) -> Result<(Plan, String), String> {
    let manifest = read_text(root, Path::new("Cargo.toml"))?;
    let workspace = manifest
        .split_once("[workspace.package]\n")
        .ok_or("workspace metadata missing")?
        .1
        .split_once("\n[")
        .ok_or("workspace metadata unterminated")?
        .0;
    if assignment(workspace, "edition")? != "2024" || assignment(workspace, "license")? != "MIT" {
        return Err("language edition/license export boundary needs review".into());
    }
    let version = super::version::root_version(root)?.to_string();
    let registry = registry_lock(&read_text(root, Path::new("Cargo.lock"))?)?;
    let mut plan = Plan::default();
    plan.copy(root, Path::new("LICENSE"), Path::new("LICENSE"))?;
    for name in CRATES {
        copy_crate(root, name, &mut plan)?;
        let path = format!("crates/{name}/Cargo.toml");
        let source = read_text(root, Path::new(&path))?;
        plan.generated(
            &path,
            crate_manifest(&source, *name == "leselang-hir", &version, &registry)?,
        )?;
    }
    plan.generated("Cargo.toml", "[workspace]\nmembers = [\"crates/leselang-syntax\", \"crates/leselang-runtime-core\", \"crates/leselang-hir\"]\nexclude = [\"consumer\"]\nresolver = \"3\"\n\n[profile.lean]\ninherits = \"dev\"\ndebug = 0\nincremental = false\n".into())?;
    plan.generated("Cargo.lock", lock_text(&registry, &version, false))?;
    let consumer_manifest = format!(
        "[workspace]\n\n[package]\nname = {CONSUMER:?}\nversion = {version:?}\nedition = \"2024\"\nlicense = \"MIT\"\npublish = false\nautotests = false\n\n[dependencies]\nleselang-hir = {{ path = \"../crates/leselang-hir\" }}\nleselang-runtime-core = {{ path = \"../crates/leselang-runtime-core\" }}\nleselang-syntax = {{ path = \"../crates/leselang-syntax\" }}\nserde = {{ version = \"={}\", features = [\"derive\"] }}\nserde_json = \"={}\"\n\n[profile.lean]\ninherits = \"dev\"\ndebug = 0\nincremental = false\n",
        registry.serde, registry.serde_json
    );
    let mut consumer_manifest = consumer_manifest;
    for (name, original) in [
        (
            "checkpoint_recovery",
            "crates/leselang-runtime-core/tests/checkpoint.rs",
        ),
        (
            "continuation_frame",
            "crates/leselang-runtime-core/tests/continuation.rs",
        ),
        (
            "group_graph",
            "crates/leselang-runtime-core/tests/group_graph.rs",
        ),
        (
            "recovery_fences",
            "crates/leselang-runtime-core/tests/recovery_fence.rs",
        ),
        (
            "successor_chain",
            "crates/leselang-runtime-core/tests/successor_chain.rs",
        ),
        (
            "durable_control",
            "crates/leselang-hir/tests/durable_control.rs",
        ),
        (
            "captured_reentry",
            "crates/leselang-hir/tests/captured_reentry.rs",
        ),
        (
            "owned_scope",
            "crates/leselang-runtime-core/tests/owned_scope.rs",
        ),
        (
            "control_session",
            "crates/leselang-hir/tests/effect_session.rs",
        ),
        ("return_stack", "crates/leselang-hir/tests/effect_stack.rs"),
        (
            "prepared_gui_host",
            "crates/leselang-hir/tests/prepared_gui_host.rs",
        ),
    ] {
        consumer_manifest.push_str(&format!(
            "\n[[test]]\nname = {name:?}\npath = \"tests/{name}.rs\"\n"
        ));
        plan.copy(
            root,
            Path::new(original),
            Path::new(&format!("consumer/tests/{name}.rs")),
        )?;
    }
    plan.generated("consumer/Cargo.toml", consumer_manifest)?;
    plan.generated("consumer/Cargo.lock", lock_text(&registry, &version, true))?;
    plan.generated(".github/workflows/ci.yml", CI.into())?;
    plan.generated("README.md", README.into())?;
    Ok((plan, version))
}

fn output_path(root: &Path, requested: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let output = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let parent = output.parent().ok_or("output has no parent")?;
    let parent =
        fs::canonicalize(parent).map_err(|e| format!("output parent must already exist: {e}"))?;
    let output = parent.join(output.file_name().ok_or("output has no name")?);
    for protected in [
        "crates",
        ".git",
        ".agents",
        ".codex",
        ".github",
        ".cargo",
        "src",
        "tests",
        "docs",
        "project",
        "apps",
        "scripts",
        "packaging",
    ] {
        if output.starts_with(root.join(protected)) {
            return Err("source/protected-tree export destination refused".into());
        }
    }
    match fs::symlink_metadata(&output) {
        Ok(_) => Err("export destination already exists; use a fresh directory".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(output),
        Err(error) => Err(format!("inspect output: {error}")),
    }
}

pub(crate) fn export(root: &Path, options: &Options) -> Result<PathBuf, String> {
    export_with_verifier(root, options, verify)
}

fn export_with_verifier(
    root: &Path,
    options: &Options,
    verifier: impl FnOnce(&Path, &Path) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let output = output_path(&root, &options.output)?;
    let (plan, version) = prepare(&root)?;
    if options.dry_run {
        println!(
            "Leselang source export dry run: version={version}, files={}, bytes={}, verify={}, output={}",
            plan.entries.len(),
            plan.bytes,
            options.verify,
            output.display()
        );
        return Ok(output);
    }
    let stage = output.with_file_name(format!(".leselang-source-{}.partial", unique_suffix()?));
    fs::create_dir(&stage).map_err(|e| format!("create private source stage: {e}"))?;
    let result = (|| {
        for (relative, entry) in &plan.entries {
            let path = stage.join(relative);
            fs::create_dir_all(path.parent().ok_or("planned file has no parent")?)
                .map_err(|e| e.to_string())?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| format!("create {}: {e}", path.display()))?;
            file.write_all(&entry.bytes).map_err(|e| e.to_string())?;
        }
        if options.verify {
            verifier(&stage, &root.join("target"))?;
        }
        check_inventory(&stage, &plan)?;
        let metadata = serde_json::to_vec_pretty(&plan.inventory(&version, options.verify)?)
            .map_err(|e| e.to_string())?;
        fs::write(stage.join("export-manifest.json"), metadata).map_err(|e| e.to_string())?;
        publish(&stage, &output)?;
        Ok(())
    })();
    if let Err(error) = result {
        if let Err(cleanup) = fs::remove_dir_all(&stage) {
            return Err(format!("{error}; source stage cleanup failed: {cleanup}"));
        }
        return Err(error);
    }
    println!(
        "Leselang native source checkpoint: files={}, bytes={}, verified={}",
        plan.entries.len(),
        plan.bytes,
        options.verify
    );
    Ok(output)
}

fn unique_suffix() -> Result<String, String> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    // Wall clocks can have coarse resolution; concurrent exports need local identity too.
    let sequence = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| "source temporary identity sequence exhausted")?;
    Ok(format!("{}-{nonce}-{sequence}", std::process::id()))
}

fn publish(stage: &Path, output: &Path) -> Result<(), String> {
    let name = output
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("export name is not UTF-8")?;
    let lock = output.with_file_name(format!(".{name}.leselang-source.lock"));
    let reservation = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|e| format!("reserve export publication: {e}"))?;
    drop(reservation);
    // A sibling reservation serializes cooperating exporters without requiring
    // rename-over-an-empty-directory, which is not portable to Windows.
    let result = (|| {
        match fs::symlink_metadata(output) {
            Ok(_) => return Err("export destination appeared during preparation".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("inspect export destination: {error}")),
        }
        fs::rename(stage, output).map_err(|e| format!("publish source export: {e}"))
    })();
    let cleanup =
        fs::remove_file(&lock).map_err(|e| format!("remove publication reservation: {e}"));
    result.and(cleanup)
}

fn check_inventory(root: &Path, plan: &Plan) -> Result<(), String> {
    fn files(
        root: &Path,
        relative: &Path,
        observed: &mut BTreeSet<PathBuf>,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 32 {
            return Err("export inventory nesting exceeds its bound".into());
        }
        for entry in fs::read_dir(root.join(relative)).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = relative.join(entry.file_name());
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                files(root, &path, observed, depth + 1)?;
            } else if kind.is_file() && observed.len() < MAX_FILES {
                safe_relative(&path)?;
                observed.insert(path);
            } else {
                return Err(
                    "export inventory contains a symlink, special file or too many files".into(),
                );
            }
        }
        Ok(())
    }
    let mut observed = BTreeSet::new();
    files(root, Path::new(""), &mut observed, 0)?;
    if observed != plan.entries.keys().cloned().collect() {
        return Err("export inventory contains unplanned or missing files".into());
    }
    for (path, entry) in &plan.entries {
        if read_regular(root, path)? != entry.bytes {
            return Err(format!("export content changed: {}", path.display()));
        }
    }
    Ok(())
}

fn cargo(directory: &Path, target: &Path, args: &[&str]) -> Result<(), String> {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(program)
        .args(args)
        .current_dir(directory)
        .env("CARGO_TARGET_DIR", target)
        .status()
        .map_err(|e| format!("run isolated Cargo: {e}"))?;
    if !status.success() {
        return Err(format!(
            "isolated Cargo {} failed: {status}",
            args.join(" ")
        ));
    }
    Ok(())
}

fn verify(directory: &Path, target: &Path) -> Result<(), String> {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let metadata = Command::new(program)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--no-deps",
        ])
        .current_dir(directory)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .map_err(|e| e.to_string())?;
    if !metadata.status.success() {
        return Err(format!(
            "isolated metadata failed: {}",
            String::from_utf8_lossy(&metadata.stderr)
        ));
    }
    let metadata: Value = serde_json::from_slice(&metadata.stdout).map_err(|e| e.to_string())?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("isolated packages missing")?;
    if packages.len() != CRATES.len() {
        return Err("isolated workspace contains non-language packages".into());
    }
    for package in packages {
        let name = package["name"]
            .as_str()
            .ok_or("isolated package name missing")?;
        if !CRATES.contains(&name) {
            return Err("non-language package in source export".into());
        }
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("isolated manifest missing")?,
        );
        if !manifest.starts_with(directory) {
            return Err("isolated manifest escapes source export".into());
        }
        for dependency in package["dependencies"]
            .as_array()
            .ok_or("isolated dependencies missing")?
        {
            if let Some(path) = dependency["path"].as_str() {
                let path = fs::canonicalize(path).map_err(|e| e.to_string())?;
                if !path.starts_with(directory) {
                    return Err("isolated dependency escapes source export".into());
                }
            } else if !["serde", "serde_json"].contains(&dependency["name"].as_str().unwrap_or(""))
            {
                return Err("unexpected non-language dependency in source export".into());
            }
        }
    }
    cargo(
        directory,
        target,
        &[
            "test",
            "--workspace",
            "--all-features",
            "--locked",
            "--offline",
            "--profile",
            "lean",
        ],
    )?;
    cargo(
        directory,
        target,
        &[
            "clippy",
            "--workspace",
            "--all-features",
            "--all-targets",
            "--locked",
            "--offline",
            "--profile",
            "lean",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo(
        &directory.join("consumer"),
        target,
        &["test", "--locked", "--offline", "--profile", "lean"],
    )?;
    cargo(
        &directory.join("consumer"),
        target,
        &[
            "clippy",
            "--all-targets",
            "--locked",
            "--offline",
            "--profile",
            "lean",
            "--",
            "-D",
            "warnings",
        ],
    )
}

const README: &str = "# Leselang Native Source Checkpoint\n\nThis self-contained MIT source workspace contains the production syntax, runtime\nfoundation and host-defined HIR/in-memory sessions, plus their unchanged native\ntests. Production Rust files are copied byte-for-byte, not independently reimplemented.\nCrate manifests own explicit metadata and pin the original locked registry closure.\nNo Gewyvern/Leserpent product crate, account service, generated asset or SQLite is\nrequired. The reference profile is not an enableable Cargo feature here; all-features\nbuilds stay native. Product-only tests and reference implementation files stay in\nthe original repository. Mixed tests retain their original unreachable cfg guards.\n\nUse Rust 1.98.0 with Clippy. From this directory run:\n\n```text\ncargo test --workspace --all-features --locked --offline --profile lean\ncargo clippy --workspace --all-features --all-targets --locked --offline --profile lean -- -D warnings\ncargo test --manifest-path consumer/Cargo.toml --locked --offline --profile lean\ncargo clippy --manifest-path consumer/Cargo.toml --all-targets --locked --offline --profile lean -- -D warnings\n```\n\nOffline builds require the locked registry dependencies to be cached first. The\nstandalone CI fetches that closure, then uses locked/offline verification. The\nconsumer is a separate workspace and exercises owned cleanup, accepted reply\nsessions, nested returns and a headless GUI adapter without monorepo source paths.\nSaved-frame and parsed durable-control consumer models recover original budgets,\ncheckpoint watermarks and settled success/refusal receipts through the native\ncompiler and walker. The shared saved-body HIR entry also drives real reference\nVM reentry; restoration hooks cannot replace its saved body or evaluator.\nThe shared flat-group kernel also checks complete original member order and\nidentities before native checkpoint commit; real VM admission and SQLite recovery\nuse it while native authority, reservations and dispatch/terminal state stay host-owned.\nShared recovery fences reject duplicate numeric reservations, check the original\nallocation watermark before imported-identity repair, and gate completion receipts\nbefore native admission. Exact numeric and GUI-local proofs run unchanged in the\nconsumer; real SQLite recovery uses the same fences before watermark repair/commit.\nShared adjacent-chain inspection retains original row order and move-only frames,\nbounded before hooks and released before handoff. SQLite keeps native pair policy\nand frozen result-decode-before-next-request fault order; no saved body is evaluated.\nReceipt authenticity, raw-result correspondence, settlement and replay remain native.\nThey do not provide a production storage backend, crash-atomic\nexternal-effect transaction or generic saved-graph interpreter.\nThese are protocol tests, not actual GUI windows or native callback sandboxing.\n\n`export-manifest.json` records relative paths, exact SHA-256 and source provenance.\n`verified` is true only after isolated workspace/consumer tests and strict lints\npass and original inventory bytes remain unchanged. It is not a signature.\n\nThis is a source checkpoint, NOT the complete independent durable VM or full\nrepository extraction gate. Durable recovery, reference-adapter/wire/auth/GUI parity\nand release publication remain separate gates in the original repository. Packages\nare deliberately marked publish=false. FFI and an OS shell remain deferred.\n";

const CI: &str = "name: Leselang Native Source\non: [push, pull_request, workflow_dispatch]\npermissions:\n  contents: read\njobs:\n  native:\n    runs-on: ubuntu-latest\n    timeout-minutes: 15\n    steps:\n      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1\n      - run: rustup toolchain install 1.98.0 --profile minimal --component clippy,rustfmt\n      - run: cargo +1.98.0 fetch --locked\n      - run: cargo +1.98.0 fetch --manifest-path consumer/Cargo.toml --locked\n      - run: cargo +1.98.0 fmt --all -- --check\n      - run: cargo +1.98.0 test --workspace --all-features --locked --offline --profile lean\n      - run: cargo +1.98.0 clippy --workspace --all-features --all-targets --locked --offline --profile lean -- -D warnings\n      - run: cargo +1.98.0 test --manifest-path consumer/Cargo.toml --locked --offline --profile lean\n      - run: cargo +1.98.0 clippy --manifest-path consumer/Cargo.toml --all-targets --locked --offline --profile lean -- -D warnings\n";

#[cfg(test)]
mod tests {
    use super::*;

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!("leselang-source-test-{}", unique_suffix().unwrap()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn concurrent_temporary_directory_owners_have_distinct_native_identities() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let mut workers = vec![];
        for _ in 0..16 {
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                (0..16).map(|_| Temp::new()).collect::<Vec<_>>()
            }));
        }
        let mut owners = vec![];
        for worker in workers {
            owners.extend(worker.join().unwrap());
        }
        let paths = owners.iter().map(|owner| &owner.0).collect::<BTreeSet<_>>();
        assert_eq!(paths.len(), 256);
        assert!(paths.iter().all(|path| path.is_dir()));
        drop(paths);
        drop(owners);
    }

    #[test]
    fn parser_rejects_overwrite_shortcuts_repeated_options_and_dot_paths() {
        let options =
            Options::parse(["--verify", "--dry-run"].into_iter().map(str::to_owned)).unwrap();
        assert!(options.verify && options.dry_run);
        for args in [
            vec!["--force"],
            vec!["--verify", "--verify"],
            vec!["--output"],
            vec!["--output", "../other"],
            vec!["--output", "."],
            vec!["--output", ""],
        ] {
            assert!(Options::parse(args.into_iter().map(str::to_owned)).is_err());
        }
    }

    #[test]
    fn plan_rejects_nonportable_traversal_duplicates_and_oversized_bytes() {
        let mut plan = Plan::default();
        for path in ["../outside", "/absolute", "private name", ""] {
            assert!(plan.generated(path, "x".into()).is_err());
        }
        plan.generated("src/lib.rs", "x".into()).unwrap();
        assert!(plan.generated("src/lib.rs", "replacement".into()).is_err());
        assert!(
            plan.generated("large.rs", "x".repeat(MAX_FILE_BYTES as usize + 1))
                .is_err()
        );
        assert_eq!(plan.entries.len(), 1);
    }

    #[test]
    fn reachable_module_selection_omits_reference_only_files_without_source_rewriting() {
        let source = "pub mod syntax;\nmod private;\n#[cfg(feature = \"leserpent-profile\")]\ninclude!(\"leserpent_profile.rs\");\nmod tests {\n}\n";
        assert_eq!(module_names(source).unwrap(), ["private", "syntax"]);
        assert!(module_names("pub mod bad-name;\n").is_err());
        assert!(module_names("mod same;\nmod same;").is_err());
    }

    #[test]
    fn manifests_remove_workspace_and_optional_product_requirements_without_enabling_reference_cfg()
    {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let registry = registry_lock(&read_text(root, Path::new("Cargo.lock")).unwrap()).unwrap();
        let source = read_text(root, Path::new("crates/leselang-hir/Cargo.toml")).unwrap();
        let manifest = crate_manifest(&source, true, "9.8.7", &registry).unwrap();
        for absent in [
            ".workspace",
            "[features]",
            "leselang-host-contract",
            "dep:leselang-host-contract",
            "default =",
        ] {
            assert!(!manifest.contains(absent));
        }
        assert!(manifest.contains("version = \"9.8.7\""));
        assert!(manifest.contains("publish = false"));
        assert!(manifest.contains("check-cfg = ['cfg(feature, values(\"leserpent-profile\"))']"));
        let changed = source.replace("[features]", "[features]\nnew-language-feature = []");
        assert!(crate_manifest(&changed, true, "9.8.7", &registry).is_err());
    }

    #[test]
    fn registry_lock_keeps_only_exact_transitive_checksummed_language_dependencies() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let source = read_text(root, Path::new("Cargo.lock")).unwrap();
        let registry = registry_lock(&source).unwrap();
        for block in registry.blocks.values() {
            assert!(source.contains(block));
        }
        let lock = lock_text(&registry, "9.8.7", true);
        for forbidden in [
            "gewyvern",
            "leserpent",
            "rusqlite",
            "silvortex",
            "host-contract",
        ] {
            assert!(!lock.contains(forbidden));
        }
        assert!(lock.contains(CONSUMER));
        assert!(registry_lock("version = 3\n").is_err());
        assert!(registry_lock("version = 4\n").is_err());
    }

    #[test]
    fn output_preflight_never_replaces_existing_files_or_directories_and_protects_sources() {
        let temp = Temp::new();
        fs::write(temp.0.join("existing"), "keep").unwrap();
        fs::create_dir(temp.0.join("crates")).unwrap();
        assert!(output_path(&temp.0, Path::new("existing")).is_err());
        assert!(output_path(&temp.0, Path::new("crates/new")).is_err());
        assert!(output_path(&temp.0, Path::new("missing/new")).is_err());
        assert_eq!(fs::read_to_string(temp.0.join("existing")).unwrap(), "keep");
        assert_eq!(
            output_path(&temp.0, Path::new("new")).unwrap(),
            temp.0.join("new")
        );
    }

    #[cfg(unix)]
    #[test]
    fn source_file_and_directory_symlinks_are_refused_before_copying() {
        use std::os::unix::fs::symlink;
        let temp = Temp::new();
        fs::write(temp.0.join("real.rs"), "private").unwrap();
        symlink("real.rs", temp.0.join("link.rs")).unwrap();
        symlink(".", temp.0.join("linked")).unwrap();
        assert!(read_regular(&temp.0, Path::new("link.rs")).is_err());
        assert!(read_regular(&temp.0, Path::new("linked/real.rs")).is_err());
        assert!(regular_children(&temp.0, Path::new("linked")).is_err());
    }

    #[test]
    fn source_plan_copies_production_and_consumer_proofs_byte_for_byte_and_records_no_absolute_paths()
     {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let (plan, version) = prepare(root).unwrap();
        for path in [
            "crates/leselang-hir/src/effect_session.rs",
            "crates/leselang-runtime-core/src/owned_scope.rs",
            "crates/leselang-runtime-core/src/checkpoint.rs",
            "crates/leselang-runtime-core/src/continuation.rs",
            "crates/leselang-runtime-core/src/group_graph.rs",
            "crates/leselang-runtime-core/src/recovery_fence.rs",
            "crates/leselang-runtime-core/src/successor_chain.rs",
            "consumer/tests/checkpoint_recovery.rs",
            "consumer/tests/continuation_frame.rs",
            "consumer/tests/group_graph.rs",
            "consumer/tests/recovery_fences.rs",
            "consumer/tests/successor_chain.rs",
            "consumer/tests/durable_control.rs",
            "consumer/tests/captured_reentry.rs",
            "consumer/tests/prepared_gui_host.rs",
        ] {
            let entry = &plan.entries[Path::new(path)];
            assert_eq!(
                entry.bytes,
                read_regular(root, Path::new(entry.source.as_deref().unwrap())).unwrap()
            );
        }
        for path in [
            "crates/leselang-hir/src/leserpent_profile.rs",
            "crates/leselang-hir/src/host_call.rs",
            "crates/leselang-hir/tests/control_flow.rs",
        ] {
            assert!(!plan.entries.contains_key(Path::new(path)));
        }
        let inventory = plan.inventory(&version, false).unwrap();
        assert_eq!(inventory["verified"], false);
        assert_eq!(inventory["full_extraction_gate"], false);
        assert_eq!(inventory["complete_vm"], false);
        assert!(!inventory.to_string().contains(root.to_str().unwrap()));
    }

    #[test]
    fn unplanned_verifier_files_cannot_enter_a_verified_source_inventory() {
        let temp = Temp::new();
        let options = Options {
            output: temp.0.join("export"),
            verify: true,
            dry_run: false,
        };
        let error = export_with_verifier(source_root(), &options, |stage, _| {
            fs::write(stage.join("unplanned.txt"), "not part of export").unwrap();
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("unplanned or missing files"));
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    fn source_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
    }

    #[test]
    fn dry_run_plans_real_sources_without_files_or_verifier_calls() {
        let temp = Temp::new();
        let options = Options {
            output: temp.0.join("export"),
            verify: true,
            dry_run: true,
        };
        export_with_verifier(source_root(), &options, |_, _| {
            panic!("dry run invoked verifier")
        })
        .unwrap();
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    #[test]
    fn failed_isolated_verification_leaves_no_published_directory_or_private_stage() {
        let temp = Temp::new();
        let options = Options {
            output: temp.0.join("export"),
            verify: true,
            dry_run: false,
        };
        let error = export_with_verifier(source_root(), &options, |stage, _| {
            assert!(stage.join("consumer/Cargo.lock").is_file());
            assert!(!stage.join("export-manifest.json").exists());
            Err("isolated proof refused".into())
        })
        .unwrap_err();
        assert!(error.contains("isolated proof refused"));
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    #[test]
    fn changed_production_bytes_after_verification_are_not_sealed_or_published() {
        let temp = Temp::new();
        let options = Options {
            output: temp.0.join("export"),
            verify: true,
            dry_run: false,
        };
        let error = export_with_verifier(source_root(), &options, |stage, _| {
            fs::write(
                stage.join("crates/leselang-hir/src/effect_session.rs"),
                "changed",
            )
            .unwrap();
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("export content changed"));
        assert_eq!(fs::read_dir(&temp.0).unwrap().count(), 0);
    }

    #[test]
    fn publication_serializes_exporters_and_never_replaces_preexisting_destination() {
        let temp = Temp::new();
        let stage = temp.0.join("stage");
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("content"), "source").unwrap();
        let output = temp.0.join("export");
        fs::write(&output, "existing").unwrap();
        assert!(publish(&stage, &output).is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "existing");
        assert!(stage.exists());
        fs::remove_file(&output).unwrap();
        let lock = temp.0.join(".export.leselang-source.lock");
        fs::write(&lock, "other exporter").unwrap();
        assert!(publish(&stage, &output).is_err());
        assert_eq!(fs::read_to_string(&lock).unwrap(), "other exporter");
        fs::remove_file(lock).unwrap();
        publish(&stage, &output).unwrap();
        assert!(!stage.exists());
        assert_eq!(
            fs::read_to_string(output.join("content")).unwrap(),
            "source"
        );
    }
}
