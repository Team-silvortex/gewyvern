use super::*;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gewyvern-certificate-state-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn state_reads_rotation_and_revocation_records() {
    let root = temp_root("scan");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(ROTATION_RECORDS_FILE),
        "identities/prod/runtime.pem\toverdue\t200\t100\t150\trotate now\n",
    )
    .unwrap();
    fs::write(
        root.join(REVOCATION_RECORDS_FILE),
        "trust/anchors/root-ca.pem\ttrust\tdistrusted\t300\t350\tlegacy anchor\n",
    )
    .unwrap();

    let state = runtime_certificate_state_from_layout(RuntimeLayout {
        config_root: PathBuf::from("/tmp/config"),
        data_root: PathBuf::from("/tmp/data"),
        state_root: PathBuf::from("/tmp/state"),
        cache_root: PathBuf::from("/tmp/cache"),
        certificate_root: PathBuf::from("/tmp/config/certificates"),
        trust_root: PathBuf::from("/tmp/config/certificates/trust"),
        authority_root: PathBuf::from("/tmp/config/certificates/authorities"),
        identity_root: PathBuf::from("/tmp/config/certificates/identities"),
        certificate_state_root: root.clone(),
        legacy_root: None,
    });

    assert!(state.rotation_records_exist);
    assert!(state.revocation_records_exist);
    assert_eq!(state.rotation_records.len(), 1);
    assert_eq!(
        state.rotation_records[0].status,
        CertificateRotationStatus::Overdue
    );
    assert_eq!(state.revocation_records.len(), 1);
    assert_eq!(
        state.revocation_records[0].status,
        CertificateRevocationStatus::Distrusted
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn upsert_and_remove_rotation_record_round_trips() {
    let mut records = vec![CertificateRotationRecord {
        relative_path: "identities/a.pem".into(),
        status: CertificateRotationStatus::Due,
        due_unix_ms: Some(10),
        last_rotated_unix_ms: Some(5),
        updated_unix_ms: Some(11),
        note: Some("soon".into()),
    }];
    upsert_rotation_record(
        &mut records,
        CertificateRotationRecord {
            relative_path: "identities/a.pem".into(),
            status: CertificateRotationStatus::Overdue,
            due_unix_ms: Some(20),
            last_rotated_unix_ms: Some(5),
            updated_unix_ms: Some(21),
            note: Some("late".into()),
        },
    );
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].status, CertificateRotationStatus::Overdue);
}

#[test]
fn validate_record_relative_path_rejects_unsafe_inputs() {
    assert!(validate_record_relative_path("identity/../runtime.pem").is_err());
    assert!(validate_record_relative_path("/absolute/path.pem").is_err());
    assert!(validate_record_relative_path("trust/./bad.pem").is_err());
    assert!(validate_record_relative_path("trust/anchors\\root.pem").is_err());
    assert!(validate_record_relative_path("identity/runtime\n.pem").is_err());
    assert!(validate_record_relative_path("identity/runtime.pem").is_ok());
}

#[test]
fn sanitize_record_note_rejects_control_characters() {
    assert_eq!(
        sanitize_record_note(Some("  rotate now ")).unwrap(),
        Some("rotate now".into())
    );
    assert!(sanitize_record_note(Some("rotate\nnow")).is_err());
}

#[test]
fn read_rotation_records_skips_unknown_status_entries() {
    let root = temp_root("invalid-rotation-status");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(ROTATION_RECORDS_FILE);
    fs::write(
        &path,
        "identity/runtime.pem\tinvalid\t10\t20\t30\tnote\n\
         identity/valid.pem\toverdue\t10\t20\t30\tnote\n",
    )
    .unwrap();

    let records = read_rotation_records(&path);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].relative_path, "identity/valid.pem");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn read_revocation_records_skips_unknown_status_and_scope_entries() {
    let root = temp_root("invalid-revocation-status");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(REVOCATION_RECORDS_FILE);
    fs::write(
        &path,
        "trust/anchors/root-ca.pem\ttrust\trevoked\t10\t20\tnote\n\
         trust/anchors/invalid.pem\tinvalid\trevoked\t10\t20\tnote\n",
    )
    .unwrap();

    let records = read_revocation_records(&path);
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].relative_path,
        "trust/anchors/root-ca.pem"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_rotation_records_mark_active_due_and_overdue() {
    let inventory = CertificateInventory {
        root: PathBuf::from("/tmp/certificates"),
        trust_root: PathBuf::from("/tmp/certificates/trust"),
        authority_root: PathBuf::from("/tmp/certificates/authorities"),
        identity_root: PathBuf::from("/tmp/certificates/identities"),
        state_root: PathBuf::from("/tmp/state/certificates"),
        root_exists: true,
        trust_root_exists: true,
        authority_root_exists: true,
        identity_root_exists: true,
        state_root_exists: true,
        require_explicit_remote_trust: false,
        trust_items: vec![CertificateItem {
            relative_path: "anchors/root-ca.pem".into(),
            asset_kind: CertificateAssetKind::BundlePem,
            bytes: 128,
            modified_unix_ms: None,
            validity: Some(crate::certificate_validity::CertificateValidityWindow {
                certificate_count: 1,
                earliest_not_before_unix_ms: Some(100),
                earliest_not_after_unix_ms: Some(90 * 24 * 60 * 60 * 1000),
                latest_not_after_unix_ms: Some(90 * 24 * 60 * 60 * 1000),
            }),
        }],
        authority_items: vec![],
        identity_items: vec![
            CertificateItem {
                relative_path: "prod/runtime.pem".into(),
                asset_kind: CertificateAssetKind::CertificatePem,
                bytes: 256,
                modified_unix_ms: None,
                validity: Some(crate::certificate_validity::CertificateValidityWindow {
                    certificate_count: 1,
                    earliest_not_before_unix_ms: Some(100),
                    earliest_not_after_unix_ms: Some(7 * 24 * 60 * 60 * 1000),
                    latest_not_after_unix_ms: Some(7 * 24 * 60 * 60 * 1000),
                }),
            },
            CertificateItem {
                relative_path: "prod/expired.pem".into(),
                asset_kind: CertificateAssetKind::CertificatePem,
                bytes: 256,
                modified_unix_ms: None,
                validity: Some(crate::certificate_validity::CertificateValidityWindow {
                    certificate_count: 1,
                    earliest_not_before_unix_ms: Some(100),
                    earliest_not_after_unix_ms: Some(5),
                    latest_not_after_unix_ms: Some(5),
                }),
            },
        ],
    };
    let records = generated_rotation_records(&inventory, 10);
    assert!(records.iter().any(|record| {
        record.relative_path == "trust/anchors/root-ca.pem"
            && record.status == CertificateRotationStatus::Active
    }));
    assert!(records.iter().any(|record| {
        record.relative_path == "identity/prod/runtime.pem"
            && record.status == CertificateRotationStatus::Due
    }));
    assert!(records.iter().any(|record| {
        record.relative_path == "identity/prod/expired.pem"
            && record.status == CertificateRotationStatus::Overdue
    }));
}
