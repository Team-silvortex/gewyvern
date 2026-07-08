use super::summary::built_in_protocol_summary;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn selected_protocols() -> &'static [&'static str] {
    &[
        "dns",
        "https",
        "http",
        "http3",
        "syslog",
        "hy2",
        "tls",
        "quic",
        "stun",
        "coap",
        "tftp",
        "ntp",
        "dhcp",
        "dhcpv6",
        "geneve",
        "wireguard",
        "vxlan",
        "mdns",
        "llmnr",
        "nbns",
        "rip",
        "ssdp",
        "postgres",
        "mysql",
        "memcached",
        "amqp",
        "redis",
        "mqtt",
        "kafka",
        "nats",
        "radius",
        "gtpu",
        "l2tp",
        "ftp",
        "pptp",
        "smtp",
        "imap",
        "pop3",
        "kerberos",
        "rtsp",
        "ssh",
        "smb",
        "rdp",
        "socks5",
        "sip",
        "ldap",
        "snmp",
    ]
}

#[derive(Default)]
struct ManifestEntryAliases {
    aliases: BTreeSet<String>,
}

fn manifest_aliases_by_entry(protocol: &str) -> BTreeMap<String, ManifestEntryAliases> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(protocol);
    let mut entries = BTreeMap::<String, ManifestEntryAliases>::new();
    for dir in fs::read_dir(root).expect("protocol dir should exist") {
        let dir = dir.expect("protocol entry should read");
        if !dir.file_type().expect("file type should read").is_dir() {
            continue;
        }
        let manifest_path = dir.path().join("gewy.pkg");
        if !manifest_path.exists() {
            continue;
        }
        let manifest = fs::read_to_string(&manifest_path).expect("manifest should read");
        let mut entry = None::<String>;
        let mut aliases = BTreeSet::<String>::new();
        for raw_line in manifest.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            match key {
                "register.entry" => entry = Some(value.to_string()),
                "register.aliases" | "register.entry_aliases" => {
                    aliases.extend(
                        value
                            .split(',')
                            .map(str::trim)
                            .filter(|item| !item.is_empty())
                            .map(str::to_string),
                    );
                }
                _ => {}
            }
        }
        let entry = entry.expect("manifest should define register.entry");
        entries.entry(entry).or_default().aliases.extend(aliases);
    }
    entries
}

#[test]
fn selected_protocol_fallback_entries_cover_manifest_aliases() {
    for protocol in selected_protocols() {
        let summary =
            built_in_protocol_summary(protocol).unwrap_or_else(|| panic!("{protocol} summary"));
        let summary_entries = summary
            .entries
            .into_iter()
            .map(|entry| {
                (
                    entry.mode,
                    entry.aliases.into_iter().collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let manifest_entries = manifest_aliases_by_entry(protocol);

        for (entry, manifest_aliases) in manifest_entries {
            let built_in_aliases = summary_entries
                .get(&entry)
                .unwrap_or_else(|| panic!("{protocol} missing built-in entry `{entry}`"));
            for alias in manifest_aliases.aliases {
                assert!(
                    built_in_aliases.contains(&alias),
                    "{protocol} built-in entry `{entry}` should cover manifest alias `{alias}`"
                );
            }
        }
    }
}

#[test]
fn selected_protocol_fallback_entries_exist_for_all_manifest_entries() {
    for protocol in selected_protocols() {
        let summary =
            built_in_protocol_summary(protocol).unwrap_or_else(|| panic!("{protocol} summary"));
        let summary_entries = summary
            .entries
            .into_iter()
            .map(|entry| entry.mode)
            .collect::<BTreeSet<_>>();
        let manifest_entries = manifest_aliases_by_entry(protocol)
            .into_keys()
            .collect::<BTreeSet<_>>();

        for entry in manifest_entries {
            assert!(
                summary_entries.contains(&entry),
                "{protocol} built-in summary should include manifest entry `{entry}`"
            );
        }
    }
}
