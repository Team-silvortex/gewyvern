#!/usr/bin/env python3
import argparse
import json
import sys
from pathlib import Path

from python_online_memory import OnlineModel


def candidate_for(snapshot: dict[str, object]) -> dict[str, object]:
    module = snapshot.get("primary_module_kind", "unknown")
    mode = snapshot.get("primary_failure_mode", "unknown")
    detail = snapshot.get("primary_failure_detail", "unknown")
    confidence = snapshot.get("primary_failure_confidence", "unknown")
    basis = snapshot.get("primary_failure_basis", "unknown")
    ambiguous = bool(snapshot.get("ambiguous", False))
    competing = list(snapshot.get("competing_hypotheses", []))

    feature_score = 0.0
    feature_score += 1.5 if confidence == "high" else 0.0
    feature_score += 1.0 if confidence == "medium" else 0.0
    feature_score += 1.2 if basis == "direct_protocol_signal" else 0.0
    feature_score += 0.7 if basis == "missing_transition" else 0.0
    feature_score -= 0.8 if ambiguous else 0.0
    feature_score += min(len(competing), 3) * 0.2

    if ambiguous and competing:
        name = "py_ml_candidate_multi_hypothesis"
        summary = (
            "multiple hypotheses remain active; keep this target open for later rerank "
            "or model-assisted clustering"
        )
        data = {
            "module": module,
            "score": round(feature_score, 3),
            "competing_hypotheses": competing,
        }
    elif confidence == "medium" and basis == "missing_transition":
        name = "py_ml_candidate_observe_longer"
        summary = (
            "the current runtime evidence still looks timeout-shaped; collect a slightly "
            "longer observation window before narrowing further"
        )
        data = {
            "module": module,
            "score": round(feature_score, 3),
            "failure_detail": detail,
        }
    elif confidence == "high" and basis == "direct_protocol_signal":
        name = "py_ml_candidate_targeted_escalation"
        summary = (
            "the protocol-level signal is direct enough for downstream escalation "
            "or stronger automated routing"
        )
        data = {
            "module": module,
            "score": round(feature_score, 3),
            "failure_mode": mode,
        }
    else:
        name = "py_ml_candidate_manual_review"
        summary = (
            "the snapshot is still advisory; keep it available for a manual "
            "or model-assisted second pass"
        )
        data = {
            "module": module,
            "score": round(feature_score, 3),
            "failure_confidence": confidence,
            "failure_basis": basis,
        }

    return {
        "kind": "py-ml-candidate",
        "name": name,
        "summary": summary,
        "confidence": "candidate",
        "producer_stage": "candidate",
        "producer_pass": "python_baseline_worker",
        "data": data,
    }


def analyze_snapshot(snapshot: dict[str, object], model: OnlineModel) -> dict[str, object]:
    augmentations = [candidate_for(snapshot)]
    learned = model.learned_candidate(snapshot)
    if learned is not None:
        augmentations.append(learned)
    return {
        "augmentations": augmentations,
        "pattern_memory_state": model.pattern_memory_state(snapshot),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--state-file")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    model = OnlineModel(Path(args.state_file) if args.state_file else None)
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        try:
            if "\t" in line:
                command, payload = line.split("\t", 1)
            else:
                command, payload = "ANALYZE", line
            if command == "ANALYZE":
                snapshot = json.loads(payload)
                output = analyze_snapshot(snapshot, model)
            elif command == "TRAIN":
                envelope = json.loads(payload)
                snapshot = envelope["snapshot"]
                label = str(envelope["label"])
                weight = float(envelope.get("weight", 1.0))
                if weight <= 0.0:
                    raise ValueError("weight must be > 0")
                output = model.train(snapshot, label, weight)
            elif command == "MEMORY_INFO":
                output = model.memory_state_summary()
            elif command == "MODEL_INFO":
                output = model.model_info()
            elif command == "MEMORY_VERSIONS":
                output = model.memory_versions()
            elif command == "MEMORY_EXPORT":
                output = model.export_memory()
            elif command == "MEMORY_IMPORT":
                output = model.import_memory(json.loads(payload))
            elif command == "MEMORY_SAVE_SLOT":
                envelope = json.loads(payload)
                output = model.save_slot(
                    str(envelope["slot"]),
                    envelope.get("label"),
                    envelope.get("note"),
                    envelope.get("source"),
                )
            elif command == "MEMORY_LOAD_SLOT":
                envelope = json.loads(payload)
                output = model.load_slot(
                    str(envelope["slot"]),
                    str(envelope.get("strategy", "replace")),
                )
            elif command == "MEMORY_DELETE_SLOT":
                envelope = json.loads(payload)
                output = model.delete_slot(str(envelope["slot"]))
            elif command == "CLEAR_MEMORY":
                output = model.clear_memory()
            else:
                raise ValueError(f"unknown command: {command}")
            sys.stdout.write("OK\t" + json.dumps(output, separators=(",", ":")) + "\n")
            sys.stdout.flush()
        except Exception as exc:  # pragma: no cover - protocol fallback
            sys.stdout.write("ERR\t" + str(exc).replace("\n", " ") + "\n")
            sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
