use std::collections::BTreeSet;

mod aliases;
mod profiles;
mod registry;

use aliases::split_protocol_alias;
use profiles::{PROTOCOL_PROFILES, find_protocol_profile};
use registry::{
    default_protocol_scan_set_from_registry, resolve_built_in_dsl_path, resolve_registry_alias,
    resolve_registry_entry_alias, scan_protocol_registry, scan_protocol_registry_in,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProtocolProfile {
    pub protocol: String,
    pub entry: String,
    pub dsl_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegistryManifest {
    protocol: String,
    entry: String,
    default: bool,
    aliases: Vec<String>,
    entry_aliases: Vec<String>,
    dsl_path: String,
}

pub fn protocol_dsl_path(protocol: &str, entry: Option<&str>) -> Option<String> {
    resolve_protocol_profile(protocol, entry).map(|profile| profile.dsl_path)
}

pub fn protocol_names() -> Vec<String> {
    if let Some(registry) = scan_protocol_registry() {
        return registry
            .into_iter()
            .map(|manifest| manifest.protocol)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    PROTOCOL_PROFILES
        .iter()
        .map(|profile| profile.name.to_string())
        .collect()
}

pub fn protocol_default_entry(protocol: &str) -> Option<String> {
    if let Some(registry) = scan_protocol_registry() {
        if let Some(manifest) = registry
            .iter()
            .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        {
            return Some(manifest.entry.clone());
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let mut candidates = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(entry) = candidates
            .iter()
            .find(|manifest| manifest.default)
            .or_else(|| candidates.first())
            .map(|manifest| manifest.entry.clone())
        {
            return Some(entry);
        }
    }
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| profile.default_entry.to_string())
}

pub fn protocol_entries(protocol: &str) -> Option<Vec<String>> {
    if let Some(registry) = scan_protocol_registry() {
        if let Some(manifest) = registry
            .iter()
            .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
        {
            let entries = registry
                .iter()
                .filter(|candidate| candidate.protocol == manifest.protocol)
                .map(|candidate| candidate.entry.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            return if entries.is_empty() {
                None
            } else {
                Some(entries)
            };
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let entries = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .map(|manifest| manifest.entry)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            return Some(entries);
        }
    }
    let (protocol_name, _) = split_protocol_alias(protocol);
    find_protocol_profile(protocol_name).map(|profile| {
        profile
            .entries
            .iter()
            .map(|entry| entry.mode.to_string())
            .collect::<Vec<_>>()
    })
}

pub fn resolve_protocol_profile(
    protocol: &str,
    entry: Option<&str>,
) -> Option<ResolvedProtocolProfile> {
    if let Some(registry) = scan_protocol_registry() {
        if entry.is_none() {
            if let Some(manifest) = registry
                .iter()
                .find(|manifest| manifest.aliases.iter().any(|alias| alias == protocol))
            {
                return Some(ResolvedProtocolProfile {
                    protocol: manifest.protocol.clone(),
                    entry: manifest.entry.clone(),
                    dsl_path: manifest.dsl_path.clone(),
                });
            }
        }
        let canonical =
            resolve_registry_alias(&registry, protocol).unwrap_or_else(|| protocol.to_string());
        let mut matches = registry
            .into_iter()
            .filter(|manifest| manifest.protocol == canonical)
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.entry.cmp(&right.entry));
        if let Some(selected) = if let Some(entry) = entry {
            let resolved_entry = resolve_registry_entry_alias(&matches, &canonical, entry)
                .map(str::to_string)
                .unwrap_or_else(|| entry.to_string());
            matches
                .into_iter()
                .find(|manifest| manifest.entry == resolved_entry)
        } else {
            matches
                .iter()
                .find(|manifest| manifest.default)
                .cloned()
                .or_else(|| matches.into_iter().next())
        } {
            return Some(ResolvedProtocolProfile {
                protocol: selected.protocol,
                entry: selected.entry,
                dsl_path: selected.dsl_path,
            });
        }
    }
    let (protocol_name, alias_entry) = split_protocol_alias(protocol);
    let profile = find_protocol_profile(protocol_name)?;
    let resolved_entry = entry.or(alias_entry).unwrap_or(profile.default_entry);
    profile
        .entries
        .iter()
        .find(|item| item.mode == resolved_entry)
        .map(|item| ResolvedProtocolProfile {
            protocol: profile.name.to_string(),
            entry: item.mode.to_string(),
            dsl_path: resolve_built_in_dsl_path(item.dsl_path),
        })
}

pub fn default_protocol_scan_set() -> Vec<ResolvedProtocolProfile> {
    if let Some(registry) = scan_protocol_registry() {
        return default_protocol_scan_set_from_registry(registry);
    }
    PROTOCOL_PROFILES
        .iter()
        .flat_map(|profile| {
            profile
                .entries
                .iter()
                .filter_map(|entry| resolve_protocol_profile(profile.name, Some(entry.mode)))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn default_protocol_scan_set_from_dir(dir: &str) -> Option<Vec<ResolvedProtocolProfile>> {
    let registry = scan_protocol_registry_in(std::path::Path::new(dir))?;
    Some(default_protocol_scan_set_from_registry(registry))
}

#[cfg(test)]
mod tests {
    use super::{protocol_dsl_path, protocol_entries, resolve_built_in_dsl_path};
    use std::fs;
    #[cfg(target_family = "unix")]
    use std::os::unix::fs as unix_fs;
    use std::path::PathBuf;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: String) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    #[test]
    fn http_entry_aliases_resolve_to_canonical_registry_targets() {
        assert_eq!(
            protocol_dsl_path("http", Some("client")),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/request".to_string())
        );
        assert_eq!(
            protocol_dsl_path("http", Some("server")),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/http/response".to_string())
        );
    }

    #[test]
    fn list_entries_prefers_canonical_http_entries() {
        let entries = protocol_entries("http").expect("http entries should resolve");
        assert!(entries.contains(&"request".to_string()));
        assert!(entries.contains(&"response".to_string()));
        assert!(!entries.contains(&"client".to_string()));
        assert!(!entries.contains(&"server".to_string()));
    }

    #[test]
    fn mysql_query_entry_resolves_to_dedicated_query_package() {
        assert_eq!(
            protocol_dsl_path("mysql", Some("query")),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/query".to_string())
        );
    }

    #[test]
    fn rtsp_package_aliases_resolve_to_canonical_entries() {
        assert_eq!(
            protocol_dsl_path("rtsp-options", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/options".to_string())
        );
        assert_eq!(
            protocol_dsl_path("rtsp-describe", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/describe".to_string())
        );
        assert_eq!(
            protocol_dsl_path("rtsp-setup", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/rtsp/setup".to_string())
        );
    }

    #[test]
    fn built_in_dsl_path_falls_back_to_packaged_share_root() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-packaged-dsl-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let dsl_dir = root.join("dsl");
        fs::create_dir_all(&dsl_dir).unwrap();
        let file = dsl_dir.join("http_request_path.gewy");
        fs::write(&file, "template(:http_request_path)\n").unwrap();
        let _guard = EnvGuard::set("GEWY_SHARE_ROOT", root.to_string_lossy().into_owned());

        let resolved = resolve_built_in_dsl_path("/definitely/missing/dsl/http_request_path.gewy");
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(PathBuf::from(resolved), file);
    }

    #[test]
    fn packaged_registry_root_is_used_when_explicitly_set() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-packaged-protocol-registry-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package_dir = root.join("http").join("request");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("gewy.pkg"),
            "name=http_request\nversion=0.10.0\nentry=main.gewy\nregister.protocol=http\nregister.entry=request\nregister.default=true\n",
        )
        .unwrap();
        fs::write(package_dir.join("main.gewy"), "template(:http_request)\n").unwrap();
        let _guard = EnvGuard::set(
            "GEWY_PROTOCOL_REGISTRY_ROOT",
            root.to_string_lossy().into_owned(),
        );

        let resolved = protocol_dsl_path("http", Some("request"));
        let expected = fs::canonicalize(&package_dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(resolved, Some(expected));
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn registry_scan_ignores_symlinked_directories() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-protocol-registry-symlink-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package_dir = root.join("mysql").join("session");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("gewy.pkg"),
            "name=mysql_session\nversion=0.10.0\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
        )
        .unwrap();
        fs::write(package_dir.join("main.gewy"), "template(:mysql_session)\n").unwrap();
        unix_fs::symlink(root.join("mysql"), root.join("mysql-link")).unwrap();

        let targets = super::default_protocol_scan_set_from_dir(root.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].protocol, "mysql");
        assert_eq!(targets[0].entry, "session");
    }
}
