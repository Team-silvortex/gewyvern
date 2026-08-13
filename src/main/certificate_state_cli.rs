use gewyvern::certificate_state::{
    CertificateMaterialScope, CertificateRevocationStatus, CertificateRotationStatus,
    material_scope_label, remove_revocation_record, remove_rotation_record,
    revocation_status_label, rotation_status_label, runtime_certificate_state,
    sync_rotation_records_from_inventory, write_revocation_record, write_rotation_record,
};

use crate::data_api::render_runtime_certificate_state_json;

pub(crate) fn try_run_certificate_state_command(args: &[String]) -> Option<i32> {
    if args.first().map(String::as_str) != Some("certificate-state") {
        return None;
    }
    match run_certificate_state_command(&args[1..]) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            Some(0)
        }
        Err(message) => {
            eprintln!("{message}");
            Some(2)
        }
    }
}

fn run_certificate_state_command(args: &[String]) -> Result<String, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(certificate_state_usage().into());
    };
    match command {
        "show" => {
            let json = args.iter().skip(1).any(|arg| arg == "--json");
            if json {
                Ok(render_runtime_certificate_state_json())
            } else {
                let state = runtime_certificate_state();
                Ok(render_state_text(&state))
            }
        }
        "sync-rotation" => {
            let json = args.iter().skip(1).any(|arg| arg == "--json");
            let report = sync_rotation_records_from_inventory()?;
            if json {
                Ok(format!(
                    "{{\"surface\":\"runtime_certificate_rotation_sync\",\"updated_record_count\":{},\"active_count\":{},\"due_count\":{},\"overdue_count\":{}}}",
                    report.updated_record_count,
                    report.active_count,
                    report.due_count,
                    report.overdue_count
                ))
            } else {
                Ok(format!(
                    "rotation sync complete: updated={} active={} due={} overdue={}",
                    report.updated_record_count,
                    report.active_count,
                    report.due_count,
                    report.overdue_count
                ))
            }
        }
        "set-rotation" => {
            let options = parse_kv_args(&args[1..])?;
            let relative_path = required_option(&options, "--path")?;
            let status = parse_rotation_status(required_option(&options, "--status")?)?;
            write_rotation_record(
                relative_path,
                status,
                parse_optional_i128_option(&options, "--due-unix-ms")?,
                parse_optional_i128_option(&options, "--last-rotated-unix-ms")?,
                parse_optional_i128_option(&options, "--updated-unix-ms")?,
                optional_option(&options, "--note"),
            )?;
            Ok(format!(
                "rotation record upserted: {} ({})",
                relative_path,
                rotation_status_label(status)
            ))
        }
        "clear-rotation" => {
            let options = parse_kv_args(&args[1..])?;
            let relative_path = required_option(&options, "--path")?;
            let removed = remove_rotation_record(relative_path)?;
            Ok(if removed {
                format!("rotation record removed: {relative_path}")
            } else {
                format!("no rotation record found for: {relative_path}")
            })
        }
        "set-revocation" => {
            let options = parse_kv_args(&args[1..])?;
            let relative_path = required_option(&options, "--path")?;
            let scope = parse_material_scope(required_option(&options, "--scope")?)?;
            let status = parse_revocation_status(required_option(&options, "--status")?)?;
            write_revocation_record(
                relative_path,
                scope,
                status,
                parse_optional_i128_option(&options, "--effective-unix-ms")?,
                parse_optional_i128_option(&options, "--updated-unix-ms")?,
                optional_option(&options, "--note"),
            )?;
            Ok(format!(
                "revocation record upserted: {} ({}/{})",
                relative_path,
                material_scope_label(scope),
                revocation_status_label(status)
            ))
        }
        "clear-revocation" => {
            let options = parse_kv_args(&args[1..])?;
            let relative_path = required_option(&options, "--path")?;
            let removed = remove_revocation_record(relative_path)?;
            Ok(if removed {
                format!("revocation record removed: {relative_path}")
            } else {
                format!("no revocation record found for: {relative_path}")
            })
        }
        "--help" | "-h" | "help" => Err(certificate_state_usage().into()),
        _ => Err(format!(
            "unknown certificate-state command '{}'\n\n{}",
            command,
            certificate_state_usage()
        )),
    }
}

fn parse_kv_args(args: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut parsed = Vec::new();
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        if flag == "--json" {
            continue;
        }
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument '{flag}'"));
        }
        let value = iter
            .next()
            .ok_or_else(|| format!("missing value for '{flag}'"))?;
        parsed.push((flag.clone(), value.clone()));
    }
    Ok(parsed)
}

fn required_option<'a>(options: &'a [(String, String)], key: &str) -> Result<&'a str, String> {
    options
        .iter()
        .find(|(flag, _)| flag == key)
        .map(|(_, value)| value.as_str())
        .ok_or_else(|| format!("missing required option '{key}'"))
}

fn optional_option<'a>(options: &'a [(String, String)], key: &str) -> Option<&'a str> {
    options
        .iter()
        .find(|(flag, _)| flag == key)
        .map(|(_, value)| value.as_str())
}

fn parse_optional_i128_option(
    options: &[(String, String)],
    key: &str,
) -> Result<Option<i128>, String> {
    match optional_option(options, key) {
        Some(value) => value
            .parse::<i128>()
            .map(Some)
            .map_err(|_| format!("invalid integer for '{key}': {value}")),
        None => Ok(None),
    }
}

fn parse_rotation_status(value: &str) -> Result<CertificateRotationStatus, String> {
    match value {
        "active" => Ok(CertificateRotationStatus::Active),
        "due" => Ok(CertificateRotationStatus::Due),
        "overdue" => Ok(CertificateRotationStatus::Overdue),
        "error" => Ok(CertificateRotationStatus::Error),
        _ => Err(format!("unsupported rotation status '{value}'")),
    }
}

fn parse_revocation_status(value: &str) -> Result<CertificateRevocationStatus, String> {
    match value {
        "revoked" => Ok(CertificateRevocationStatus::Revoked),
        "distrusted" => Ok(CertificateRevocationStatus::Distrusted),
        "cleared" => Ok(CertificateRevocationStatus::Cleared),
        _ => Err(format!("unsupported revocation status '{value}'")),
    }
}

fn parse_material_scope(value: &str) -> Result<CertificateMaterialScope, String> {
    match value {
        "trust" => Ok(CertificateMaterialScope::Trust),
        "authority" => Ok(CertificateMaterialScope::Authority),
        "identity" => Ok(CertificateMaterialScope::Identity),
        "other" => Ok(CertificateMaterialScope::Other),
        _ => Err(format!("unsupported certificate material scope '{value}'")),
    }
}

fn render_state_text(state: &gewyvern::certificate_state::CertificateRuntimeState) -> String {
    let mut rendered = String::new();
    rendered.push_str("Certificate State\n");
    rendered.push_str(&format!("root: {}\n", state.root.display()));
    rendered.push_str(&format!(
        "rotation records: {} ({})\n",
        state.rotation_records.len(),
        state.rotation_records_path.display()
    ));
    rendered.push_str(&format!(
        "revocation records: {} ({})\n",
        state.revocation_records.len(),
        state.revocation_records_path.display()
    ));
    if !state.rotation_records.is_empty() {
        rendered.push_str("\nRotations:\n");
        for record in &state.rotation_records {
            rendered.push_str(&format!(
                "- {} [{}]\n",
                record.relative_path,
                rotation_status_label(record.status)
            ));
        }
    }
    if !state.revocation_records.is_empty() {
        rendered.push_str("\nRevocations:\n");
        for record in &state.revocation_records {
            rendered.push_str(&format!(
                "- {} [{}/{}]\n",
                record.relative_path,
                material_scope_label(record.scope),
                revocation_status_label(record.status)
            ));
        }
    }
    rendered
}

fn certificate_state_usage() -> &'static str {
    "usage:
  gewyvern certificate-state show [--json]
  gewyvern certificate-state sync-rotation [--json]
  gewyvern certificate-state set-rotation --path <relative-path> --status <active|due|overdue|error> [--due-unix-ms <ms>] [--last-rotated-unix-ms <ms>] [--updated-unix-ms <ms>] [--note <text>]
  gewyvern certificate-state clear-rotation --path <relative-path>
  gewyvern certificate-state set-revocation --path <relative-path> --scope <trust|authority|identity|other> --status <revoked|distrusted|cleared> [--effective-unix-ms <ms>] [--updated-unix-ms <ms>] [--note <text>]
  gewyvern certificate-state clear-revocation --path <relative-path>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::process::Stdio;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn parse_rotation_set_command_requires_path_and_status() {
        let result = run_certificate_state_command(&["set-rotation".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_show_json_command() {
        let result = run_certificate_state_command(&["show".into(), "--json".into()]);
        assert!(
            result
                .unwrap()
                .contains("\"surface\":\"runtime_certificate_state\"")
        );
    }

    #[test]
    fn sync_rotation_then_show_json_lists_generated_records() {
        let _guard = env_test_lock().lock().unwrap();
        let root = temp_root("sync-show");
        let cert_root = root.join("certs");
        let trust_root = cert_root.join("trust");
        let authority_root = cert_root.join("authorities");
        let identity_root = cert_root.join("identities");
        let state_root = root.join("state").join("certificates");
        fs::create_dir_all(trust_root.join("anchors")).unwrap();
        fs::create_dir_all(&authority_root).unwrap();
        fs::create_dir_all(identity_root.join("prod")).unwrap();
        fs::create_dir_all(&state_root).unwrap();

        write_test_certificate(
            &trust_root.join("anchors").join("root-ca.pem"),
            &trust_root.join("anchors").join("root-ca.key"),
            90,
            "gw-root-ca",
        );
        write_test_certificate(
            &identity_root.join("prod").join("runtime.pem"),
            &identity_root.join("prod").join("runtime.key"),
            1,
            "gw-runtime",
        );

        let _env = TestEnvGuard::set(&[
            (
                "GEWY_CERTIFICATE_ROOT",
                cert_root.to_string_lossy().as_ref(),
            ),
            ("GEWY_TRUST_ROOT", trust_root.to_string_lossy().as_ref()),
            (
                "GEWY_AUTHORITY_ROOT",
                authority_root.to_string_lossy().as_ref(),
            ),
            (
                "GEWY_IDENTITY_ROOT",
                identity_root.to_string_lossy().as_ref(),
            ),
            (
                "GEWY_CERTIFICATE_STATE_ROOT",
                state_root.to_string_lossy().as_ref(),
            ),
        ]);

        let sync =
            run_certificate_state_command(&["sync-rotation".into(), "--json".into()]).unwrap();
        assert!(sync.contains("\"updated_record_count\":2"));
        assert!(sync.contains("\"active_count\":1"));
        assert!(sync.contains("\"due_count\":1"));

        let shown = run_certificate_state_command(&["show".into(), "--json".into()]).unwrap();
        assert!(shown.contains("\"rotation_records_exist\":true"));
        assert!(shown.contains("\"relative_path\":\"identity/prod/runtime.pem\""));
        assert!(shown.contains("\"status\":\"due\""));
        assert!(shown.contains("\"relative_path\":\"trust/anchors/root-ca.pem\""));
        assert!(shown.contains("\"status\":\"active\""));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_revocation_then_show_and_clear_json_round_trips() {
        let _guard = env_test_lock().lock().unwrap();
        let root = temp_root("revoke-show");
        let cert_root = root.join("certs");
        let trust_root = cert_root.join("trust");
        let authority_root = cert_root.join("authorities");
        let identity_root = cert_root.join("identities");
        let state_root = root.join("state").join("certificates");
        fs::create_dir_all(trust_root.join("anchors")).unwrap();
        fs::create_dir_all(&authority_root).unwrap();
        fs::create_dir_all(&identity_root).unwrap();
        fs::create_dir_all(&state_root).unwrap();

        let _env = TestEnvGuard::set(&[
            (
                "GEWY_CERTIFICATE_ROOT",
                cert_root.to_string_lossy().as_ref(),
            ),
            ("GEWY_TRUST_ROOT", trust_root.to_string_lossy().as_ref()),
            (
                "GEWY_AUTHORITY_ROOT",
                authority_root.to_string_lossy().as_ref(),
            ),
            (
                "GEWY_IDENTITY_ROOT",
                identity_root.to_string_lossy().as_ref(),
            ),
            (
                "GEWY_CERTIFICATE_STATE_ROOT",
                state_root.to_string_lossy().as_ref(),
            ),
        ]);

        let upsert = run_certificate_state_command(&[
            "set-revocation".into(),
            "--path".into(),
            "trust/anchors/root-ca.pem".into(),
            "--scope".into(),
            "trust".into(),
            "--status".into(),
            "distrusted".into(),
            "--effective-unix-ms".into(),
            "12345".into(),
            "--updated-unix-ms".into(),
            "23456".into(),
            "--note".into(),
            "manual distrust".into(),
        ])
        .unwrap();
        assert!(upsert.contains("revocation record upserted"));
        assert!(upsert.contains("trust/distrusted"));

        let shown = run_certificate_state_command(&["show".into(), "--json".into()]).unwrap();
        assert!(shown.contains("\"revocation_records_exist\":true"));
        assert!(shown.contains("\"relative_path\":\"trust/anchors/root-ca.pem\""));
        assert!(shown.contains("\"scope\":\"trust\""));
        assert!(shown.contains("\"status\":\"distrusted\""));
        assert!(shown.contains("\"active_revocations\":1"));

        let cleared = run_certificate_state_command(&[
            "clear-revocation".into(),
            "--path".into(),
            "trust/anchors/root-ca.pem".into(),
        ])
        .unwrap();
        assert!(cleared.contains("revocation record removed"));

        let shown_after_clear =
            run_certificate_state_command(&["show".into(), "--json".into()]).unwrap();
        assert!(shown_after_clear.contains("\"revocation_records\":[]"));
        assert!(shown_after_clear.contains("\"active_revocations\":0"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_rotation_rejects_relative_path_traversal() {
        let _guard = env_test_lock().lock().unwrap();
        let root = temp_root("rotation-invalid-path");
        let state_root = root.join("state").join("certificates");
        fs::create_dir_all(&state_root).unwrap();

        let _env = TestEnvGuard::set(&[(
            "GEWY_CERTIFICATE_STATE_ROOT",
            state_root.to_string_lossy().as_ref(),
        )]);

        let result = run_certificate_state_command(&[
            "set-rotation".into(),
            "--path".into(),
            "identity/../runtime.pem".into(),
            "--status".into(),
            "due".into(),
        ]);

        let error = result.expect_err("command should fail");
        assert!(error.contains("relative path contains invalid segment"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn set_revocation_rejects_note_control_characters() {
        let _guard = env_test_lock().lock().unwrap();
        let root = temp_root("revocation-invalid-note");
        let state_root = root.join("state").join("certificates");
        fs::create_dir_all(&state_root).unwrap();

        let _env = TestEnvGuard::set(&[(
            "GEWY_CERTIFICATE_STATE_ROOT",
            state_root.to_string_lossy().as_ref(),
        )]);

        let result = run_certificate_state_command(&[
            "set-revocation".into(),
            "--path".into(),
            "trust/anchors/root-ca.pem".into(),
            "--scope".into(),
            "trust".into(),
            "--status".into(),
            "distrusted".into(),
            "--note".into(),
            "line1\nline2".into(),
        ]);

        let error = result.expect_err("command should fail");
        assert!(error.contains("note contains control characters"));
        fs::remove_dir_all(root).unwrap();
    }

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "gewyvern-certificate-state-cli-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_test_certificate(cert_path: &Path, key_path: &Path, days: usize, common_name: &str) {
        let status = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key_path.to_string_lossy().as_ref(),
                "-out",
                cert_path.to_string_lossy().as_ref(),
                "-days",
                &days.to_string(),
                "-nodes",
                "-subj",
                &format!("/CN={common_name}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    struct TestEnvGuard {
        previous: Vec<(&'static str, Option<String>)>,
    }

    impl TestEnvGuard {
        fn set(values: &[(&'static str, &str)]) -> Self {
            let mut previous = Vec::with_capacity(values.len());
            for (key, value) in values {
                previous.push((*key, std::env::var(key).ok()));
                unsafe {
                    std::env::set_var(key, value);
                }
            }
            Self { previous }
        }
    }

    impl Drop for TestEnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..).rev() {
                match value {
                    Some(value) => unsafe {
                        std::env::set_var(key, value);
                    },
                    None => unsafe {
                        std::env::remove_var(key);
                    },
                }
            }
        }
    }
}
