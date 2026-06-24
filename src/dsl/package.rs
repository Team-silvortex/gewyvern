use super::{DslError, PACKAGE_MANIFEST_FILE};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PackageContext {
    pub(super) package_scope: String,
    pub(super) root_dir: PathBuf,
    pub(super) entry_file: String,
    pub(super) dependencies: BTreeMap<String, PathBuf>,
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
        let entry_file = path.to_string_lossy().into_owned();
        let root_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(PackageContext {
            package_scope: "standalone".to_string(),
            root_dir,
            entry_file,
            dependencies: BTreeMap::new(),
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
        return Ok(PackageContext {
            package_scope: "standalone".to_string(),
            entry_file: path.to_string_lossy().into_owned(),
            root_dir: path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            dependencies: BTreeMap::new(),
        });
    };

    let manifest = read_package_manifest(&manifest_path)?;
    let package_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let package_root = canonicalize_existing_path(&package_root)?;
    let entry_path = canonicalize_existing_path(&package_root.join(manifest.entry))?;
    ensure_within_root(&entry_path, &package_root)?;
    Ok(PackageContext {
        package_scope: manifest.name,
        entry_file: entry_path.to_string_lossy().into_owned(),
        root_dir: package_root,
        dependencies: manifest.dependencies,
    })
}

fn read_package_manifest(path: &Path) -> Result<PackageManifest, DslError> {
    let input = fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))?;
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
            "name" => name = Some(value),
            "version" => version = Some(value),
            "entry" => entry = Some(value),
            source if source.starts_with("source.") => {
                let source_path = canonicalize_existing_path(&manifest_root.join(value))?;
                sources.insert(source["source.".len()..].trim().to_string(), source_path);
            }
            dep if dep.starts_with("dep.") => {
                let dep_path = resolve_dependency_root(&manifest_root, &sources, &value)?;
                dependencies.insert(dep["dep.".len()..].trim().to_string(), dep_path);
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
        ensure_within_root(&resolved, source_root)?;
        return Ok(resolved);
    }
    canonicalize_existing_path(&manifest_root.join(value))
}

pub(super) fn resolve_include_path(
    package: &PackageContext,
    include: &str,
) -> Result<PathBuf, DslError> {
    if let Some((dep_name, file)) = include.split_once(':') {
        let dep_root = package.dependencies.get(dep_name).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown package dependency '{dep_name}'"))
        })?;
        let resolved = canonicalize_existing_path(&dep_root.join(file))?;
        ensure_within_root(&resolved, dep_root)?;
        return Ok(resolved);
    }
    let resolved = canonicalize_existing_path(&package.root_dir.join(include))?;
    ensure_within_root(&resolved, &package.root_dir)?;
    Ok(resolved)
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, DslError> {
    path.canonicalize()
        .map_err(|err| DslError::Io(err.to_string()))
}

fn ensure_within_root(path: &Path, root: &Path) -> Result<(), DslError> {
    let normalized_root = root
        .canonicalize()
        .map_err(|err| DslError::Io(err.to_string()))?;
    if path.starts_with(&normalized_root) {
        Ok(())
    } else {
        Err(DslError::InvalidValue(format!(
            "included path '{}' escapes package root '{}'",
            path.display(),
            normalized_root.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
