import json
import time
from pathlib import Path
from typing import Any

from python_online_memory_support import (
    MEMORY_MODEL_VERSION,
    MEMORY_STATE_SCHEMA_VERSION,
    SNAPSHOT_HISTORY_LIMIT,
    SNAPSHOT_SLOT_LIMIT,
    SUPPORTED_COMMANDS,
    SUPPORTED_TRAINING_LABELS,
    WORKER_PROTOCOL_VERSION,
    build_pattern_key,
    clean_optional_text,
    label_transition_policy,
)


class OnlineModel:
    DECAY = 0.875
    COMPATIBLE_DECAY = 0.94
    COMPETING_DECAY = 0.65
    BOOST = 1.0
    MIN_SCORE = 0.05

    def __init__(self, state_file: Path | None) -> None:
        self.state_file = state_file
        self.pattern_labels: dict[str, dict[str, dict[str, Any]]] = {}
        self.snapshot_slots: dict[str, dict[str, Any]] = {}
        self.snapshot_history: list[dict[str, Any]] = []
        if state_file is not None and state_file.exists():
            self._load()

    def _load(self) -> None:
        assert self.state_file is not None
        payload = json.loads(self.state_file.read_text(encoding="utf-8"))
        self.pattern_labels = self._normalize_pattern_labels(payload.get("pattern_labels", {}))
        self.snapshot_slots = self._normalize_snapshot_slots(payload.get("snapshot_slots", {}))
        self.snapshot_history = self._normalize_snapshot_history(
            payload.get("snapshot_history", [])
        )

    def _normalize_pattern_labels(
        self, raw: dict[str, Any] | Any
    ) -> dict[str, dict[str, dict[str, Any]]]:
        if not isinstance(raw, dict):
            raise ValueError("pattern_labels must be an object")
        normalized_pattern_labels: dict[str, dict[str, dict[str, Any]]] = {}
        for key, labels in raw.items():
            if not isinstance(labels, dict):
                raise ValueError(f"pattern labels for key '{key}' must be an object")
            normalized_labels: dict[str, dict[str, Any]] = {}
            for label, metadata in labels.items():
                if isinstance(metadata, dict):
                    normalized_labels[str(label)] = {
                        "score": float(metadata.get("score", 0.0)),
                        "train_count": max(1, int(metadata.get("train_count", 1))),
                        "last_trained_unix_ms": int(metadata.get("last_trained_unix_ms", 0)),
                    }
                else:
                    normalized_labels[str(label)] = {
                        "score": float(metadata),
                        "train_count": max(1, int(round(float(metadata)))),
                        "last_trained_unix_ms": 0,
                    }
            normalized_pattern_labels[str(key)] = normalized_labels
        return normalized_pattern_labels

    def _normalize_snapshot_slots(
        self, raw: dict[str, Any] | Any
    ) -> dict[str, dict[str, Any]]:
        if not isinstance(raw, dict):
            raise ValueError("snapshot_slots must be an object")
        normalized_slots: dict[str, dict[str, Any]] = {}
        for slot, snapshot in raw.items():
            normalized_slots[str(slot)] = self._normalize_memory_snapshot(
                snapshot, require_snapshot_wrapper=False
            )
            normalized_slots[str(slot)]["slot"] = str(slot)
        return normalized_slots

    def _normalize_snapshot_history(self, raw: list[Any] | Any) -> list[dict[str, Any]]:
        if not isinstance(raw, list):
            raise ValueError("snapshot_history must be an array")
        normalized_history: list[dict[str, Any]] = []
        for event in raw[:SNAPSHOT_HISTORY_LIMIT]:
            if not isinstance(event, dict):
                continue
            normalized_history.append(
                {
                    "action": str(event.get("action", "unknown")),
                    "slot": str(event.get("slot", "")),
                    "strategy": str(event.get("strategy", "replace")),
                    "label": clean_optional_text(event.get("label")),
                    "note": clean_optional_text(event.get("note")),
                    "source": clean_optional_text(event.get("source")) or "manual",
                    "saved_unix_ms": int(event.get("saved_unix_ms", 0)),
                    "pattern_count": max(0, int(event.get("pattern_count", 0))),
                    "label_count": max(0, int(event.get("label_count", 0))),
                }
            )
        return normalized_history

    def _normalize_memory_snapshot(
        self, payload: dict[str, Any] | Any, require_snapshot_wrapper: bool
    ) -> dict[str, Any]:
        if not isinstance(payload, dict):
            raise ValueError("memory snapshot must be an object")
        snapshot = payload.get("snapshot") if require_snapshot_wrapper else payload
        if not isinstance(snapshot, dict):
            raise ValueError("memory snapshot payload must include a snapshot object")
        schema_version = int(snapshot.get("schema_version", MEMORY_STATE_SCHEMA_VERSION))
        if schema_version != MEMORY_STATE_SCHEMA_VERSION:
            raise ValueError(
                f"unsupported schema_version {schema_version}; expected {MEMORY_STATE_SCHEMA_VERSION}"
            )
        model_version = str(snapshot.get("model_version", MEMORY_MODEL_VERSION))
        if model_version != MEMORY_MODEL_VERSION:
            raise ValueError(
                f"unsupported model_version '{model_version}'; expected '{MEMORY_MODEL_VERSION}'"
            )
        normalized_pattern_labels = self._normalize_pattern_labels(
            snapshot.get("pattern_labels", {})
        )
        pattern_count = len(normalized_pattern_labels)
        label_count = sum(len(labels) for labels in normalized_pattern_labels.values())
        last_trained_unix_ms = max(
            (
                int(metadata.get("last_trained_unix_ms", 0))
                for labels in normalized_pattern_labels.values()
                for metadata in labels.values()
            ),
            default=0,
        )
        return {
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "exported_unix_ms": int(snapshot.get("exported_unix_ms", self._now_unix_ms())),
            "pattern_count": pattern_count,
            "label_count": label_count,
            "last_trained_unix_ms": last_trained_unix_ms if last_trained_unix_ms > 0 else None,
            "label": clean_optional_text(snapshot.get("label")),
            "note": clean_optional_text(snapshot.get("note")),
            "source": clean_optional_text(snapshot.get("source")) or "manual",
            "pattern_labels": normalized_pattern_labels,
        }

    def _trim_snapshot_slots(self) -> None:
        if len(self.snapshot_slots) <= SNAPSHOT_SLOT_LIMIT:
            return
        ranked = sorted(
            self.snapshot_slots.items(),
            key=lambda item: (int(item[1].get("exported_unix_ms", 0)), item[0]),
            reverse=True,
        )
        self.snapshot_slots = dict(ranked[:SNAPSHOT_SLOT_LIMIT])

    def _record_snapshot_history(
        self,
        action: str,
        slot: str,
        strategy: str,
        label: str | None,
        note: str | None,
        source: str | None,
        pattern_count: int,
        label_count: int,
    ) -> None:
        self.snapshot_history.insert(
            0,
            {
                "action": action,
                "slot": slot,
                "strategy": strategy,
                "label": label,
                "note": note,
                "source": source or "manual",
                "saved_unix_ms": self._now_unix_ms(),
                "pattern_count": pattern_count,
                "label_count": label_count,
            },
        )
        self.snapshot_history = self.snapshot_history[:SNAPSHOT_HISTORY_LIMIT]

    def _merge_pattern_labels(
        self,
        current: dict[str, dict[str, dict[str, Any]]],
        incoming: dict[str, dict[str, dict[str, Any]]],
    ) -> dict[str, dict[str, dict[str, Any]]]:
        merged: dict[str, dict[str, dict[str, Any]]] = {}
        for pattern_key in set(current.keys()) | set(incoming.keys()):
            merged_labels: dict[str, dict[str, Any]] = {}
            current_labels = current.get(pattern_key, {})
            incoming_labels = incoming.get(pattern_key, {})
            for label in set(current_labels.keys()) | set(incoming_labels.keys()):
                current_meta = current_labels.get(label)
                incoming_meta = incoming_labels.get(label)
                if current_meta and incoming_meta:
                    merged_labels[label] = {
                        "score": round(
                            float(current_meta.get("score", 0.0))
                            + float(incoming_meta.get("score", 0.0)),
                            6,
                        ),
                        "train_count": int(current_meta.get("train_count", 0))
                        + int(incoming_meta.get("train_count", 0)),
                        "last_trained_unix_ms": max(
                            int(current_meta.get("last_trained_unix_ms", 0)),
                            int(incoming_meta.get("last_trained_unix_ms", 0)),
                        ),
                    }
                elif current_meta:
                    merged_labels[label] = dict(current_meta)
                elif incoming_meta:
                    merged_labels[label] = dict(incoming_meta)
            merged[pattern_key] = merged_labels
        return merged

    def _save(self) -> None:
        if self.state_file is None:
            return
        self.state_file.parent.mkdir(parents=True, exist_ok=True)
        self.state_file.write_text(
            json.dumps(
                {
                    "schema_version": MEMORY_STATE_SCHEMA_VERSION,
                    "model_version": MEMORY_MODEL_VERSION,
                    "pattern_labels": self.pattern_labels,
                    "snapshot_slots": self.snapshot_slots,
                    "snapshot_history": self.snapshot_history,
                },
                separators=(",", ":"),
            ),
            encoding="utf-8",
        )

    def _now_unix_ms(self) -> int:
        return int(time.time() * 1000)

    def transition_decay_for(self, incoming_label: str, existing_label: str) -> float:
        if incoming_label == existing_label:
            return 1.0
        policy = label_transition_policy(incoming_label)
        if existing_label in policy["compatible_with"]:
            return self.COMPATIBLE_DECAY
        if existing_label in policy["competes_with"]:
            return self.COMPETING_DECAY
        return self.DECAY

    def train(self, snapshot: dict[str, Any], label: str, weight: float = 1.0) -> dict[str, Any]:
        key = build_pattern_key(snapshot)
        labels = self.pattern_labels.setdefault(key, {})
        for existing_label in list(labels.keys()):
            if existing_label == label:
                continue
            decay = self.transition_decay_for(label, existing_label)
            labels[existing_label]["score"] = round(
                float(labels[existing_label]["score"]) * decay, 6
            )
            if labels[existing_label]["score"] < self.MIN_SCORE:
                del labels[existing_label]
        now_unix_ms = self._now_unix_ms()
        existing = labels.get(label, {"score": 0.0, "train_count": 0, "last_trained_unix_ms": 0})
        labels[label] = {
            "score": round(float(existing.get("score", 0.0)) + (self.BOOST * weight), 6),
            "train_count": int(existing.get("train_count", 0)) + 1,
            "last_trained_unix_ms": now_unix_ms,
        }
        self._save()
        policy = label_transition_policy(label)
        return {
            "status": "trained",
            "pattern_key": key,
            "label": label,
            "weight": round(weight, 6),
            "score": round(float(labels[label]["score"]), 6),
            "support_count": max(1, int(round(float(labels[label]["score"])))),
            "train_count": int(labels[label]["train_count"]),
            "last_trained_unix_ms": int(labels[label]["last_trained_unix_ms"]),
            "compatible_with": policy["compatible_with"],
            "competes_with": policy["competes_with"],
        }

    def learned_candidate(self, snapshot: dict[str, Any]) -> dict[str, Any] | None:
        key = build_pattern_key(snapshot)
        labels = self.pattern_labels.get(key)
        if not labels:
            return None
        ranked = sorted(
            labels.items(),
            key=lambda item: (
                float(item[1]["score"]),
                int(item[1].get("last_trained_unix_ms", 0)),
                item[0],
            ),
            reverse=True,
        )
        top_label, top_metadata = ranked[0]
        top_score = float(top_metadata["score"])
        runner_up_label = ranked[1][0] if len(ranked) > 1 else None
        runner_up_metadata = ranked[1][1] if len(ranked) > 1 else None
        runner_up_score = float(runner_up_metadata["score"]) if runner_up_metadata else 0.0
        score_margin = round(top_score - runner_up_score, 6)
        policy = label_transition_policy(top_label)
        data = {
            "pattern_key": key,
            "learned_label": top_label,
            "support_score": round(top_score, 6),
            "support_count": max(1, int(round(top_score))),
            "train_count": int(top_metadata.get("train_count", 1)),
            "last_trained_unix_ms": int(top_metadata.get("last_trained_unix_ms", 0)),
            "score_margin": score_margin,
            "compatible_with": policy["compatible_with"],
            "competes_with": policy["competes_with"],
        }
        if runner_up_label is not None and runner_up_metadata is not None:
            data["runner_up_label"] = runner_up_label
            data["runner_up_score"] = round(runner_up_score, 6)
            data["runner_up_train_count"] = int(runner_up_metadata.get("train_count", 1))
            data["runner_up_last_trained_unix_ms"] = int(
                runner_up_metadata.get("last_trained_unix_ms", 0)
            )
        return {
            "kind": "py-ml-learned-route",
            "name": "py_ml_candidate_learned_route",
            "summary": "the online learner has seen this failure shape before and suggests a learned route",
            "confidence": "candidate",
            "producer_stage": "candidate",
            "producer_pass": "python_online_memory",
            "data": data,
        }

    def pattern_memory_state(self, snapshot: dict[str, Any]) -> dict[str, Any] | None:
        key = build_pattern_key(snapshot)
        labels = self.pattern_labels.get(key)
        if not labels:
            return None
        ranked = sorted(
            labels.items(),
            key=lambda item: (
                float(item[1]["score"]),
                int(item[1].get("last_trained_unix_ms", 0)),
                item[0],
            ),
            reverse=True,
        )
        rendered_labels = []
        for label, metadata in ranked:
            policy = label_transition_policy(label)
            rendered_labels.append(
                {
                    "label": label,
                    "support_score": round(float(metadata["score"]), 6),
                    "train_count": int(metadata.get("train_count", 1)),
                    "last_trained_unix_ms": int(metadata.get("last_trained_unix_ms", 0)),
                    "compatible_with": policy["compatible_with"],
                    "competes_with": policy["competes_with"],
                }
            )
        return {"pattern_key": key, "label_count": len(rendered_labels), "labels": rendered_labels}

    def memory_state_summary(self) -> dict[str, Any]:
        pattern_count = len(self.pattern_labels)
        label_count = sum(len(labels) for labels in self.pattern_labels.values())
        last_trained_unix_ms = max(
            (
                int(metadata.get("last_trained_unix_ms", 0))
                for labels in self.pattern_labels.values()
                for metadata in labels.values()
            ),
            default=0,
        )
        return {
            "status": "ready" if label_count > 0 else "empty",
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "persistent": self.state_file is not None,
            "state_file": str(self.state_file) if self.state_file is not None else None,
            "pattern_count": pattern_count,
            "label_count": label_count,
            "last_trained_unix_ms": last_trained_unix_ms if last_trained_unix_ms > 0 else None,
            "snapshot_slot_count": len(self.snapshot_slots),
            "snapshot_history_count": len(self.snapshot_history),
        }

    def model_info(self) -> dict[str, Any]:
        return {
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "worker_protocol_version": WORKER_PROTOCOL_VERSION,
            "persistent": self.state_file is not None,
            "state_file": str(self.state_file) if self.state_file is not None else None,
            "supported_commands": SUPPORTED_COMMANDS,
            "supported_training_labels": SUPPORTED_TRAINING_LABELS,
            "training_label_count": len(SUPPORTED_TRAINING_LABELS),
            "supported_import_strategies": ["replace", "merge"],
            "snapshot_slot_limit": SNAPSHOT_SLOT_LIMIT,
            "snapshot_history_limit": SNAPSHOT_HISTORY_LIMIT,
            "snapshot_slot_metadata_fields": ["slot", "label", "note", "source"],
        }

    def export_memory(self) -> dict[str, Any]:
        summary = self.memory_state_summary()
        return self._normalize_memory_snapshot(
            {
                "schema_version": MEMORY_STATE_SCHEMA_VERSION,
                "model_version": MEMORY_MODEL_VERSION,
                "exported_unix_ms": self._now_unix_ms(),
                "pattern_count": summary["pattern_count"],
                "label_count": summary["label_count"],
                "pattern_labels": self.pattern_labels,
                "source": "live_export",
            },
            require_snapshot_wrapper=False,
        ) | {"status": "empty" if summary["label_count"] == 0 else "exported"}

    def import_memory(self, payload: dict[str, Any]) -> dict[str, Any]:
        strategy = str(payload.get("strategy", "replace"))
        if strategy not in {"replace", "merge"}:
            raise ValueError("strategy must be one of: replace, merge")
        snapshot = self._normalize_memory_snapshot(payload, require_snapshot_wrapper="snapshot" in payload)
        incoming_pattern_labels = snapshot["pattern_labels"]
        snapshot_label = clean_optional_text(payload.get("label")) or snapshot.get("label")
        snapshot_note = clean_optional_text(payload.get("note")) or snapshot.get("note")
        snapshot_source = clean_optional_text(payload.get("source")) or snapshot.get("source")
        if strategy == "replace":
            self.pattern_labels = incoming_pattern_labels
        else:
            self.pattern_labels = self._merge_pattern_labels(self.pattern_labels, incoming_pattern_labels)
        save_as_slot = payload.get("save_as_slot")
        if save_as_slot is not None:
            self.save_slot(
                str(save_as_slot),
                label=snapshot_label,
                note=snapshot_note,
                source=snapshot_source,
                record_history=False,
            )
        self._save()
        summary = self.memory_state_summary()
        summary["status"] = "loaded" if summary["label_count"] > 0 else "cleared"
        summary["imported_pattern_count"] = summary["pattern_count"]
        summary["imported_label_count"] = summary["label_count"]
        summary["imported_unix_ms"] = self._now_unix_ms()
        summary["strategy"] = strategy
        summary["label"] = snapshot_label
        summary["note"] = snapshot_note
        summary["source"] = snapshot_source
        self._record_snapshot_history(
            "import_memory",
            str(save_as_slot or ""),
            strategy,
            snapshot_label,
            snapshot_note,
            snapshot_source,
            int(summary["imported_pattern_count"]),
            int(summary["imported_label_count"]),
        )
        self._save()
        return summary

    def memory_versions(self) -> dict[str, Any]:
        slots = []
        for slot, snapshot in sorted(
            self.snapshot_slots.items(),
            key=lambda item: (int(item[1].get("exported_unix_ms", 0)), item[0]),
            reverse=True,
        ):
            slots.append(
                {
                    "slot": slot,
                    "saved_unix_ms": int(snapshot.get("exported_unix_ms", 0)),
                    "pattern_count": int(snapshot.get("pattern_count", 0)),
                    "label_count": int(snapshot.get("label_count", 0)),
                    "last_trained_unix_ms": snapshot.get("last_trained_unix_ms"),
                    "label": snapshot.get("label"),
                    "note": snapshot.get("note"),
                    "source": snapshot.get("source"),
                }
            )
        return {
            "status": "ready",
            "schema_version": MEMORY_STATE_SCHEMA_VERSION,
            "model_version": MEMORY_MODEL_VERSION,
            "slot_count": len(slots),
            "slots": slots,
            "history": self.snapshot_history,
        }

    def save_slot(
        self,
        slot: str,
        label: str | None = None,
        note: str | None = None,
        source: str | None = None,
        record_history: bool = True,
    ) -> dict[str, Any]:
        slot = slot.strip()
        if not slot:
            raise ValueError("slot must not be empty")
        snapshot = self.export_memory()
        snapshot["slot"] = slot
        snapshot["label"] = clean_optional_text(label)
        snapshot["note"] = clean_optional_text(note)
        snapshot["source"] = clean_optional_text(source) or "manual"
        self.snapshot_slots[slot] = snapshot
        self._trim_snapshot_slots()
        if record_history:
            self._record_snapshot_history(
                "save_slot",
                slot,
                "replace",
                snapshot.get("label"),
                snapshot.get("note"),
                snapshot.get("source"),
                int(snapshot["pattern_count"]),
                int(snapshot["label_count"]),
            )
        self._save()
        return {
            "status": "saved",
            "slot": slot,
            "label": snapshot.get("label"),
            "note": snapshot.get("note"),
            "source": snapshot.get("source"),
            "pattern_count": int(snapshot["pattern_count"]),
            "label_count": int(snapshot["label_count"]),
            "saved_unix_ms": int(snapshot["exported_unix_ms"]),
            "slot_count": len(self.snapshot_slots),
        }

    def load_slot(self, slot: str, strategy: str = "replace") -> dict[str, Any]:
        slot = slot.strip()
        if not slot:
            raise ValueError("slot must not be empty")
        snapshot = self.snapshot_slots.get(slot)
        if snapshot is None:
            raise ValueError(f"unknown slot '{slot}'")
        result = self.import_memory({"strategy": strategy, "snapshot": snapshot})
        result["slot"] = slot
        self._record_snapshot_history(
            "load_slot",
            slot,
            strategy,
            snapshot.get("label"),
            snapshot.get("note"),
            snapshot.get("source"),
            int(result["imported_pattern_count"]),
            int(result["imported_label_count"]),
        )
        self._save()
        return result

    def delete_slot(self, slot: str) -> dict[str, Any]:
        slot = slot.strip()
        if not slot:
            raise ValueError("slot must not be empty")
        snapshot = self.snapshot_slots.pop(slot, None)
        if snapshot is None:
            raise ValueError(f"unknown slot '{slot}'")
        self._record_snapshot_history(
            "delete_slot",
            slot,
            "replace",
            snapshot.get("label"),
            snapshot.get("note"),
            snapshot.get("source"),
            int(snapshot.get("pattern_count", 0)),
            int(snapshot.get("label_count", 0)),
        )
        self._save()
        return {
            "status": "deleted",
            "slot": slot,
            "label": snapshot.get("label"),
            "note": snapshot.get("note"),
            "source": snapshot.get("source"),
            "deleted_pattern_count": int(snapshot.get("pattern_count", 0)),
            "deleted_label_count": int(snapshot.get("label_count", 0)),
            "slot_count": len(self.snapshot_slots),
        }

    def clear_memory(self) -> dict[str, Any]:
        before = self.memory_state_summary()
        self.pattern_labels = {}
        self._save()
        cleared = self.memory_state_summary()
        cleared["status"] = "cleared"
        cleared["cleared_pattern_count"] = int(before["pattern_count"])
        cleared["cleared_label_count"] = int(before["label_count"])
        return cleared
