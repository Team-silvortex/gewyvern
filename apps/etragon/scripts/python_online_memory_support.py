from typing import Any


MEMORY_STATE_SCHEMA_VERSION = 1
MEMORY_MODEL_VERSION = "python-online-memory-v1"
WORKER_PROTOCOL_VERSION = 1
SUPPORTED_COMMANDS = [
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
]
SUPPORTED_TRAINING_LABELS = [
    "network_observe_longer",
    "targeted_escalation",
    "http_request_followup",
]
SNAPSHOT_SLOT_LIMIT = 16
SNAPSHOT_HISTORY_LIMIT = 24


def clean_optional_text(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def build_pattern_key(snapshot: dict[str, Any]) -> str:
    return "|".join(
        [
            str(snapshot.get("primary_module_kind", "unknown")),
            str(snapshot.get("primary_failure_mode", "unknown")),
            str(snapshot.get("primary_failure_detail", "unknown")),
            str(snapshot.get("primary_failure_confidence", "unknown")),
            str(snapshot.get("primary_failure_basis", "unknown")),
            "ambiguous" if bool(snapshot.get("ambiguous", False)) else "clear",
        ]
    )


def label_transition_policy(label: str) -> dict[str, list[str]]:
    policies = {
        "network_observe_longer": {
            "compatible_with": ["http_request_followup"],
            "competes_with": ["targeted_escalation"],
        },
        "targeted_escalation": {
            "compatible_with": [],
            "competes_with": ["network_observe_longer", "http_request_followup"],
        },
        "http_request_followup": {
            "compatible_with": ["network_observe_longer"],
            "competes_with": ["targeted_escalation"],
        },
    }
    return policies.get(label, {"compatible_with": [], "competes_with": []})
