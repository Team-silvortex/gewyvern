use super::{
    MAX_GEWYLANG_PACKAGE_MANIFEST_BYTES, PACKAGE_MANIFEST_FILE, SyntaxError as DslError,
    read_bounded_utf8_file,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageContext {
    pub(crate) package_scope: String,
    pub(crate) source_root: PathBuf,
    pub(crate) package_root: PathBuf,
    pub(crate) entry_file: String,
    pub(crate) dependencies: Arc<BTreeMap<String, PathBuf>>,
}

pub(super) struct ResolvedInclude {
    pub(super) path: PathBuf,
    pub(super) package_scope: String,
    pub(super) package_root: PathBuf,
    pub(super) dependency: Option<String>,
}

impl PackageContext {
    pub(super) fn for_include(&self, include: &ResolvedInclude) -> Self {
        let source_root = include
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| include.package_root.clone());
        Self {
            package_scope: include.package_scope.clone(),
            source_root,
            package_root: include.package_root.clone(),
            entry_file: include.path.to_string_lossy().into_owned(),
            dependencies: Arc::clone(&self.dependencies),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageManifest {
    name: String,
    version: String,
    entry: String,
    sources: BTreeMap<String, PathBuf>,
    dependencies: BTreeMap<String, PathBuf>,
}

pub fn build_lockfile(path: &str) -> Result<String, DslError> {
    let path = Path::new(path);
    let manifest_path = if path.is_dir() {
        path.join(PACKAGE_MANIFEST_FILE)
    } else {
        path.to_path_buf()
    };
    let manifest = read_package_manifest(&manifest_path)?;
    let package_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let package_root = canonicalize_existing_path(&package_root)?;

    let mut lines = vec![
        format!("name={}", manifest.name),
        format!("version={}", manifest.version),
        format!("entry={}", manifest.entry),
        format!("root={}", package_root.to_string_lossy()),
    ];

    for (name, source_root) in manifest.sources {
        lines.push(format!("source.{name}={}", source_root.to_string_lossy()));
    }
    for (name, dep_root) in manifest.dependencies {
        lines.push(format!("dep.{name}={}", dep_root.to_string_lossy()));
    }
    Ok(lines.join("\n") + "\n")
}

pub(super) fn resolve_package_context(path: &str) -> Result<PackageContext, DslError> {
    let path = Path::new(path);
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "gewy")
    {
        let entry_path = canonicalize_existing_path(path)?;
        let root_dir = entry_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(PackageContext {
            package_scope: "standalone".to_string(),
            source_root: root_dir.clone(),
            package_root: root_dir,
            entry_file: entry_path.to_string_lossy().into_owned(),
            dependencies: Arc::new(BTreeMap::new()),
        });
    }

    let manifest_path = if path.is_dir() {
        path.join(PACKAGE_MANIFEST_FILE)
    } else if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == PACKAGE_MANIFEST_FILE)
    {
        path.to_path_buf()
    } else {
        let entry_path = canonicalize_existing_path(path)?;
        return Ok(PackageContext {
            package_scope: "standalone".to_string(),
            entry_file: entry_path.to_string_lossy().into_owned(),
            source_root: entry_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            package_root: entry_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            dependencies: Arc::new(BTreeMap::new()),
        });
    };

    let manifest = read_package_manifest(&manifest_path)?;
    let package_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let package_root = canonicalize_existing_path(&package_root)?;
    let entry_path = canonicalize_existing_path(&package_root.join(manifest.entry))?;
    ensure_within_canonical_root(&entry_path, &package_root)?;
    Ok(PackageContext {
        package_scope: manifest.name,
        entry_file: entry_path.to_string_lossy().into_owned(),
        source_root: package_root.clone(),
        package_root,
        dependencies: Arc::new(manifest.dependencies),
    })
}

fn read_package_manifest(path: &Path) -> Result<PackageManifest, DslError> {
    let input = read_bounded_utf8_file(
        path,
        MAX_GEWYLANG_PACKAGE_MANIFEST_BYTES,
        "gewylang package manifest",
    )?;
    let mut name = None;
    let mut version = None;
    let mut entry = None;
    let mut sources = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    let manifest_root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()))?;
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "name" => set_manifest_field(&mut name, "name", value)?,
            "version" => set_manifest_field(&mut version, "version", value)?,
            "entry" => set_manifest_field(&mut entry, "entry", value)?,
            source if source.starts_with("source.") => {
                let source_name = source["source.".len()..].trim().to_string();
                if sources.contains_key(&source_name) {
                    return Err(duplicate_manifest_field(source));
                }
                let source_path = canonicalize_existing_path(&manifest_root.join(value))?;
                sources.insert(source_name, source_path);
            }
            dep if dep.starts_with("dep.") => {
                let dependency_name = dep["dep.".len()..].trim().to_string();
                if dependencies.contains_key(&dependency_name) {
                    return Err(duplicate_manifest_field(dep));
                }
                let dep_path = resolve_dependency_root(&manifest_root, &sources, &value)?;
                dependencies.insert(dependency_name, dep_path);
            }
            _ => {}
        }
    }

    Ok(PackageManifest {
        name: name.ok_or(DslError::MissingField("name"))?,
        version: version.ok_or(DslError::MissingField("version"))?,
        entry: entry.ok_or(DslError::MissingField("entry"))?,
        sources,
        dependencies,
    })
}

fn set_manifest_field(
    slot: &mut Option<String>,
    field: &'static str,
    value: String,
) -> Result<(), DslError> {
    if slot.replace(value).is_some() {
        return Err(duplicate_manifest_field(field));
    }
    Ok(())
}

fn duplicate_manifest_field(field: &str) -> DslError {
    DslError::InvalidValue(format!(
        "duplicate gewylang package manifest field '{field}'"
    ))
}

fn resolve_dependency_root(
    manifest_root: &Path,
    sources: &BTreeMap<String, PathBuf>,
    value: &str,
) -> Result<PathBuf, DslError> {
    if let Some(rest) = value.strip_prefix("source:") {
        let (source_name, package_path) = rest.split_once('/').ok_or_else(|| {
            DslError::InvalidValue(format!(
                "invalid source dependency '{value}', expected source:<name>/<package>"
            ))
        })?;
        let source_root = sources.get(source_name).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown package source '{source_name}'"))
        })?;
        let resolved = canonicalize_existing_path(&source_root.join(package_path))?;
        ensure_within_canonical_root(&resolved, source_root)?;
        return Ok(resolved);
    }
    canonicalize_existing_path(&manifest_root.join(value))
}

pub(super) fn resolve_include(
    package: &PackageContext,
    include: &str,
) -> Result<ResolvedInclude, DslError> {
    if let Some((dep_name, file)) = include.split_once(':') {
        let dep_root = package.dependencies.get(dep_name).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown package dependency '{dep_name}'"))
        })?;
        let resolved = canonicalize_existing_path(&dep_root.join(file))?;
        ensure_within_canonical_root(&resolved, dep_root)?;
        return Ok(ResolvedInclude {
            path: resolved,
            package_scope: dep_name.to_string(),
            package_root: dep_root.clone(),
            dependency: Some(dep_name.to_string()),
        });
    }
    let resolved = canonicalize_existing_path(&package.source_root.join(include))?;
    ensure_within_canonical_root(&resolved, &package.package_root)?;
    Ok(ResolvedInclude {
        path: resolved,
        package_scope: package.package_scope.clone(),
        package_root: package.package_root.clone(),
        dependency: None,
    })
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, DslError> {
    path.canonicalize()
        .map_err(|err| DslError::Io(err.to_string()))
}

fn ensure_within_canonical_root(path: &Path, root: &Path) -> Result<(), DslError> {
    debug_assert!(root.is_absolute());
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(DslError::InvalidValue(format!(
            "included path '{}' escapes package root '{}'",
            path.display(),
            root.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "gewy-package-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn source_dependency_rejects_escape_from_named_source_root() {
        let root = temp_root("source-escape");
        let app = root.join("app");
        let registry = root.join("registry");
        let outside = root.join("outside_dep");
        fs::create_dir_all(&app).unwrap();
        fs::create_dir_all(&registry).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(
            app.join(PACKAGE_MANIFEST_FILE),
            format!(
                "name=source_escape\nversion=0.1.0\nentry=main.gewy\nsource.local={}\ndep.std=source:local/../outside_dep\n",
                registry.to_string_lossy()
            ),
        )
        .unwrap();

        let err = read_package_manifest(&app.join(PACKAGE_MANIFEST_FILE)).unwrap_err();
        let _ = fs::remove_dir_all(root);
        assert!(
            format!("{err:?}").contains("escapes package root"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn package_manifest_rejects_duplicate_critical_fields() {
        let root = temp_root("duplicate-field");
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join(PACKAGE_MANIFEST_FILE);
        fs::write(
            &manifest,
            "name=first\nname=second\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();

        let err = read_package_manifest(&manifest).unwrap_err();
        let _ = fs::remove_dir_all(root);
        assert!(
            format!("{err:?}").contains("duplicate gewylang package manifest field 'name'"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn package_manifest_rejects_oversized_input() {
        let root = temp_root("oversized");
        fs::create_dir_all(&root).unwrap();
        let manifest = root.join(PACKAGE_MANIFEST_FILE);
        fs::write(
            &manifest,
            vec![b'x'; MAX_GEWYLANG_PACKAGE_MANIFEST_BYTES + 1],
        )
        .unwrap();

        let err = read_package_manifest(&manifest).unwrap_err();
        let _ = fs::remove_dir_all(root);
        assert_eq!(
            err,
            DslError::InvalidValue(format!(
                "gewylang package manifest exceeds {MAX_GEWYLANG_PACKAGE_MANIFEST_BYTES} bytes"
            ))
        );
    }

    #[cfg(unix)]
    #[test]
    fn package_manifest_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("actual.pkg");
        let manifest = root.join(PACKAGE_MANIFEST_FILE);
        fs::write(&target, "name=test\nversion=0.1.0\nentry=main.gewy\n").unwrap();
        symlink(&target, &manifest).unwrap();

        let err = read_package_manifest(&manifest).unwrap_err();
        let _ = fs::remove_dir_all(root);
        assert!(
            format!("{err:?}").contains("is not a regular file"),
            "unexpected error: {err:?}"
        );
    }
}
