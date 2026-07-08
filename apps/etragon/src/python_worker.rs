use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonWorkerConfig {
    pub python_bin: String,
    pub worker_script: PathBuf,
    pub state_file: Option<PathBuf>,
}

impl Default for PythonWorkerConfig {
    fn default() -> Self {
        Self {
            python_bin: "python3".into(),
            worker_script: default_python_worker_script(),
            state_file: None,
        }
    }
}

pub fn default_python_worker_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join("python_baseline_worker.py")
}

pub struct PythonWorkerClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

pub fn with_python_worker<T, F>(config: &PythonWorkerConfig, f: F) -> Result<T, String>
where
    F: FnOnce(&mut PythonWorkerClient) -> Result<T, String>,
{
    let mut worker = PythonWorkerClient::spawn(config)?;
    f(&mut worker)
}

impl PythonWorkerClient {
    pub fn spawn(config: &PythonWorkerConfig) -> Result<Self, String> {
        if !config.worker_script.exists() {
            return Err(format!(
                "python worker script does not exist: {}",
                config.worker_script.display()
            ));
        }
        let mut child = Command::new(&config.python_bin);
        child.arg(&config.worker_script);
        if let Some(state_file) = &config.state_file {
            child.arg("--state-file").arg(state_file);
        }
        let mut child = child
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| {
                format!(
                    "failed to spawn python worker '{}' with script '{}': {err}",
                    config.python_bin,
                    config.worker_script.display()
                )
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "python worker stdin is unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "python worker stdout is unavailable".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }

    pub fn analyze_json(&mut self, snapshot_json: &str) -> Result<String, String> {
        self.send_command("ANALYZE", snapshot_json.trim())
    }

    pub fn train_json(&mut self, snapshot_json: &str, label: &str) -> Result<String, String> {
        self.train_json_with_weight(snapshot_json, label, 1.0)
    }

    pub fn train_json_with_weight(
        &mut self,
        snapshot_json: &str,
        label: &str,
        weight: f64,
    ) -> Result<String, String> {
        let payload = format!(
            "{{\"snapshot\":{},\"label\":\"{}\",\"weight\":{}}}",
            snapshot_json.trim(),
            escape_json_string(label),
            weight
        );
        self.send_command("TRAIN", &payload)
    }

    pub fn memory_info_json(&mut self) -> Result<String, String> {
        self.send_command("MEMORY_INFO", "{}")
    }

    pub fn model_info_json(&mut self) -> Result<String, String> {
        self.send_command("MODEL_INFO", "{}")
    }

    pub fn memory_versions_json(&mut self) -> Result<String, String> {
        self.send_command("MEMORY_VERSIONS", "{}")
    }

    pub fn export_memory_json(&mut self) -> Result<String, String> {
        self.send_command("MEMORY_EXPORT", "{}")
    }

    pub fn import_memory_json(&mut self, memory_snapshot_json: &str) -> Result<String, String> {
        self.send_command("MEMORY_IMPORT", memory_snapshot_json.trim())
    }

    pub fn import_memory_with_strategy_json(
        &mut self,
        memory_snapshot_json: &str,
        strategy: &str,
    ) -> Result<String, String> {
        let payload = format!(
            "{{\"strategy\":\"{}\",\"snapshot\":{}}}",
            escape_json_string(strategy),
            memory_snapshot_json.trim()
        );
        self.send_command("MEMORY_IMPORT", &payload)
    }

    pub fn save_memory_slot_json(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String> {
        let mut fields = vec![format!("\"slot\":\"{}\"", escape_json_string(slot))];
        if let Some(label) = label {
            fields.push(format!("\"label\":\"{}\"", escape_json_string(label)));
        }
        if let Some(note) = note {
            fields.push(format!("\"note\":\"{}\"", escape_json_string(note)));
        }
        if let Some(source) = source {
            fields.push(format!("\"source\":\"{}\"", escape_json_string(source)));
        }
        let payload = format!("{{{}}}", fields.join(","));
        self.send_command("MEMORY_SAVE_SLOT", &payload)
    }

    pub fn load_memory_slot_json(&mut self, slot: &str, strategy: &str) -> Result<String, String> {
        let payload = format!(
            "{{\"slot\":\"{}\",\"strategy\":\"{}\"}}",
            escape_json_string(slot),
            escape_json_string(strategy)
        );
        self.send_command("MEMORY_LOAD_SLOT", &payload)
    }

    pub fn delete_memory_slot_json(&mut self, slot: &str) -> Result<String, String> {
        let payload = format!("{{\"slot\":\"{}\"}}", escape_json_string(slot));
        self.send_command("MEMORY_DELETE_SLOT", &payload)
    }

    pub fn clear_memory_json(&mut self) -> Result<String, String> {
        self.send_command("CLEAR_MEMORY", "{}")
    }

    fn send_command(&mut self, command: &str, payload: &str) -> Result<String, String> {
        self.stdin
            .write_all(command.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\t"))
            .and_then(|_| self.stdin.write_all(payload.as_bytes()))
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|err| format!("failed to write request to python worker: {err}"))?;

        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .map_err(|err| format!("failed to read python worker response: {err}"))?;
        if line.is_empty() {
            return Err("python worker exited without producing a response".to_string());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        let (status, payload) = line
            .split_once('\t')
            .ok_or_else(|| format!("invalid python worker response: {line}"))?;
        match status {
            "OK" => Ok(payload.to_string()),
            "ERR" => Err(format!("python worker error: {payload}")),
            _ => Err(format!("invalid python worker status '{status}'")),
        }
    }
}

impl Drop for PythonWorkerClient {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn escape_json_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn fixture(name: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join(name),
        )
        .expect("fixture should read")
    }

    #[test]
    fn python_worker_analyzes_missing_transition_fixture() {
        let mut worker =
            PythonWorkerClient::spawn(&PythonWorkerConfig::default()).expect("worker should spawn");
        let output = worker
            .analyze_json(&fixture("missing_transition_analysis.json"))
            .expect("worker should analyze fixture");

        assert!(output.contains("\"augmentations\":["));
        assert!(output.contains("\"name\":\"py_ml_candidate_observe_longer\""));
        assert!(output.contains("\"producer_pass\":\"python_baseline_worker\""));
    }

    #[test]
    fn python_worker_keeps_running_for_multiple_requests() {
        let mut worker =
            PythonWorkerClient::spawn(&PythonWorkerConfig::default()).expect("worker should spawn");
        let first = worker
            .analyze_json(&fixture("direct_signal_analysis.json"))
            .expect("worker should analyze first fixture");
        let second = worker
            .analyze_json(&fixture("ambiguous_analysis.json"))
            .expect("worker should analyze second fixture");

        assert!(first.contains("py_ml_candidate_targeted_escalation"));
        assert!(second.contains("py_ml_candidate_multi_hypothesis"));
    }

    #[test]
    fn python_worker_supports_online_training_and_learned_route() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path = std::env::temp_dir().join(format!("etragon-online-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");
        let mut worker = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        let trained = worker
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train");
        assert!(trained.contains("\"status\":\"trained\""));
        assert!(trained.contains("\"weight\":1.0"));
        assert!(trained.contains("\"train_count\":1"));
        assert!(trained.contains("\"last_trained_unix_ms\":"));
        assert!(trained.contains("\"compatible_with\":[\"http_request_followup\"]"));
        assert!(trained.contains("\"competes_with\":[\"targeted_escalation\"]"));
        let output = worker
            .analyze_json(&snapshot)
            .expect("worker should analyze after training");
        assert!(output.contains("\"name\":\"py_ml_candidate_learned_route\""));
        assert!(output.contains("\"pattern_memory_state\":{\"pattern_key\":"));
        assert!(output.contains("\"label_count\":1"));
        assert!(output.contains("\"labels\":[{\"label\":\"network_observe_longer\""));
        assert!(output.contains("\"learned_label\":\"network_observe_longer\""));
        assert!(output.contains("\"support_score\":"));
        assert!(output.contains("\"train_count\":1"));
        assert!(output.contains("\"last_trained_unix_ms\":"));
        assert!(output.contains("\"compatible_with\":[\"http_request_followup\"]"));
        assert!(output.contains("\"competes_with\":[\"targeted_escalation\"]"));
        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn python_worker_online_training_can_shift_preferred_label() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path = std::env::temp_dir().join(format!("etragon-online-shift-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");
        let mut worker = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        worker
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train first label");
        let first = worker
            .analyze_json(&snapshot)
            .expect("worker should analyze after first training");
        assert!(first.contains("\"learned_label\":\"network_observe_longer\""));

        worker
            .train_json(&snapshot, "targeted_escalation")
            .expect("worker should train second label");
        worker
            .train_json(&snapshot, "targeted_escalation")
            .expect("worker should reinforce second label");
        let second = worker
            .analyze_json(&snapshot)
            .expect("worker should analyze after shifting preference");
        assert!(second.contains("\"learned_label\":\"targeted_escalation\""));
        assert!(second.contains("\"score_margin\":"));
        assert!(second.contains("\"train_count\":2"));
        assert!(second.contains("\"last_trained_unix_ms\":"));
        assert!(second.contains("\"runner_up_label\":\"network_observe_longer\""));
        assert!(second.contains("\"runner_up_score\":"));

        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn python_worker_weighted_training_can_override_previous_label_faster() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path =
            std::env::temp_dir().join(format!("etragon-online-weighted-shift-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");
        let mut worker = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        worker
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train first label");
        worker
            .train_json_with_weight(&snapshot, "targeted_escalation", 2.5)
            .expect("worker should train weighted second label");
        let output = worker
            .analyze_json(&snapshot)
            .expect("worker should analyze after weighted training");
        assert!(output.contains("\"pattern_memory_state\":{\"pattern_key\":"));
        assert!(output.contains("\"label_count\":2"));
        assert!(output.contains("\"labels\":[{\"label\":\"targeted_escalation\""));
        assert!(output.contains("\"label\":\"network_observe_longer\""));
        assert!(output.contains("\"learned_label\":\"targeted_escalation\""));
        assert!(output.contains("\"support_score\":"));
        assert!(output.contains("\"train_count\":1"));
        assert!(output.contains("\"last_trained_unix_ms\":"));
        assert!(output.contains("\"compatible_with\":[]"));
        assert!(
            output.contains(
                "\"competes_with\":[\"network_observe_longer\",\"http_request_followup\"]"
            )
        );

        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn python_worker_reports_memory_info_and_supports_clear() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path = std::env::temp_dir().join(format!("etragon-online-info-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");
        let mut worker = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        let empty = worker
            .memory_info_json()
            .expect("worker should report empty memory state");
        assert!(empty.contains("\"status\":\"empty\""));
        assert!(empty.contains("\"schema_version\":1"));
        assert!(empty.contains("\"model_version\":\"python-online-memory-v1\""));
        assert!(empty.contains("\"persistent\":true"));
        assert!(empty.contains("\"pattern_count\":0"));
        assert!(empty.contains("\"label_count\":0"));

        worker
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train");
        let info = worker
            .memory_info_json()
            .expect("worker should report learned memory state");
        assert!(info.contains("\"status\":\"ready\""));
        assert!(info.contains("\"schema_version\":1"));
        assert!(info.contains("\"model_version\":\"python-online-memory-v1\""));
        assert!(info.contains("\"persistent\":true"));
        assert!(info.contains("\"pattern_count\":1"));
        assert!(info.contains("\"label_count\":1"));
        assert!(info.contains("\"last_trained_unix_ms\":"));

        let cleared = worker
            .clear_memory_json()
            .expect("worker should clear memory state");
        assert!(cleared.contains("\"status\":\"cleared\""));
        assert!(cleared.contains("\"cleared_pattern_count\":1"));
        assert!(cleared.contains("\"cleared_label_count\":1"));
        assert!(cleared.contains("\"pattern_count\":0"));
        assert!(cleared.contains("\"label_count\":0"));

        let after = worker
            .memory_info_json()
            .expect("worker should report empty memory after clear");
        assert!(after.contains("\"status\":\"empty\""));
        assert!(after.contains("\"pattern_count\":0"));
        assert!(after.contains("\"label_count\":0"));

        let persisted = std::fs::read_to_string(&state_path).expect("state file should exist");
        assert!(persisted.contains("\"schema_version\":1"));
        assert!(persisted.contains("\"model_version\":\"python-online-memory-v1\""));
        assert!(persisted.contains("\"pattern_labels\":{}"));

        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn python_worker_supports_memory_export_and_import_roundtrip() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path = std::env::temp_dir().join(format!("etragon-export-import-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");

        let mut exporter = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        exporter
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train");
        let exported = exporter
            .export_memory_json()
            .expect("worker should export memory snapshot");
        assert!(exported.contains("\"status\":\"exported\""));
        assert!(exported.contains("\"pattern_count\":1"));
        assert!(exported.contains("\"label_count\":1"));
        assert!(exported.contains("\"pattern_labels\":{"));

        let mut importer = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        importer
            .clear_memory_json()
            .expect("worker should clear memory");
        let cleared = importer
            .memory_info_json()
            .expect("worker should report empty memory");
        assert!(cleared.contains("\"status\":\"empty\""));

        let imported = importer
            .import_memory_json(&exported)
            .expect("worker should import exported memory snapshot");
        assert!(imported.contains("\"status\":\"loaded\""));
        assert!(imported.contains("\"imported_pattern_count\":1"));
        assert!(imported.contains("\"imported_label_count\":1"));
        assert!(imported.contains("\"model_version\":\"python-online-memory-v1\""));

        let output = importer
            .analyze_json(&snapshot)
            .expect("worker should recover learned route after import");
        assert!(output.contains("\"name\":\"py_ml_candidate_learned_route\""));
        assert!(output.contains("\"learned_label\":\"network_observe_longer\""));

        let _ = std::fs::remove_file(state_path);
    }

    #[test]
    fn python_worker_supports_slot_management_and_merge_import() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let state_path = std::env::temp_dir().join(format!("etragon-slot-merge-{unique}.json"));
        let config = PythonWorkerConfig {
            state_file: Some(state_path.clone()),
            ..PythonWorkerConfig::default()
        };
        let snapshot = fixture("missing_transition_analysis.json");

        let mut worker = PythonWorkerClient::spawn(&config).expect("worker should spawn");
        worker
            .train_json(&snapshot, "network_observe_longer")
            .expect("worker should train first label");
        let saved = worker
            .save_memory_slot_json(
                "baseline",
                Some("baseline-v1"),
                Some("manual checkpoint"),
                Some("test_suite"),
            )
            .expect("worker should save baseline slot");
        assert!(saved.contains("\"status\":\"saved\""));
        assert!(saved.contains("\"slot\":\"baseline\""));
        assert!(saved.contains("\"label\":\"baseline-v1\""));
        assert!(saved.contains("\"note\":\"manual checkpoint\""));
        assert!(saved.contains("\"source\":\"test_suite\""));

        worker
            .train_json(&snapshot, "targeted_escalation")
            .expect("worker should train competing label");
        let snapshot_export = worker
            .export_memory_json()
            .expect("worker should export enriched snapshot");
        worker
            .clear_memory_json()
            .expect("worker should clear memory state");

        let loaded = worker
            .load_memory_slot_json("baseline", "replace")
            .expect("worker should load baseline slot");
        assert!(loaded.contains("\"status\":\"loaded\""));
        assert!(loaded.contains("\"slot\":\"baseline\""));
        assert!(loaded.contains("\"strategy\":\"replace\""));

        let merged = worker
            .import_memory_with_strategy_json(&snapshot_export, "merge")
            .expect("worker should merge imported snapshot");
        assert!(merged.contains("\"status\":\"loaded\""));
        assert!(merged.contains("\"strategy\":\"merge\""));

        let versions = worker
            .memory_versions_json()
            .expect("worker should report saved memory versions");
        assert!(versions.contains("\"slot_count\":1"));
        assert!(versions.contains("\"slot\":\"baseline\""));
        assert!(versions.contains("\"label\":\"baseline-v1\""));
        assert!(versions.contains("\"note\":\"manual checkpoint\""));
        assert!(versions.contains("\"source\":\"test_suite\""));
        assert!(versions.contains("\"history\":["));

        let deleted = worker
            .delete_memory_slot_json("baseline")
            .expect("worker should delete saved slot");
        assert!(deleted.contains("\"status\":\"deleted\""));
        assert!(deleted.contains("\"label\":\"baseline-v1\""));
        assert!(deleted.contains("\"slot_count\":0"));

        let _ = std::fs::remove_file(state_path);
    }
}
