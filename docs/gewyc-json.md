# gewyc JSON Surfaces

Use this page when you need the machine-facing contract for `gewyc` output.

This is the reference shelf for the current JSON surfaces, not a tutorial.

Read this page when the question is:

- what does `gewyc ... --json` return right now?
- which top-level groups are stable enough to consume?
- how should a tool read `summary`, `focused_report`, `status`, or `counts`?
- where is the same information repeated for compatibility?

Read these companion pages beside it:

- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
- [docs/gewyc-field-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-field-contract.md)
- [docs/gewyc-sample-index.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-sample-index.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
- [docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md)

Representative fixture snapshots for this page:

- [docs/fixtures/gewyc_frontend_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_frontend_udp_process_debug.json)
- [docs/fixtures/gewyc_stages_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_stages_udp_process_debug.json)
- [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_validation_udp_process_debug.json)
- [docs/fixtures/gewyc_explain_parse_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_parse_failure.json)
- [docs/fixtures/gewyc_explain_validation_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_validation_failure.json)

## Scope

This page covers the JSON emitted by the compiler-facing `gewyc` surfaces:

- `frontend`
- `binding`
- `diagnostics`
- `findings`
- `stages`
- `envelope`
- `explain`
- `ir`

This page does not define the runtime API under `--serve`.

## JSON Wrapper

Every current `gewyc ... --json` surface now uses one shared top-level wrapper:

```json
{
  "surface_id": "gewyc.frontend",
  "schema_hint": {
    "family": "gewyc",
    "surface": "frontend",
    "schema_version": 1
  },
  "contract_hint": {
    "stability": "candidate",
    "compatibility": "grouped_payload_preferred",
    "legacy_fields": "retained_in_payload"
  },
  "payload": {
    "...": "surface-specific body"
  }
}
```

Read the wrapper in this order:

1. `surface_id`
2. `schema_hint.family`
3. `schema_hint.surface`
4. `schema_hint.schema_version`
5. `contract_hint`
6. `payload`

Wrapper meaning:

- `contract_hint.stability = "candidate"`
  Stable enough for real consumers in the current line, but still pre-`1.0.0`.
- `contract_hint.compatibility = "grouped_payload_preferred"`
  New consumers should read grouped objects under `payload` first.
- `contract_hint.legacy_fields = "retained_in_payload"`
  Flat compatibility fields may still exist inside `payload`.

The examples below describe the structure inside `payload`.

## Design Rule

The current JSON direction follows one simple rule:

1. add structured groups first
2. keep older flat fields during the tightening line
3. let callers migrate toward grouped reads
4. remove compatibility duplicates only in a clearly announced later line

## Stable Reading Heuristic

For most surfaces, read in this order:

1. `surface_id`
2. `schema_hint`
3. `contract_hint`
4. `payload.status`
5. `payload.counts`
6. `payload.analysis`
7. `payload.shape_notes`
8. `payload.excerpts`
9. `payload.report` or legacy flat fields

This keeps scripts resilient even when detail payloads widen.

## Frontend Surface

Command examples:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus functions
```

Full fixture:

- [docs/fixtures/gewyc_frontend_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_frontend_udp_process_debug.json)

Top-level shape:

```json
{
  "surface_id": "gewyc.frontend",
  "schema_hint": {
    "family": "gewyc",
    "surface": "frontend",
    "schema_version": 1
  },
  "payload": {
    "summary": {
      "kind": "pipeline",
      "module_doc": null,
      "template_doc": null,
      "function_count": 1,
      "merged_step_count": 8,
      "focus": null
    },
    "focused_report": null,
    "report": {
      "kind": "pipeline",
      "status": { "present": true },
      "authoring": {
        "module_doc": null,
        "template_doc": null,
        "documented_functions": []
      },
      "counts": {
        "functions": 1,
        "merged_steps": 8,
        "includes": 0,
        "use_edges": 1,
        "graph_nodes": 2,
        "graph_edges": 1,
        "expansion_previews": 1
      }
    }
  }
}
```

Grouped fields to prefer:

- `payload.report.status.present`
- `payload.report.authoring`
- `payload.report.counts`

## Binding Surface

Command example:

```bash
cargo run -p gewyc -- binding /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `payload.status.has_window`
- `payload.status.has_reason_profile`
- `payload.status.has_program_model`
- `payload.counts.fragments`
- `payload.counts.fragment_params`
- `payload.counts.evidence_overrides`

Legacy fields still present:

- `payload.template_id`
- `payload.fragments`
- `payload.window`
- `payload.reason_profile`
- `payload.program_model`
- `payload.fragment_params`
- `payload.evidence_overrides`

Use the grouped fields when you only need posture.
Use the legacy fields when you need exact binding detail.

## Diagnostics Surface

Command example:

```bash
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `payload.status.has_program_model`
- `payload.status.has_reason_model`
- `payload.counts.fragments`
- `payload.counts.program_rules`
- `payload.counts.reason_rules`

Legacy fields still present:

- `payload.template_id`
- `payload.fragments`
- `payload.program_model`
- `payload.reason_model`

Each model still carries exact per-rule diagnostics under `rules[]`.

## Findings Surface

Command example:

```bash
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Current shape:

```json
{
  "surface_id": "gewyc.findings",
  "schema_hint": {
    "family": "gewyc",
    "surface": "findings",
    "schema_version": 1
  },
  "payload": {
    "findings": []
  }
}
```

This is intentionally narrow.

Treat `payload.findings[]` as the stable contract.

## Stages Surface

Command example:

```bash
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Full fixture:

- [docs/fixtures/gewyc_stages_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_stages_udp_process_debug.json)

Grouped fields to prefer:

- `payload.status.parse_ok`
- `payload.status.validation_ok`
- `payload.status.diagnostics_ok`
- `payload.counts.validation_fragments`
- `payload.counts.validation_program_rules`
- `payload.counts.validation_reason_rules`
- `payload.counts.sampled_payload_offsets`
- `payload.counts.required_payload_offsets`
- `payload.counts.unsupported_payload_offsets`

Detailed phase sections remain:

- `payload.parse`
- `payload.validation`
- `payload.diagnostics`

This surface is the best machine-readable phase spine below `explain`.

## Envelope Surface

Command example:

```bash
cargo run -p gewyc -- envelope /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `payload.status.has_binding`
- `payload.status.has_diagnostics`
- `payload.status.finding_count`
- `payload.surfaces.binding`
- `payload.surfaces.diagnostics`
- `payload.surfaces.findings`
- `payload.surfaces.stages`

Compatibility fields remain at top level:

- `payload.binding`
- `payload.diagnostics`
- `payload.findings`
- `payload.stages`

The `surfaces` object is the preferred grouped entry point for new consumers.

## IR Surface

Command example:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus ir
```

Grouped fields to prefer:

- `payload.status.has_program_model`
- `payload.status.has_reason_model`
- `payload.status.has_model_compare`
- `payload.counts.program_rules`
- `payload.counts.reason_rules`
- `payload.analysis.model_compare`
- `payload.analysis.history_snapshot`

Legacy fields remain:

- `payload.template_id`
- `payload.program_model`
- `payload.reason_model`
- `payload.model_compare`
- `payload.history_snapshot`

## Explain Surface

Command examples:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus frontend
```

Focused validation fixture:

- [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_validation_udp_process_debug.json)

The explain surface is the umbrella machine-facing troubleshooting view.

Top-level shape:

```json
{
  "surface_id": "gewyc.explain",
  "schema_hint": {
    "family": "gewyc",
    "surface": "explain",
    "schema_version": 1
  },
  "payload": {
    "ok": true,
    "summary": { "...": "..." },
    "focused_report": null,
    "frontend": { "...": "..." },
    "binding": { "...": "..." },
    "validation": { "...": "..." },
    "diagnostics": { "...": "..." },
    "findings": { "...": "..." }
  }
}
```

Prefer these grouped fields in `payload.summary`:

- `stage_status`
- `analysis`
- `shape_notes`
- `excerpts`

Compatibility fields remain in parallel:

- `parse_ok`
- `validation_ok`
- `diagnostics_ok`
- `authoring_context`
- `lowered_binding_summary`
- `frontend_lowering_delta`
- `binding_shape_note`
- `validation_shape_note`
- `diagnostics_shape_note`
- `parse_source_excerpt`
- `validation_excerpt`
- `diagnostics_excerpt`

### Explain `focused_report`

Focused JSON now tries to use one shared shell:

- `kind`
- `status`
- `analysis`
- `shape_notes`
- `excerpts`
- `report`

Not every focus uses every group, but new consumers should expect this shell.

Examples:

- parse focus:
  - `status.ok`
  - `analysis.finding`
  - `excerpts.parse_source`
- frontend focus:
  - `status.present`
  - `analysis.authoring_context`
  - `report`
- binding focus:
  - `status.present`
  - `analysis.lowered_binding_summary`
  - `analysis.frontend_lowering_delta`
  - `shape_notes.binding`
- ir focus:
  - `status.present`
  - `analysis.ir_lowering_delta`
  - `shape_notes.ir`
- validation focus:
  - `status.ok`
  - `shape_notes.validation`
  - `excerpts.validation`
- diagnostics focus:
  - `status.ok`
  - `status.present`
  - `shape_notes.diagnostics`
  - `excerpts.diagnostics`

## Consumption Patterns

This section is for practical consumers that want a stable first read without
relearning every surface.

### `jq`: gate on `explain.payload.summary.stage_status`

Use this when you want one command that decides whether the source is healthy
enough to continue.

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json \
  | jq '.payload.summary.stage_status'
```

Typical read pattern:

1. if `parse == false`, stop and inspect parse-focused output
2. if `validation == false`, inspect validation-focused output
3. if `diagnostics == false`, inspect diagnostics-focused output
4. otherwise continue into binding, IR, or runtime validation

### `jq`: pull the first parse excerpt

Use this when an editor or pre-commit hook wants a source-local marker.

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus parse \
  | jq '.payload.focused_report.excerpts.parse_source'
```

Preferred read:

- `payload.focused_report.status.ok`
- `payload.focused_report.analysis.finding`
- `payload.focused_report.excerpts.parse_source`

Failure fixture:

- [docs/fixtures/gewyc_explain_parse_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_parse_failure.json)

### `jq`: pull the first validation coverage issue

Use this when a tool wants payload-offset posture instead of general findings.

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus validation \
  | jq '.payload.focused_report.excerpts.validation'
```

Preferred read:

- `payload.focused_report.status.ok`
- `payload.focused_report.shape_notes.validation`
- `payload.focused_report.excerpts.validation`

Failure fixture:

- [docs/fixtures/gewyc_explain_validation_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_validation_failure.json)

### `jq`: pull frontend authoring context

Use this when a review tool wants the documentation posture without reading the
entire frontend graph.

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json \
  | jq '.payload.report.authoring'
```

Preferred read:

- `payload.report.status.present`
- `payload.report.authoring.module_doc`
- `payload.report.authoring.template_doc`
- `payload.report.authoring.documented_functions`

### Editor / diagnostics adapter

For a lightweight editor integration, the practical sequence is:

1. run `gewyc explain <path> --json`
2. read `payload.summary.stage_status`
3. if parse failed, rerun with `--focus parse`
4. if validation failed, rerun with `--focus validation`
5. if diagnostics failed, rerun with `--focus diagnostics`
6. only show `report` detail panes after the stage gate is green

### Lese / panel consumer

For a panel-oriented consumer such as `leserpent`, a good default mapping is:

- top strip:
  - `explain.payload.summary.stage_status`
  - `explain.payload.summary.next_step`
- authoring card:
  - `explain.payload.summary.analysis.authoring_context`
- lowering card:
  - `explain.payload.summary.analysis.lowered_binding_summary`
  - `explain.payload.summary.analysis.frontend_lowering_delta`
- diagnostics card:
  - `explain.payload.summary.shape_notes`
  - `explain.payload.summary.excerpts`
- drilldown tabs:
  - `explain.payload.frontend`
  - `explain.payload.binding`
  - `explain.payload.validation`
  - `explain.payload.diagnostics`
  - `explain.payload.focused_report`

### Migration rule for existing consumers

If a consumer already reads older flat fields, migrate in this order:

1. switch routing logic to grouped fields
2. keep legacy field reads for fallback rendering
3. stop branching on ad hoc sibling fields
4. treat missing grouped objects as an older-surface compatibility path

This lets consumers become more robust without requiring an all-at-once
rewrite.

## Evolution Rules

If you are adding to a `gewyc` JSON surface, prefer this order:

1. add to a grouped object first
2. keep older flat fields if they are already shipped
3. update tests that lock the grouped contract
4. update this page in the same patch

Do not widen the surface by adding ad hoc sibling fields when one of the
existing groups already matches the meaning.

## Implementation Anchors

These files are the current implementation anchors for the JSON surfaces:

- [src/gewyc/frontend_focus/json.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/frontend_focus/json.rs)
- [src/gewyc/render/surfaces.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/render/surfaces.rs)
- [src/gewyc/explain/render.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/explain/render.rs)
- [src/gewyc/explain_support/focus.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/explain_support/focus.rs)
- [src/gewyc/ir_focus/render.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/ir_focus/render.rs)

These tests currently lock the grouped contract direction:

- [src/gewyc/tests/frontend_surface.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/tests/frontend_surface.rs)
- [src/gewyc/tests/explain_surface.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/tests/explain_surface.rs)
- [src/gewyc/tests/integration.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/tests/integration.rs)
- [src/gewyc/tests/ir.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/tests/ir.rs)
