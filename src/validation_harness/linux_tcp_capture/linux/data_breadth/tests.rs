use super::catalog::{Group, Protocol};
use serde_json::Value;
use std::collections::BTreeSet;

const CAMPAIGNS: [(Group, &str); 6] = [
    (Group::Data, "data_boundaries"),
    (Group::MysqlMemcached, "mysql_memcached"),
    (Group::Messaging, "messaging"),
    (Group::Smtp, "smtp"),
    (Group::Pop3, "pop3"),
    (Group::Rtsp, "rtsp"),
];

#[test]
fn catalog_preserves_all_existing_campaigns_and_package_expectations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut modes = BTreeSet::new();
    let mut protocols = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (group, fixture) in CAMPAIGNS {
        let proof: Value = serde_json::from_slice(
            &std::fs::read(root.join(format!(
                "docs/fixtures/gewyvern_{fixture}_linux_x86_64_20261113.json"
            )))
            .unwrap(),
        )
        .unwrap();
        let run = &proof["runs"][0];
        let (mode, selection) = group.selection();
        assert!(modes.insert(mode));
        assert_eq!(run["mode"], mode);
        assert_eq!(run["checks"], serde_json::json!(group.checks()));
        let phases = run["phases"].as_array().unwrap();
        assert_eq!(phases.len(), selection.len());
        for (protocol, phase) in selection.iter().zip(phases) {
            assert!(protocols.insert(protocol.name()));
            assert_eq!(phase["protocol"], protocol.name());
            assert_eq!(phase["image"], protocol.image());
            assert_eq!(
                phase["selected_query_or_command_count"],
                protocol.selected_command_count()
            );
            let cookies = phase["selected_cookies"].as_array().unwrap();
            assert_eq!(cookies.len(), 2);
            let packages = phase["packages"].as_array().unwrap();
            assert_eq!(packages.len(), protocol.cases().len());
            for (case, package) in protocol.cases().iter().zip(packages) {
                assert!(paths.insert(case.path));
                assert_eq!(package["path"], case.path);
                assert_eq!(package["phase"], case.terminal_phase);
                let mut expected: Vec<_> = cookies
                    .iter()
                    .zip(case.expected_connections)
                    .filter_map(|(cookie, yes)| yes.then_some(cookie.as_u64().unwrap()))
                    .collect();
                expected.sort_unstable();
                assert_eq!(package["matched_cookies"], serde_json::json!(expected));
                assert_eq!(
                    std::fs::read(root.join(case.path)).unwrap(),
                    case.source.as_bytes()
                );
            }
        }
    }
    assert_eq!((modes.len(), protocols.len(), paths.len()), (6, 9, 37));
}

#[test]
fn protocol_cases_are_shared_static_data_and_keep_scope_identity() {
    for (protocol, port) in [
        (Protocol::Postgres, 5432),
        (Protocol::Redis, 6379),
        (Protocol::Mysql, 3306),
        (Protocol::Memcached, 11211),
        (Protocol::Mqtt, 1883),
        (Protocol::Nats, 4222),
        (Protocol::Smtp, 25),
        (Protocol::Pop3, 110),
        (Protocol::Rtsp, 554),
    ] {
        assert_eq!(protocol.port(), port);
        assert!(!protocol.cases().is_empty());
        assert!(std::ptr::eq(protocol.cases(), protocol.cases()));
        assert!(protocol.image().contains("@sha256:"));
    }
}
