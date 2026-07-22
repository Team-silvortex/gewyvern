use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MEMORY_STATE_SCHEMA_VERSION: u64 = 1;
const MEMORY_MODEL_VERSION: &str = "etragon-native-memory-v1";
const LEGACY_MEMORY_MODEL_VERSION: &str = "python-online-memory-v1";
const WORKER_PROTOCOL_VERSION: u64 = 1;
const SNAPSHOT_SLOT_LIMIT: usize = 16;
const SNAPSHOT_HISTORY_LIMIT: usize = 24;
const DEFAULT_DECAY: f64 = 0.875;
const COMPATIBLE_DECAY: f64 = 0.94;
const COMPETING_DECAY: f64 = 0.65;
const MIN_SCORE: f64 = 0.05;

const SUPPORTED_COMMANDS: &[&str] = &[
    "ANALYZE",
    "TRAIN",
    "MEMORY_INFO",
    "MODEL_INFO",
    "MEMORY_VERSIONS",
    "MEMORY_EXPORT",
    "MEMORY_IMPORT",
    "MEMORY_SAVE_SLOT",
    "MEMORY_LOAD_SLOT",
    "MEMORY_DELETE_SLOT",
    "CLEAR_MEMORY",
];

const SUPPORTED_TRAINING_LABELS: &[&str] = &[
    "network_observe_longer",
    "targeted_escalation",
    "http_request_followup",
];

type PatternLabels = BTreeMap<String, BTreeMap<String, LabelMetadata>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeLearningConfig {
    pub state_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct LabelMetadata {
    score: f64,
    train_count: u64,
    last_trained_unix_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MemorySnapshot {
    schema_version: u64,
    model_version: String,
    exported_unix_ms: u64,
    pattern_count: usize,
    label_count: usize,
    last_trained_unix_ms: Option<u64>,
    label: Option<String>,
    note: Option<String>,
    source: String,
    pattern_labels: PatternLabels,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SnapshotSlot {
    slot: String,
    #[serde(flatten)]
    snapshot: MemorySnapshot,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SnapshotHistoryEvent {
    action: String,
    slot: String,
    strategy: String,
    label: Option<String>,
    note: Option<String>,
    source: String,
    saved_unix_ms: u64,
    pattern_count: usize,
    label_count: usize,
}

#[derive(Debug)]
pub struct NativeLearningBackend {
    state_file: Option<PathBuf>,
    pattern_labels: PatternLabels,
    snapshot_slots: BTreeMap<String, SnapshotSlot>,
    snapshot_history: Vec<SnapshotHistoryEvent>,
}

impl NativeLearningBackend {
    pub fn open(config: NativeLearningConfig) -> Result<Self, String> {
        let mut backend = Self {
            state_file: config.state_file,
            pattern_labels: BTreeMap::new(),
            snapshot_slots: BTreeMap::new(),
            snapshot_history: Vec::new(),
        };
        if backend
            .state_file
            .as_ref()
            .is_some_and(|path| path.exists())
        {
            backend.load_state()?;
        }
        Ok(backend)
    }

    pub fn analyze_json(&self, snapshot_json: &str) -> Result<String, String> {
        let snapshot = parse_object(snapshot_json, "analysis snapshot")?;
        let mut augmentations = vec![baseline_candidate(&snapshot)];
        if let Some(candidate) = self.learned_candidate(&snapshot) {
            augmentations.push(candidate);
        }
        encode(&json!({
            "augmentations": augmentations,
            "pattern_memory_state": self.pattern_memory_state(&snapshot),
        }))
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
        if !weight.is_finite() || weight <= 0.0 {
            return Err("weight must be a finite number greater than 0".to_string());
        }
        if !SUPPORTED_TRAINING_LABELS.contains(&label) {
            return Err(format!(
                "unsupported training label '{label}'; expected one of: {}",
                SUPPORTED_TRAINING_LABELS.join(", ")
            ));
        }
        let snapshot = parse_object(snapshot_json, "analysis snapshot")?;
        let pattern_key = build_pattern_key(&snapshot);
        let policy = label_transition_policy(label);
        let labels = self.pattern_labels.entry(pattern_key.clone()).or_default();
        labels.retain(|existing_label, metadata| {
            if existing_label == label {
                return true;
            }
            metadata.score =
                rounded_score(metadata.score * transition_decay(label, existing_label));
            metadata.score >= MIN_SCORE
        });
        let now = now_unix_ms()?;
        let metadata = labels.entry(label.to_string()).or_default();
        metadata.score = rounded_score(metadata.score + weight);
        metadata.train_count += 1;
        metadata.last_trained_unix_ms = now;
        let result = json!({
            "status": "trained",
            "pattern_key": pattern_key,
            "label": label,
            "weight": rounded_score(weight),
            "score": metadata.score,
            "support_count": support_count(metadata.score),
            "train_count": metadata.train_count,
            "last_trained_unix_ms": metadata.last_trained_unix_ms,
            "compatible_with": policy.compatible_with,
            "competes_with": policy.competes_with,
            "backend": "rust-native",
        });
        self.save_state()?;
        encode(&result)
    }

    pub fn memory_info_json(&self) -> Result<String, String> {
        encode(&self.memory_state_summary())
    }

    pub fn model_info_json(&self) -> Result<String, String> {
        encode(&json!({
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "worker_protocol_version": WORKER_PROTOCOL_VERSION,
            "backend": "rust-native",
            "persistent": self.state_file.is_some(),
            "state_file": self.state_file.as_ref().map(|path| path.display().to_string()),
            "supported_commands": SUPPORTED_COMMANDS,
            "supported_training_labels": SUPPORTED_TRAINING_LABELS,
            "training_label_count": SUPPORTED_TRAINING_LABELS.len(),
            "supported_import_strategies": ["replace", "merge"],
            "snapshot_slot_limit": SNAPSHOT_SLOT_LIMIT,
            "snapshot_history_limit": SNAPSHOT_HISTORY_LIMIT,
            "snapshot_slot_metadata_fields": ["slot", "label", "note", "source"],
            "compatible_model_versions": [MEMORY_MODEL_VERSION, LEGACY_MEMORY_MODEL_VERSION],
        }))
    }

    pub fn memory_versions_json(&self) -> Result<String, String> {
        let mut slots = self.snapshot_slots.values().collect::<Vec<_>>();
        slots.sort_by(|left, right| {
            right
                .snapshot
                .exported_unix_ms
                .cmp(&left.snapshot.exported_unix_ms)
                .then_with(|| right.slot.cmp(&left.slot))
        });
        let slots = slots
            .into_iter()
            .map(|slot| {
                json!({
                    "slot": slot.slot,
                    "saved_unix_ms": slot.snapshot.exported_unix_ms,
                    "pattern_count": slot.snapshot.pattern_count,
                    "label_count": slot.snapshot.label_count,
                    "last_trained_unix_ms": slot.snapshot.last_trained_unix_ms,
                    "label": slot.snapshot.label,
                    "note": slot.snapshot.note,
                    "source": slot.snapshot.source,
                })
            })
            .collect::<Vec<_>>();
        encode(&json!({
            "status": "ready",
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "slot_count": slots.len(),
            "slots": slots,
            "history": self.snapshot_history,
        }))
    }

    pub fn export_memory_json(&self) -> Result<String, String> {
        let snapshot = self.export_snapshot(None, None, Some("live_export".to_string()))?;
        let mut value = serde_json::to_value(snapshot)
            .map_err(|err| format!("failed to encode memory snapshot: {err}"))?;
        value["status"] = Value::String(if self.label_count() == 0 {
            "empty".to_string()
        } else {
            "exported".to_string()
        });
        encode(&value)
    }

    pub fn import_memory_json(&mut self, memory_snapshot_json: &str) -> Result<String, String> {
        self.import_memory_payload(memory_snapshot_json, None)
    }

    pub fn import_memory_with_strategy_json(
        &mut self,
        memory_snapshot_json: &str,
        strategy: &str,
    ) -> Result<String, String> {
        self.import_memory_payload(memory_snapshot_json, Some(strategy))
    }

    pub fn save_memory_slot_json(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String> {
        self.save_slot(slot, label, note, source, true)
    }

    pub fn load_memory_slot_json(&mut self, slot: &str, strategy: &str) -> Result<String, String> {
        validate_strategy(strategy)?;
        let slot = clean_required_text(slot, "slot")?;
        let saved = self
            .snapshot_slots
            .get(&slot)
            .cloned()
            .ok_or_else(|| format!("unknown slot '{slot}'"))?;
        let snapshot_json = encode(
            &serde_json::to_value(&saved.snapshot)
                .map_err(|err| format!("failed to encode memory slot: {err}"))?,
        )?;
        let result_json = self.import_memory_payload(&snapshot_json, Some(strategy))?;
        let mut result: Value = serde_json::from_str(&result_json)
            .map_err(|err| format!("failed to decode memory import result: {err}"))?;
        result["slot"] = Value::String(slot.clone());
        self.record_history(
            "load_slot",
            &slot,
            strategy,
            saved.snapshot.label.clone(),
            saved.snapshot.note.clone(),
            Some(saved.snapshot.source.clone()),
            saved.snapshot.pattern_count,
            saved.snapshot.label_count,
        )?;
        self.save_state()?;
        encode(&result)
    }

    pub fn delete_memory_slot_json(&mut self, slot: &str) -> Result<String, String> {
        let slot = clean_required_text(slot, "slot")?;
        let saved = self
            .snapshot_slots
            .remove(&slot)
            .ok_or_else(|| format!("unknown slot '{slot}'"))?;
        self.record_history(
            "delete_slot",
            &slot,
            "replace",
            saved.snapshot.label.clone(),
            saved.snapshot.note.clone(),
            Some(saved.snapshot.source.clone()),
            saved.snapshot.pattern_count,
            saved.snapshot.label_count,
        )?;
        self.save_state()?;
        encode(&json!({
            "status": "deleted",
            "slot": slot,
            "label": saved.snapshot.label,
            "note": saved.snapshot.note,
            "source": saved.snapshot.source,
            "deleted_pattern_count": saved.snapshot.pattern_count,
            "deleted_label_count": saved.snapshot.label_count,
            "slot_count": self.snapshot_slots.len(),
        }))
    }

    pub fn clear_memory_json(&mut self) -> Result<String, String> {
        let cleared_pattern_count = self.pattern_labels.len();
        let cleared_label_count = self.label_count();
        self.pattern_labels.clear();
        self.save_state()?;
        let mut summary = self.memory_state_summary();
        summary["status"] = Value::String("cleared".to_string());
        summary["cleared_pattern_count"] = json!(cleared_pattern_count);
        summary["cleared_label_count"] = json!(cleared_label_count);
        encode(&summary)
    }

    fn import_memory_payload(
        &mut self,
        memory_snapshot_json: &str,
        forced_strategy: Option<&str>,
    ) -> Result<String, String> {
        let payload = parse_object(memory_snapshot_json, "memory snapshot")?;
        let strategy = forced_strategy
            .or_else(|| payload.get("strategy").and_then(Value::as_str))
            .unwrap_or("replace");
        validate_strategy(strategy)?;
        let inline_snapshot = Value::Object(payload.clone());
        let snapshot_value = payload.get("snapshot").unwrap_or(&inline_snapshot);
        let snapshot = normalize_snapshot(snapshot_value)?;
        if strategy == "replace" {
            self.pattern_labels = snapshot.pattern_labels.clone();
        } else {
            merge_pattern_labels(&mut self.pattern_labels, &snapshot.pattern_labels);
        }
        let label = clean_optional_value(payload.get("label")).or(snapshot.label.clone());
        let note = clean_optional_value(payload.get("note")).or(snapshot.note.clone());
        let source =
            clean_optional_value(payload.get("source")).or_else(|| Some(snapshot.source.clone()));
        let save_as_slot = clean_optional_value(payload.get("save_as_slot"));
        if let Some(slot) = save_as_slot.as_deref() {
            self.save_slot(
                slot,
                label.as_deref(),
                note.as_deref(),
                source.as_deref(),
                false,
            )?;
        }
        let imported_pattern_count = self.pattern_labels.len();
        let imported_label_count = self.label_count();
        self.record_history(
            "import_memory",
            save_as_slot.as_deref().unwrap_or(""),
            strategy,
            label.clone(),
            note.clone(),
            source.clone(),
            imported_pattern_count,
            imported_label_count,
        )?;
        self.save_state()?;
        let mut summary = self.memory_state_summary();
        summary["status"] = Value::String(if imported_label_count > 0 {
            "loaded".to_string()
        } else {
            "cleared".to_string()
        });
        summary["imported_pattern_count"] = json!(imported_pattern_count);
        summary["imported_label_count"] = json!(imported_label_count);
        summary["imported_unix_ms"] = json!(now_unix_ms()?);
        summary["strategy"] = Value::String(strategy.to_string());
        summary["label"] = label.map(Value::String).unwrap_or(Value::Null);
        summary["note"] = note.map(Value::String).unwrap_or(Value::Null);
        summary["source"] = source.map(Value::String).unwrap_or(Value::Null);
        encode(&summary)
    }

    fn save_slot(
        &mut self,
        slot: &str,
        label: Option<&str>,
        note: Option<&str>,
        source: Option<&str>,
        record_history: bool,
    ) -> Result<String, String> {
        let slot = clean_required_text(slot, "slot")?;
        let snapshot = self.export_snapshot(
            clean_optional_text(label),
            clean_optional_text(note),
            clean_optional_text(source).or_else(|| Some("manual".to_string())),
        )?;
        let saved_unix_ms = snapshot.exported_unix_ms;
        let pattern_count = snapshot.pattern_count;
        let label_count = snapshot.label_count;
        let result_label = snapshot.label.clone();
        let result_note = snapshot.note.clone();
        let result_source = snapshot.source.clone();
        self.snapshot_slots.insert(
            slot.clone(),
            SnapshotSlot {
                slot: slot.clone(),
                snapshot,
            },
        );
        self.trim_snapshot_slots();
        if record_history {
            self.record_history(
                "save_slot",
                &slot,
                "replace",
                result_label.clone(),
                result_note.clone(),
                Some(result_source.clone()),
                pattern_count,
                label_count,
            )?;
        }
        self.save_state()?;
        encode(&json!({
            "status": "saved",
            "slot": slot,
            "label": result_label,
            "note": result_note,
            "source": result_source,
            "pattern_count": pattern_count,
            "label_count": label_count,
            "saved_unix_ms": saved_unix_ms,
            "slot_count": self.snapshot_slots.len(),
        }))
    }

    fn learned_candidate(&self, snapshot: &Map<String, Value>) -> Option<Value> {
        let pattern_key = build_pattern_key(snapshot);
        let ranked = ranked_labels(self.pattern_labels.get(&pattern_key)?);
        let (top_label, top) = ranked.first()?;
        let runner_up = ranked.get(1);
        let runner_up_score = runner_up.map(|(_, meta)| meta.score).unwrap_or(0.0);
        let policy = label_transition_policy(top_label);
        let mut data = json!({
            "pattern_key": pattern_key,
            "learned_label": top_label,
            "support_score": rounded_score(top.score),
            "support_count": support_count(top.score),
            "train_count": top.train_count,
            "last_trained_unix_ms": top.last_trained_unix_ms,
            "score_margin": rounded_score(top.score - runner_up_score),
            "compatible_with": policy.compatible_with,
            "competes_with": policy.competes_with,
        });
        if let Some((label, metadata)) = runner_up {
            data["runner_up_label"] = Value::String((*label).clone());
            data["runner_up_score"] = json!(rounded_score(metadata.score));
            data["runner_up_train_count"] = json!(metadata.train_count);
            data["runner_up_last_trained_unix_ms"] = json!(metadata.last_trained_unix_ms);
        }
        Some(json!({
            "kind": "native-ml-learned-route",
            "name": "ml_candidate_learned_route",
            "summary": "the native online learner has seen this failure shape before and suggests a learned route",
            "confidence": "candidate",
            "producer_stage": "candidate",
            "producer_pass": "etragon_native_memory",
            "data": data,
        }))
    }

    fn pattern_memory_state(&self, snapshot: &Map<String, Value>) -> Option<Value> {
        let pattern_key = build_pattern_key(snapshot);
        let labels = ranked_labels(self.pattern_labels.get(&pattern_key)?);
        let rendered = labels
            .into_iter()
            .map(|(label, metadata)| {
                let policy = label_transition_policy(label);
                json!({
                    "label": label,
                    "support_score": rounded_score(metadata.score),
                    "train_count": metadata.train_count,
                    "last_trained_unix_ms": metadata.last_trained_unix_ms,
                    "compatible_with": policy.compatible_with,
                    "competes_with": policy.competes_with,
                })
            })
            .collect::<Vec<_>>();
        Some(json!({
            "pattern_key": pattern_key,
            "label_count": rendered.len(),
            "labels": rendered,
        }))
    }

    fn memory_state_summary(&self) -> Value {
        let label_count = self.label_count();
        json!({
            "status": if label_count > 0 { "ready" } else { "empty" },
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "backend": "rust-native",
            "persistent": self.state_file.is_some(),
            "state_file": self.state_file.as_ref().map(|path| path.display().to_string()),
            "pattern_count": self.pattern_labels.len(),
            "label_count": label_count,
            "last_trained_unix_ms": self.last_trained_unix_ms(),
            "snapshot_slot_count": self.snapshot_slots.len(),
            "snapshot_history_count": self.snapshot_history.len(),
        })
    }

    fn export_snapshot(
        &self,
        label: Option<String>,
        note: Option<String>,
        source: Option<String>,
    ) -> Result<MemorySnapshot, String> {
        Ok(MemorySnapshot {
            schema_version: MEMORY_STATE_SCHEMA_VERSION,
            model_version: MEMORY_MODEL_VERSION.to_string(),
            exported_unix_ms: now_unix_ms()?,
            pattern_count: self.pattern_labels.len(),
            label_count: self.label_count(),
            last_trained_unix_ms: self.last_trained_unix_ms(),
            label,
            note,
            source: source.unwrap_or_else(|| "manual".to_string()),
            pattern_labels: self.pattern_labels.clone(),
        })
    }

    fn label_count(&self) -> usize {
        self.pattern_labels.values().map(BTreeMap::len).sum()
    }

    fn last_trained_unix_ms(&self) -> Option<u64> {
        self.pattern_labels
            .values()
            .flat_map(BTreeMap::values)
            .map(|metadata| metadata.last_trained_unix_ms)
            .filter(|timestamp| *timestamp > 0)
            .max()
    }

    fn trim_snapshot_slots(&mut self) {
        while self.snapshot_slots.len() > SNAPSHOT_SLOT_LIMIT {
            let oldest = self
                .snapshot_slots
                .iter()
                .min_by_key(|(slot, snapshot)| (snapshot.snapshot.exported_unix_ms, *slot))
                .map(|(slot, _)| slot.clone());
            if let Some(slot) = oldest {
                self.snapshot_slots.remove(&slot);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_history(
        &mut self,
        action: &str,
        slot: &str,
        strategy: &str,
        label: Option<String>,
        note: Option<String>,
        source: Option<String>,
        pattern_count: usize,
        label_count: usize,
    ) -> Result<(), String> {
        self.snapshot_history.insert(
            0,
            SnapshotHistoryEvent {
                action: action.to_string(),
                slot: slot.to_string(),
                strategy: strategy.to_string(),
                label,
                note,
                source: source.unwrap_or_else(|| "manual".to_string()),
                saved_unix_ms: now_unix_ms()?,
                pattern_count,
                label_count,
            },
        );
        self.snapshot_history.truncate(SNAPSHOT_HISTORY_LIMIT);
        Ok(())
    }

    fn load_state(&mut self) -> Result<(), String> {
        let state_file = self.state_file.as_ref().expect("state path was checked");
        let raw = fs::read_to_string(state_file).map_err(|err| {
            format!(
                "failed to read native learning state '{}': {err}",
                state_file.display()
            )
        })?;
        let value: Value = serde_json::from_str(&raw).map_err(|err| {
            format!(
                "failed to parse native learning state '{}': {err}",
                state_file.display()
            )
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| "native learning state must be a JSON object".to_string())?;
        validate_schema_and_model(object)?;
        self.pattern_labels = normalize_pattern_labels(
            object
                .get("pattern_labels")
                .unwrap_or(&Value::Object(Map::new())),
        )?;
        self.snapshot_slots = normalize_snapshot_slots(
            object
                .get("snapshot_slots")
                .unwrap_or(&Value::Object(Map::new())),
        )?;
        self.snapshot_history = object
            .get("snapshot_history")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|err| format!("invalid snapshot_history: {err}"))?
            .unwrap_or_default();
        self.snapshot_history.truncate(SNAPSHOT_HISTORY_LIMIT);
        self.trim_snapshot_slots();
        Ok(())
    }

    fn save_state(&self) -> Result<(), String> {
        let Some(state_file) = &self.state_file else {
            return Ok(());
        };
        if let Some(parent) = state_file
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|err| {
                format!(
                    "failed to create native learning state directory '{}': {err}",
                    parent.display()
                )
            })?;
        }
        let value = json!({
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "backend": "rust-native",
            "pattern_labels": self.pattern_labels,
            "snapshot_slots": self.snapshot_slots,
            "snapshot_history": self.snapshot_history,
        });
        let payload = serde_json::to_vec(&value)
            .map_err(|err| format!("failed to encode native learning state: {err}"))?;
        let temp_file = temporary_state_path(state_file)?;
        fs::write(&temp_file, payload).map_err(|err| {
            format!(
                "failed to write native learning state '{}': {err}",
                temp_file.display()
            )
        })?;
        if let Err(err) = fs::rename(&temp_file, state_file) {
            let _ = fs::remove_file(&temp_file);
            return Err(format!(
                "failed to replace native learning state '{}': {err}",
                state_file.display()
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct LabelPolicy {
    compatible_with: &'static [&'static str],
    competes_with: &'static [&'static str],
}

fn label_transition_policy(label: &str) -> LabelPolicy {
    match label {
        "network_observe_longer" => LabelPolicy {
            compatible_with: &["http_request_followup"],
            competes_with: &["targeted_escalation"],
        },
        "targeted_escalation" => LabelPolicy {
            compatible_with: &[],
            competes_with: &["network_observe_longer", "http_request_followup"],
        },
        "http_request_followup" => LabelPolicy {
            compatible_with: &["network_observe_longer"],
            competes_with: &["targeted_escalation"],
        },
        _ => LabelPolicy {
            compatible_with: &[],
            competes_with: &[],
        },
    }
}

fn transition_decay(incoming_label: &str, existing_label: &str) -> f64 {
    let policy = label_transition_policy(incoming_label);
    if policy.compatible_with.contains(&existing_label) {
        COMPATIBLE_DECAY
    } else if policy.competes_with.contains(&existing_label) {
        COMPETING_DECAY
    } else {
        DEFAULT_DECAY
    }
}

fn baseline_candidate(snapshot: &Map<String, Value>) -> Value {
    let module = string_field(snapshot, "primary_module_kind", "unknown");
    let mode = string_field(snapshot, "primary_failure_mode", "unknown");
    let detail = string_field(snapshot, "primary_failure_detail", "unknown");
    let confidence = string_field(snapshot, "primary_failure_confidence", "unknown");
    let basis = string_field(snapshot, "primary_failure_basis", "unknown");
    let ambiguous = snapshot
        .get("ambiguous")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let competing = snapshot
        .get("competing_hypotheses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let score = rounded_milli(
        if confidence == "high" {
            1.5
        } else if confidence == "medium" {
            1.0
        } else {
            0.0
        } + if basis == "direct_protocol_signal" {
            1.2
        } else if basis == "missing_transition" {
            0.7
        } else {
            0.0
        } - if ambiguous { 0.8 } else { 0.0 }
            + competing.len().min(3) as f64 * 0.2,
    );
    let (name, summary, data) = if ambiguous && !competing.is_empty() {
        (
            "ml_candidate_multi_hypothesis",
            "multiple hypotheses remain active; keep this target open for later rerank or model-assisted clustering",
            json!({"module": module, "score": score, "competing_hypotheses": competing}),
        )
    } else if confidence == "medium" && basis == "missing_transition" {
        (
            "ml_candidate_observe_longer",
            "the current runtime evidence still looks timeout-shaped; collect a slightly longer observation window before narrowing further",
            json!({"module": module, "score": score, "failure_detail": detail}),
        )
    } else if confidence == "high" && basis == "direct_protocol_signal" {
        (
            "ml_candidate_targeted_escalation",
            "the protocol-level signal is direct enough for downstream escalation or stronger automated routing",
            json!({"module": module, "score": score, "failure_mode": mode}),
        )
    } else {
        (
            "ml_candidate_manual_review",
            "the snapshot is still advisory; keep it available for a manual or model-assisted second pass",
            json!({"module": module, "score": score, "failure_confidence": confidence, "failure_basis": basis}),
        )
    };
    json!({
        "kind": "native-ml-candidate",
        "name": name,
        "summary": summary,
        "confidence": "candidate",
        "producer_stage": "candidate",
        "producer_pass": "etragon_native_baseline",
        "data": data,
    })
}

fn build_pattern_key(snapshot: &Map<String, Value>) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        string_field(snapshot, "primary_module_kind", "unknown"),
        string_field(snapshot, "primary_failure_mode", "unknown"),
        string_field(snapshot, "primary_failure_detail", "unknown"),
        string_field(snapshot, "primary_failure_confidence", "unknown"),
        string_field(snapshot, "primary_failure_basis", "unknown"),
        if snapshot
            .get("ambiguous")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            "ambiguous"
        } else {
            "clear"
        }
    )
}

fn ranked_labels(labels: &BTreeMap<String, LabelMetadata>) -> Vec<(&String, &LabelMetadata)> {
    let mut ranked = labels.iter().collect::<Vec<_>>();
    ranked.sort_by(|(left_label, left), (right_label, right)| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.last_trained_unix_ms.cmp(&left.last_trained_unix_ms))
            .then_with(|| right_label.cmp(left_label))
    });
    ranked
}

fn merge_pattern_labels(current: &mut PatternLabels, incoming: &PatternLabels) {
    for (pattern_key, incoming_labels) in incoming {
        let current_labels = current.entry(pattern_key.clone()).or_default();
        for (label, incoming_metadata) in incoming_labels {
            let metadata = current_labels.entry(label.clone()).or_default();
            metadata.score = rounded_score(metadata.score + incoming_metadata.score);
            metadata.train_count += incoming_metadata.train_count;
            metadata.last_trained_unix_ms = metadata
                .last_trained_unix_ms
                .max(incoming_metadata.last_trained_unix_ms);
        }
    }
}

fn normalize_snapshot(value: &Value) -> Result<MemorySnapshot, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "memory snapshot must be an object".to_string())?;
    validate_schema_and_model(object)?;
    let pattern_labels = normalize_pattern_labels(
        object
            .get("pattern_labels")
            .unwrap_or(&Value::Object(Map::new())),
    )?;
    let pattern_count = pattern_labels.len();
    let label_count = pattern_labels.values().map(BTreeMap::len).sum();
    let last_trained_unix_ms = pattern_labels
        .values()
        .flat_map(BTreeMap::values)
        .map(|metadata| metadata.last_trained_unix_ms)
        .filter(|timestamp| *timestamp > 0)
        .max();
    Ok(MemorySnapshot {
        schema_version: MEMORY_STATE_SCHEMA_VERSION,
        model_version: MEMORY_MODEL_VERSION.to_string(),
        exported_unix_ms: object
            .get("exported_unix_ms")
            .and_then(Value::as_u64)
            .unwrap_or(now_unix_ms()?),
        pattern_count,
        label_count,
        last_trained_unix_ms,
        label: clean_optional_value(object.get("label")),
        note: clean_optional_value(object.get("note")),
        source: clean_optional_value(object.get("source")).unwrap_or_else(|| "manual".to_string()),
        pattern_labels,
    })
}

fn normalize_pattern_labels(value: &Value) -> Result<PatternLabels, String> {
    let patterns = value
        .as_object()
        .ok_or_else(|| "pattern_labels must be an object".to_string())?;
    let mut normalized = BTreeMap::new();
    for (pattern_key, raw_labels) in patterns {
        let labels = raw_labels
            .as_object()
            .ok_or_else(|| format!("pattern labels for key '{pattern_key}' must be an object"))?;
        let mut normalized_labels = BTreeMap::new();
        for (label, raw_metadata) in labels {
            let metadata = if let Some(score) = raw_metadata.as_f64() {
                LabelMetadata {
                    score,
                    train_count: score.round().max(1.0) as u64,
                    last_trained_unix_ms: 0,
                }
            } else {
                let object = raw_metadata.as_object().ok_or_else(|| {
                    format!("metadata for label '{label}' must be an object or number")
                })?;
                LabelMetadata {
                    score: object.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                    train_count: object
                        .get("train_count")
                        .and_then(Value::as_u64)
                        .unwrap_or(1)
                        .max(1),
                    last_trained_unix_ms: object
                        .get("last_trained_unix_ms")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                }
            };
            if !metadata.score.is_finite() || metadata.score < 0.0 {
                return Err(format!(
                    "metadata score for label '{label}' must be finite and non-negative"
                ));
            }
            normalized_labels.insert(label.clone(), metadata);
        }
        normalized.insert(pattern_key.clone(), normalized_labels);
    }
    Ok(normalized)
}

fn normalize_snapshot_slots(value: &Value) -> Result<BTreeMap<String, SnapshotSlot>, String> {
    let slots = value
        .as_object()
        .ok_or_else(|| "snapshot_slots must be an object".to_string())?;
    let mut normalized = BTreeMap::new();
    for (slot, value) in slots {
        normalized.insert(
            slot.clone(),
            SnapshotSlot {
                slot: slot.clone(),
                snapshot: normalize_snapshot(value)?,
            },
        );
    }
    Ok(normalized)
}

fn validate_schema_and_model(object: &Map<String, Value>) -> Result<(), String> {
    let schema = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(MEMORY_STATE_SCHEMA_VERSION);
    if schema != MEMORY_STATE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {schema}; expected {MEMORY_STATE_SCHEMA_VERSION}"
        ));
    }
    let model = object
        .get("model_version")
        .and_then(Value::as_str)
        .unwrap_or(MEMORY_MODEL_VERSION);
    if ![MEMORY_MODEL_VERSION, LEGACY_MEMORY_MODEL_VERSION].contains(&model) {
        return Err(format!(
            "unsupported model_version '{model}'; expected '{MEMORY_MODEL_VERSION}' or legacy '{LEGACY_MEMORY_MODEL_VERSION}'"
        ));
    }
    Ok(())
}

fn parse_object(input: &str, description: &str) -> Result<Map<String, Value>, String> {
    serde_json::from_str::<Value>(input)
        .map_err(|err| format!("failed to parse {description}: {err}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{description} must be a JSON object"))
}

fn string_field(object: &Map<String, Value>, key: &str, default: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn clean_required_text(value: &str, field: &str) -> Result<String, String> {
    clean_optional_text(Some(value)).ok_or_else(|| format!("{field} must not be empty"))
}

fn clean_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn clean_optional_value(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => clean_optional_text(Some(value)),
        Some(Value::Null) | None => None,
        Some(value) => clean_optional_text(Some(&value.to_string())),
    }
}

fn validate_strategy(strategy: &str) -> Result<(), String> {
    if matches!(strategy, "replace" | "merge") {
        Ok(())
    } else {
        Err("strategy must be one of: replace, merge".to_string())
    }
}

fn support_count(score: f64) -> u64 {
    score.round().max(1.0) as u64
}

fn rounded_score(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn rounded_milli(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn now_unix_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|err| format!("system clock is before the Unix epoch: {err}"))
}

fn temporary_state_path(state_file: &Path) -> Result<PathBuf, String> {
    let file_name = state_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "invalid native learning state path '{}'",
                state_file.display()
            )
        })?;
    Ok(state_file.with_file_name(format!(
        "{file_name}.{}.{}.tmp",
        std::process::id(),
        now_unix_ms()?
    )))
}

fn encode(value: &Value) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|err| format!("failed to encode native learning output: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("fixtures")
                .join(name),
        )
        .expect("fixture should read")
    }

    fn temporary_state_path_for_test(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "etragon-native-{name}-{}-{}.json",
            std::process::id(),
            now_unix_ms().expect("clock should work")
        ))
    }

    #[test]
    fn native_backend_analyzes_without_an_external_runtime() {
        let backend = NativeLearningBackend::open(NativeLearningConfig::default())
            .expect("backend should open");
        let output = backend
            .analyze_json(&fixture("missing_transition_analysis.json"))
            .expect("backend should analyze fixture");
        assert!(output.contains("ml_candidate_observe_longer"));
        assert!(output.contains("etragon_native_baseline"));
        assert!(output.contains("\"pattern_memory_state\":null"));
    }

    #[test]
    fn native_training_persists_and_restores_learned_routes() {
        let state_file = temporary_state_path_for_test("persistence");
        let config = NativeLearningConfig {
            state_file: Some(state_file.clone()),
        };
        let snapshot = fixture("missing_transition_analysis.json");
        {
            let mut backend =
                NativeLearningBackend::open(config.clone()).expect("backend should open");
            let trained = backend
                .train_json(&snapshot, "network_observe_longer")
                .expect("training should succeed");
            assert!(trained.contains("\"backend\":\"rust-native\""));
            assert!(trained.contains("\"score\":1.0"));
        }
        let backend = NativeLearningBackend::open(config).expect("state should reload");
        let output = backend
            .analyze_json(&snapshot)
            .expect("analysis should succeed");
        assert!(output.contains("ml_candidate_learned_route"));
        assert!(output.contains("etragon_native_memory"));
        let _ = fs::remove_file(state_file);
    }

    #[test]
    fn competing_training_decays_the_previous_route() {
        let mut backend = NativeLearningBackend::open(NativeLearningConfig::default())
            .expect("backend should open");
        let snapshot = fixture("missing_transition_analysis.json");
        backend
            .train_json_with_weight(&snapshot, "network_observe_longer", 2.0)
            .expect("first training should succeed");
        backend
            .train_json(&snapshot, "targeted_escalation")
            .expect("competing training should succeed");
        let output = backend
            .analyze_json(&snapshot)
            .expect("analysis should succeed");
        assert!(output.contains("\"support_score\":1.3"));
        assert!(output.contains("\"runner_up_score\":1.0"));
    }

    #[test]
    fn memory_slots_restore_a_saved_model() {
        let mut backend = NativeLearningBackend::open(NativeLearningConfig::default())
            .expect("backend should open");
        let snapshot = fixture("direct_signal_analysis.json");
        backend
            .train_json(&snapshot, "targeted_escalation")
            .expect("training should succeed");
        backend
            .save_memory_slot_json("known-good", Some("Known good"), None, Some("test"))
            .expect("slot should save");
        backend.clear_memory_json().expect("memory should clear");
        assert!(
            backend
                .analyze_json(&snapshot)
                .expect("analysis should succeed")
                .contains("\"pattern_memory_state\":null")
        );
        backend
            .load_memory_slot_json("known-good", "replace")
            .expect("slot should load");
        assert!(
            backend
                .analyze_json(&snapshot)
                .expect("analysis should succeed")
                .contains("ml_candidate_learned_route")
        );
    }

    #[test]
    fn native_backend_imports_legacy_python_memory() {
        let mut backend = NativeLearningBackend::open(NativeLearningConfig::default())
            .expect("backend should open");
        let legacy = json!({
            "schema_version": 1,
            "model_version": LEGACY_MEMORY_MODEL_VERSION,
            "pattern_labels": {
                "network|timeout|missing|medium|missing_transition|clear": {
                    "network_observe_longer": 2.0
                }
            }
        });
        let loaded = backend
            .import_memory_json(&legacy.to_string())
            .expect("legacy memory should import");
        assert!(loaded.contains("\"status\":\"loaded\""));
        assert!(loaded.contains("\"label_count\":1"));
    }
}
