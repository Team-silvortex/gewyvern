use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions, Permissions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VersionAction {
    Check,
    Set {
        version: ProductVersion,
        dry_run: bool,
    },
}

impl VersionAction {
    pub(crate) fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut arguments = arguments.peekable();
        match arguments.next().as_deref() {
            Some("check") => {
                reject_trailing(arguments)?;
                Ok(Self::Check)
            }
            Some("set") => {
                let raw = arguments
                    .next()
                    .ok_or_else(|| "version set requires VERSION".to_string())?;
                let version = ProductVersion::parse(&raw)?;
                let mut dry_run = false;
                for argument in arguments {
                    match argument.as_str() {
                        "--dry-run" if !dry_run => dry_run = true,
                        _ => {
                            return Err(format!(
                                "unknown or repeated version set option `{argument}`"
                            ));
                        }
                    }
                }
                Ok(Self::Set { version, dry_run })
            }
            Some(value) => Err(format!(
                "unknown version action `{value}`; expected check or set"
            )),
            None => Err("version requires check or set".to_string()),
        }
    }

    pub(crate) const fn label(&self) -> &'static str {
        match self {
            Self::Check => "version-check",
            Self::Set { .. } => "version-set",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProductVersion {
    raw: String,
    major: u64,
    minor: u64,
}

impl ProductVersion {
    fn parse(raw: &str) -> Result<Self, String> {
        if raw.is_empty() || !raw.is_ascii() || raw.chars().any(char::is_whitespace) {
            return Err(format!("invalid semantic version `{raw}`"));
        }
        let core_end = raw.find(['-', '+']).unwrap_or(raw.len());
        let core = &raw[..core_end];
        let parts = core.split('.').collect::<Vec<_>>();
        if parts.len() != 3 || parts.iter().any(|part| !valid_numeric_identifier(part)) {
            return Err(format!(
                "version must contain three canonical numeric components; got `{raw}`"
            ));
        }
        validate_suffix(&raw[core_end..], raw)?;
        let major = parts[0]
            .parse()
            .map_err(|_| format!("version major is out of range: `{raw}`"))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| format!("version minor is out of range: `{raw}`"))?;
        let _: u64 = parts[2]
            .parse()
            .map_err(|_| format!("version patch is out of range: `{raw}`"))?;
        Ok(Self {
            raw: raw.to_string(),
            major,
            minor,
        })
    }

    fn active_line(&self) -> String {
        format!("{}.{}.x", self.major, self.minor)
    }
}

impl fmt::Display for ProductVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.raw)
    }
}

fn valid_numeric_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn validate_suffix(suffix: &str, raw: &str) -> Result<(), String> {
    if suffix.is_empty() {
        return Ok(());
    }
    let (prerelease, build) = match suffix.strip_prefix('-') {
        Some(rest) => match rest.split_once('+') {
            Some((prerelease, build)) => (Some(prerelease), Some(build)),
            None => (Some(rest), None),
        },
        None => match suffix.strip_prefix('+') {
            Some(build) => (None, Some(build)),
            None => return Err(format!("invalid semantic version suffix in `{raw}`")),
        },
    };
    for (label, value) in [("prerelease", prerelease), ("build", build)] {
        let Some(value) = value else { continue };
        if value.is_empty()
            || value.split('.').any(|identifier| {
                identifier.is_empty()
                    || !identifier
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    || (label == "prerelease"
                        && identifier.bytes().all(|byte| byte.is_ascii_digit())
                        && identifier.len() > 1
                        && identifier.starts_with('0'))
            })
        {
            return Err(format!("invalid {label} identifiers in `{raw}`"));
        }
    }
    Ok(())
}

pub(crate) fn execute(root: &Path, action: VersionAction) -> Result<(), String> {
    match action {
        VersionAction::Check => {
            let version = root_version(root)?;
            check_surfaces(root, &version)?;
            println!(
                "version surfaces valid: version={} line={}",
                version,
                version.active_line()
            );
            Ok(())
        }
        VersionAction::Set { version, dry_run } => {
            let current = root_version(root)?;
            if current == version {
                check_surfaces(root, &current)?;
                println!("version already current: {current}");
                return Ok(());
            }
            let updates = plan_update(root, &current, &version)?;
            let update_count = updates.len();
            if dry_run {
                println!(
                    "version update dry run: {} -> {} ({} files)",
                    current, version, update_count
                );
                for path in updates.relative_paths() {
                    println!("- {}", path.display());
                }
                return Ok(());
            }
            updates.commit(|| check_surfaces(root, &version))?;
            println!(
                "version updated: {} -> {} ({} files)",
                current, version, update_count
            );
            Ok(())
        }
    }
}

fn root_version(root: &Path) -> Result<ProductVersion, String> {
    let manifest = read_regular_file(&root.join("Cargo.toml"))?;
    let workspace = section(&manifest, "workspace.package")?;
    let raw = quoted_assignment(workspace, "version")?;
    let package = section(&manifest, "package")?;
    if !package
        .lines()
        .any(|line| line == "version.workspace = true")
    {
        return Err("[package] must inherit version.workspace = true".to_string());
    }
    if package.lines().any(|line| line.starts_with("version = \"")) {
        return Err("[package] must not duplicate the workspace version".to_string());
    }
    ProductVersion::parse(raw)
}

fn section<'a>(document: &'a str, name: &str) -> Result<&'a str, String> {
    let header = format!("[{name}]");
    let start = document
        .lines()
        .position(|line| line == header)
        .ok_or_else(|| format!("missing [{name}] section"))?;
    let mut byte_start = 0;
    for line in document.lines().take(start + 1) {
        byte_start += line.len() + 1;
    }
    let tail = &document[byte_start.min(document.len())..];
    let byte_end = tail
        .find("\n[")
        .map(|index| index + 1)
        .unwrap_or(tail.len());
    Ok(&tail[..byte_end])
}

fn quoted_assignment<'a>(section: &'a str, key: &str) -> Result<&'a str, String> {
    let prefix = format!("{key} = \"");
    section
        .lines()
        .find_map(|line| line.strip_prefix(&prefix)?.strip_suffix('"'))
        .ok_or_else(|| format!("missing quoted `{key}` assignment"))
}

fn check_surfaces(root: &Path, version: &ProductVersion) -> Result<(), String> {
    let mut errors = Vec::new();
    let exact = version.to_string();
    let line = version.active_line();

    match root_version(root) {
        Ok(observed) if observed == *version => {}
        Ok(observed) => errors.push(format!(
            "Cargo.toml workspace version is {observed}, expected {version}"
        )),
        Err(error) => errors.push(error),
    }

    check_contains(
        root,
        "Directory.Build.props",
        &format!("<Version>{exact}</Version>"),
        &mut errors,
    );
    for (path, expected) in [
        ("README.md", format!("# gewyvern v{line}\n")),
        ("README.md", format!("project version: `{line}`")),
        (
            "README.md",
            format!("current release line: `v{line}`, with `v{exact}`"),
        ),
        (
            "ROADMAP.md",
            format!("`v{exact}` as the current shared release"),
        ),
        (
            "apps/leserpent/README.md",
            format!("Current shared release: `{exact}`"),
        ),
        (
            "docs/leserpent-2-architecture.md",
            format!("current implementation checkpoint is the shared `v{exact}` release"),
        ),
        (
            "docs/leserpent-2-roadmap.md",
            format!("Current implementation checkpoint: shared release `v{exact}`"),
        ),
        (
            "project/status/catalog.json",
            format!("\"checkpoint\": \"v{exact}-to-v2.0.0-roadmap\""),
        ),
        (
            "docs/development.md",
            format!("cargo dev version set {exact} --dry-run"),
        ),
        (
            "docs/fixtures/leserpent_macos_release_preflight.json",
            format!("\"version\": \"{exact}\""),
        ),
    ] {
        check_contains(root, path, &expected, &mut errors);
    }

    check_lockfile(root, version, &mut errors);
    for path in [
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/packages.development.lock.json",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/packages.lock.json",
    ] {
        check_contains(
            root,
            path,
            &format!("\"Leserpent.RendererCore\": \"[{exact}, )\""),
            &mut errors,
        );
    }

    match read_regular_file(&root.join("scripts/packaging/build_packages.sh")) {
        Ok(script) => {
            for expected in [
                "VERSION=\"$(read_version)\"",
                "RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v${VERSION}}\"",
            ] {
                if !script.contains(expected) {
                    errors.push(format!(
                        "scripts/packaging/build_packages.sh is missing `{expected}`"
                    ));
                }
            }
        }
        Err(error) => errors.push(error),
    }
    match read_regular_file(&root.join("src/validation_harness/container_packaging.rs")) {
        Ok(source) => {
            let expected = "RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v${PRODUCT_VERSION}}\"";
            if !source.contains(expected) {
                errors.push(format!(
                    "src/validation_harness/container_packaging.rs is missing `{expected}`"
                ));
            }
        }
        Err(error) => errors.push(error),
    }
    match read_regular_file(&root.join("src/validation_harness/remote_host.rs")) {
        Ok(source) => {
            for expected in [
                "format!(\"v{}\", env!(\"CARGO_PKG_VERSION\"))",
                "env::var(\"GEWY_RELEASE_LINE\").unwrap_or_else(|_| default_release_line())",
            ] {
                if !source.contains(expected) {
                    errors.push(format!(
                        "src/validation_harness/remote_host.rs is missing `{expected}`"
                    ));
                }
            }
        }
        Err(error) => errors.push(error),
    }

    if let Err(error) = check_active_line_documents(root, version) {
        errors.push(error);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!("version surface drift:\n- {}", errors.join("\n- ")))
    }
}

fn check_contains(root: &Path, relative: &str, expected: &str, errors: &mut Vec<String>) {
    match read_regular_file(&root.join(relative)) {
        Ok(document) if document.contains(expected) => {}
        Ok(_) => errors.push(format!("{relative} is missing `{expected}`")),
        Err(error) => errors.push(error),
    }
}

fn check_lockfile(root: &Path, expected: &ProductVersion, errors: &mut Vec<String>) {
    let lockfile = match read_regular_file(&root.join("Cargo.lock")) {
        Ok(lockfile) => lockfile,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let mut local = 0;
    for block in lockfile.split("\n\n") {
        if !block.starts_with("[[package]]")
            || block.lines().any(|line| line.starts_with("source = "))
        {
            continue;
        }
        local += 1;
        let name = quoted_assignment(block, "name").unwrap_or("<unknown>");
        match quoted_assignment(block, "version") {
            Ok(version) if version == expected.raw => {}
            Ok(version) => errors.push(format!(
                "Cargo.lock local package {name} has version {version}, expected {expected}"
            )),
            Err(error) => errors.push(format!("Cargo.lock local package {name}: {error}")),
        }
    }
    if local == 0 {
        errors.push("Cargo.lock contains no local workspace packages".to_string());
    }
}

fn check_active_line_documents(root: &Path, version: &ProductVersion) -> Result<(), String> {
    let mut drift = Vec::new();
    for path in markdown_files(root)? {
        let document = read_regular_file(&path)?;
        for (major, minor, offset) in release_line_markers(&document) {
            if major != version.major
                || minor == version.minor
                || (version.major == 1 && minor == 0)
            {
                continue;
            }
            let line = document[..offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            drift.push(format!(
                "{}:{line} uses {major}.{minor}.x instead of {}",
                path.strip_prefix(root).unwrap_or(&path).display(),
                version.active_line()
            ));
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "active-line documentation drift: {}",
            drift.join(", ")
        ))
    }
}

fn release_line_markers(document: &str) -> Vec<(u64, u64, usize)> {
    let bytes = document.as_bytes();
    let mut markers = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let number_start = if bytes[index] == b'v' {
            index + 1
        } else {
            index
        };
        if number_start >= bytes.len() || !bytes[number_start].is_ascii_digit() {
            index += 1;
            continue;
        }
        if start > 0 && bytes[start - 1].is_ascii_alphanumeric() {
            index += 1;
            continue;
        }
        let Some((major, after_major)) = parse_number(bytes, number_start) else {
            index += 1;
            continue;
        };
        if bytes.get(after_major) != Some(&b'.') {
            index += 1;
            continue;
        }
        let Some((minor, after_minor)) = parse_number(bytes, after_major + 1) else {
            index += 1;
            continue;
        };
        if bytes.get(after_minor..after_minor + 2) != Some(&b".x"[..])
            || bytes
                .get(after_minor + 2)
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
        {
            index += 1;
            continue;
        }
        markers.push((major, minor, start));
        index = after_minor + 2;
    }
    markers
}

fn parse_number(bytes: &[u8], start: usize) -> Option<(u64, usize)> {
    let mut end = start;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == start {
        return None;
    }
    std::str::from_utf8(&bytes[start..end])
        .ok()?
        .parse()
        .ok()
        .map(|value| (value, end))
}

fn plan_update(
    root: &Path,
    current: &ProductVersion,
    target: &ProductVersion,
) -> Result<UpdateSet, String> {
    let mut updates = UpdateSet::new(root);
    updates.transform("Cargo.toml", |document| {
        replace_section_version(document, "workspace.package", &current.raw, &target.raw)
    })?;
    updates.replace_once(
        "Directory.Build.props",
        &format!("<Version>{current}</Version>"),
        &format!("<Version>{target}</Version>"),
    )?;
    updates.replace_at_least_once("README.md", &format!("v{current}"), &format!("v{target}"))?;
    updates.replace_at_least_once("ROADMAP.md", &format!("v{current}"), &format!("v{target}"))?;
    updates.replace_once(
        "apps/leserpent/README.md",
        &format!("Current shared release: `{current}`"),
        &format!("Current shared release: `{target}`"),
    )?;
    updates.replace_once(
        "docs/leserpent-2-architecture.md",
        &format!("current implementation checkpoint is the shared `v{current}` release"),
        &format!("current implementation checkpoint is the shared `v{target}` release"),
    )?;
    updates.replace_once(
        "docs/leserpent-2-roadmap.md",
        &format!("Current implementation checkpoint: shared release `v{current}`"),
        &format!("Current implementation checkpoint: shared release `v{target}`"),
    )?;
    updates.replace_once(
        "project/status/catalog.json",
        &format!("\"checkpoint\": \"v{current}-to-v2.0.0-roadmap\""),
        &format!("\"checkpoint\": \"v{target}-to-v2.0.0-roadmap\""),
    )?;
    updates.replace_once(
        "docs/development.md",
        &format!("cargo dev version set {current} --dry-run"),
        &format!("cargo dev version set {target} --dry-run"),
    )?;
    updates.replace_once(
        "docs/fixtures/leserpent_macos_release_preflight.json",
        &format!("\"version\": \"{current}\""),
        &format!("\"version\": \"{target}\""),
    )?;
    for path in [
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/packages.development.lock.json",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/packages.lock.json",
    ] {
        updates.replace_once(
            path,
            &format!("\"Leserpent.RendererCore\": \"[{current}, )\""),
            &format!("\"Leserpent.RendererCore\": \"[{target}, )\""),
        )?;
    }
    updates.transform("Cargo.lock", |document| {
        update_local_lock_versions(document, &current.raw, &target.raw)
    })?;

    if current.active_line() != target.active_line() {
        let old_line = current.active_line();
        let new_line = target.active_line();
        let mut replacements = 0;
        for path in markdown_files(root)? {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_path_buf();
            replacements += updates.replace_all_if_present(&relative, &old_line, &new_line)?;
        }
        if replacements == 0 {
            return Err(format!(
                "no active-line documentation used the previous line {old_line}"
            ));
        }
    }
    if updates.is_empty() {
        return Err("version update produced no file changes".to_string());
    }
    Ok(updates)
}

fn replace_section_version(
    document: &str,
    name: &str,
    current: &str,
    target: &str,
) -> Result<String, String> {
    let header = format!("[{name}]");
    let start = document
        .find(&format!("{header}\n"))
        .ok_or_else(|| format!("missing {header}"))?
        + header.len()
        + 1;
    let end = document[start..]
        .find("\n[")
        .map(|offset| start + offset + 1)
        .unwrap_or(document.len());
    let expected = format!("version = \"{current}\"");
    let replacement = format!("version = \"{target}\"");
    let body = &document[start..end];
    if body.matches(&expected).count() != 1 {
        return Err(format!("{header} must contain exactly one `{expected}`"));
    }
    let mut updated = String::with_capacity(document.len());
    updated.push_str(&document[..start]);
    updated.push_str(&body.replacen(&expected, &replacement, 1));
    updated.push_str(&document[end..]);
    Ok(updated)
}

fn update_local_lock_versions(
    document: &str,
    current: &str,
    target: &str,
) -> Result<String, String> {
    let expected = format!("version = \"{current}\"");
    let replacement = format!("version = \"{target}\"");
    let mut changed = 0;
    let mut blocks = Vec::new();
    for block in document.split("\n\n") {
        if block.starts_with("[[package]]")
            && !block.lines().any(|line| line.starts_with("source = "))
        {
            if block.matches(&expected).count() != 1 {
                let name = quoted_assignment(block, "name").unwrap_or("<unknown>");
                return Err(format!(
                    "Cargo.lock local package {name} does not have version {current}"
                ));
            }
            blocks.push(block.replacen(&expected, &replacement, 1));
            changed += 1;
        } else {
            blocks.push(block.to_string());
        }
    }
    if changed == 0 {
        return Err("Cargo.lock contains no local packages to update".to_string());
    }
    let mut updated = blocks.join("\n\n");
    if document.ends_with('\n') && !updated.ends_with('\n') {
        updated.push('\n');
    }
    Ok(updated)
}

struct UpdateSet {
    root: PathBuf,
    changes: BTreeMap<PathBuf, FileChange>,
}

struct FileChange {
    original: String,
    updated: String,
    permissions: Permissions,
}

impl UpdateSet {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            changes: BTreeMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.changes.len()
    }

    fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    fn relative_paths(&self) -> impl Iterator<Item = &Path> {
        self.changes
            .keys()
            .map(|path| path.strip_prefix(&self.root).unwrap_or(path))
    }

    fn transform(
        &mut self,
        relative: impl AsRef<Path>,
        transform: impl FnOnce(&str) -> Result<String, String>,
    ) -> Result<(), String> {
        let path = self.root.join(relative.as_ref());
        let (current, original, permissions) = match self.changes.get(&path) {
            Some(change) => (
                change.updated.clone(),
                change.original.clone(),
                change.permissions.clone(),
            ),
            None => {
                let metadata = regular_metadata(&path)?;
                let original = read_regular_file(&path)?;
                (original.clone(), original, metadata.permissions())
            }
        };
        let updated = transform(&current)?;
        if updated == original {
            self.changes.remove(&path);
        } else {
            self.changes.insert(
                path,
                FileChange {
                    original,
                    updated,
                    permissions,
                },
            );
        }
        Ok(())
    }

    fn replace_once(
        &mut self,
        relative: impl AsRef<Path>,
        current: &str,
        target: &str,
    ) -> Result<(), String> {
        let display = relative.as_ref().display().to_string();
        self.transform(relative, |document| {
            let count = document.matches(current).count();
            if count != 1 {
                return Err(format!(
                    "{display} must contain exactly one `{current}`, observed {count}"
                ));
            }
            Ok(document.replacen(current, target, 1))
        })
    }

    fn replace_at_least_once(
        &mut self,
        relative: impl AsRef<Path>,
        current: &str,
        target: &str,
    ) -> Result<(), String> {
        let display = relative.as_ref().display().to_string();
        self.transform(relative, |document| {
            if !document.contains(current) {
                return Err(format!("{display} is missing `{current}`"));
            }
            Ok(document.replace(current, target))
        })
    }

    fn replace_all_if_present(
        &mut self,
        relative: impl AsRef<Path>,
        current: &str,
        target: &str,
    ) -> Result<usize, String> {
        let path = self.root.join(relative.as_ref());
        let document = self
            .changes
            .get(&path)
            .map(|change| change.updated.clone())
            .unwrap_or(read_regular_file(&path)?);
        let count = document.matches(current).count();
        if count > 0 {
            self.transform(relative, |document| Ok(document.replace(current, target)))?;
        }
        Ok(count)
    }

    fn commit(self, validate: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
        let pid = std::process::id();
        let mut staged = Vec::new();
        for (index, (path, change)) in self.changes.iter().enumerate() {
            if read_regular_file(path)? != change.original {
                cleanup_paths(staged.iter().map(|entry: &StagedFile| &entry.temporary));
                return Err(format!(
                    "version surface changed while the update was being prepared: {}",
                    path.display()
                ));
            }
            let temporary = adjacent_path(path, pid, index, "tmp")?;
            let backup = adjacent_path(path, pid, index, "bak")?;
            if temporary.exists()
                || temporary.is_symlink()
                || backup.exists()
                || backup.is_symlink()
            {
                cleanup_paths(staged.iter().map(|entry: &StagedFile| &entry.temporary));
                return Err(format!(
                    "stale version transaction artifact exists beside {}",
                    path.display()
                ));
            }
            let mut file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => file,
                Err(error) => {
                    cleanup_paths(staged.iter().map(|entry: &StagedFile| &entry.temporary));
                    return Err(format!("failed to stage {}: {error}", path.display()));
                }
            };
            if let Err(error) = file
                .write_all(change.updated.as_bytes())
                .and_then(|_| file.sync_all())
                .and_then(|_| fs::set_permissions(&temporary, change.permissions.clone()))
            {
                let _ = fs::remove_file(&temporary);
                cleanup_paths(staged.iter().map(|entry: &StagedFile| &entry.temporary));
                return Err(format!("failed to stage {}: {error}", path.display()));
            }
            staged.push(StagedFile {
                path: path.clone(),
                temporary,
                backup,
            });
        }

        for (committed, entry) in staged.iter().enumerate() {
            if let Err(error) = fs::rename(&entry.path, &entry.backup) {
                rollback(&staged[..committed]);
                cleanup_paths(staged[committed..].iter().map(|entry| &entry.temporary));
                return Err(format!(
                    "failed to open version transaction for {}: {error}",
                    entry.path.display()
                ));
            }
            if let Err(error) = fs::rename(&entry.temporary, &entry.path) {
                let _ = fs::rename(&entry.backup, &entry.path);
                rollback(&staged[..committed]);
                cleanup_paths(staged[committed + 1..].iter().map(|entry| &entry.temporary));
                return Err(format!(
                    "failed to publish version surface {}: {error}",
                    entry.path.display()
                ));
            }
        }

        if let Err(error) = validate() {
            rollback(&staged);
            return Err(format!(
                "version update rolled back after validation failed: {error}"
            ));
        }
        for entry in &staged {
            fs::remove_file(&entry.backup).map_err(|error| {
                format!(
                    "version update succeeded but backup cleanup failed for {}: {error}",
                    entry.backup.display()
                )
            })?;
        }
        Ok(())
    }
}

struct StagedFile {
    path: PathBuf,
    temporary: PathBuf,
    backup: PathBuf,
}

fn adjacent_path(path: &Path, pid: u32, index: usize, suffix: &str) -> Result<PathBuf, String> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "version surface has an invalid filename: {}",
                path.display()
            )
        })?;
    Ok(path.with_file_name(format!(".{name}.gewyvern-version-{pid}-{index}.{suffix}")))
}

fn rollback(entries: &[StagedFile]) {
    for entry in entries.iter().rev() {
        let _ = fs::remove_file(&entry.path);
        let _ = fs::rename(&entry.backup, &entry.path);
        let _ = fs::remove_file(&entry.temporary);
    }
}

fn cleanup_paths<'a>(paths: impl Iterator<Item = &'a PathBuf>) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn visit(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
            if skipped_version_path(relative) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                visit(root, &path, files)?;
            } else if metadata.is_file()
                && path.extension().and_then(|value| value.to_str()) == Some("md")
            {
                files.push(path);
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn skipped_version_path(relative: &Path) -> bool {
    const SKIPPED_COMPONENTS: &[&str] =
        &[".git", "target", "node_modules", "artifacts", "bin", "obj"];
    if relative.starts_with("docs/history") || relative.starts_with("docs/fixtures") {
        return true;
    }
    relative.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|value| SKIPPED_COMPONENTS.contains(&value))
    })
}

fn regular_metadata(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "version surface must be a regular non-symlink file: {}",
            path.display()
        ));
    }
    Ok(metadata)
}

fn read_regular_file(path: &Path) -> Result<String, String> {
    regular_metadata(path)?;
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn reject_trailing(mut arguments: impl Iterator<Item = String>) -> Result<(), String> {
    match arguments.next() {
        Some(argument) => Err(format!("unexpected argument `{argument}`")),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn semantic_versions_are_canonical_and_support_release_candidates() {
        for version in ["1.17.4", "2.0.0-rc.1", "2.0.0+build.7"] {
            assert_eq!(ProductVersion::parse(version).unwrap().to_string(), version);
        }
        for version in [
            "1.17",
            "01.17.4",
            "1.017.4",
            "1.17.04",
            "1.17.4-01",
            "1.17.4+",
        ] {
            assert!(
                ProductVersion::parse(version).is_err(),
                "accepted {version}"
            );
        }
    }

    #[test]
    fn version_actions_are_explicit() {
        assert_eq!(
            VersionAction::parse(vec!["check".into()].into_iter()).unwrap(),
            VersionAction::Check
        );
        assert_eq!(
            VersionAction::parse(
                vec!["set".into(), "1.17.4".into(), "--dry-run".into()].into_iter()
            )
            .unwrap(),
            VersionAction::Set {
                version: ProductVersion::parse("1.17.4").unwrap(),
                dry_run: true,
            }
        );
        assert!(VersionAction::parse(vec!["set".into()].into_iter()).is_err());
    }

    #[test]
    fn lockfile_update_does_not_touch_registry_packages() {
        let lock = "[[package]]\nname = \"local\"\nversion = \"1.16.0\"\n\n[[package]]\nname = \"remote\"\nversion = \"1.16.0\"\nsource = \"registry+https://example.test\"\n";
        let updated = update_local_lock_versions(lock, "1.16.0", "1.17.4").unwrap();
        assert!(updated.contains("name = \"local\"\nversion = \"1.17.4\""));
        assert!(updated.contains("name = \"remote\"\nversion = \"1.16.0\""));
    }

    #[test]
    fn active_line_scanner_distinguishes_product_lines() {
        assert_eq!(
            release_line_markers("current `v1.17.x`, baseline `1.0.x`")
                .into_iter()
                .map(|(major, minor, _)| (major, minor))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([(1, 0), (1, 17)])
        );
    }

    #[test]
    fn repository_version_surfaces_remain_aligned() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        let version = root_version(root).unwrap();
        check_surfaces(root, &version).unwrap();
    }
}
