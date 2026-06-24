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

## Design Rule

The current JSON direction follows one simple rule:

1. add structured groups first
2. keep older flat fields during the tightening line
3. let callers migrate toward grouped reads
4. remove compatibility duplicates only in a clearly announced later line

That means many surfaces intentionally expose both:

- a grouped shape such as `status`, `counts`, `analysis`, `shape_notes`, or
  `excerpts`
- older flat fields such as `template_id`, `program_model`, `finding`, or
  `module_doc`

When both exist, new consumers should prefer the grouped shape first.

## Stable Reading Heuristic

For most surfaces, read in this order:

1. `status`
2. `counts`
3. `analysis`
4. `shape_notes`
5. `excerpts`
6. `report` or legacy flat fields

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
```

Grouped fields to prefer:

- `report.status.present`
- `report.authoring`
- `report.counts`

Legacy fields still present:

- `report.module_doc`
- `report.template_doc`
- `report.function_count`
- `report.merged_step_count`
- `report.function_nodes`
- `report.include_sources`
- `report.use_edges`
- `report.graph_nodes`
- `report.graph_edges`
- `report.expansion_previews`

## Binding Surface

Command example:

```bash
cargo run -p gewyc -- binding /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `status.has_window`
- `status.has_reason_profile`
- `status.has_program_model`
- `counts.fragments`
- `counts.fragment_params`
- `counts.evidence_overrides`

Legacy fields still present:

- `template_id`
- `fragments`
- `window`
- `reason_profile`
- `program_model`
- `fragment_params`
- `evidence_overrides`

Use the grouped fields when you only need posture.
Use the legacy fields when you need exact binding detail.

## Diagnostics Surface

Command example:

```bash
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `status.has_program_model`
- `status.has_reason_model`
- `counts.fragments`
- `counts.program_rules`
- `counts.reason_rules`

Legacy fields still present:

- `template_id`
- `fragments`
- `program_model`
- `reason_model`

Each model still carries exact per-rule diagnostics under `rules[]`.

## Findings Surface

Command example:

```bash
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Current shape:

```json
{
  "findings": []
}
```

This is intentionally narrow.

Treat `findings[]` as the stable contract.

## Stages Surface

Command example:

```bash
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Full fixture:

- [docs/fixtures/gewyc_stages_udp_process_debug.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_stages_udp_process_debug.json)

Grouped fields to prefer:

- `status.parse_ok`
- `status.validation_ok`
- `status.diagnostics_ok`
- `counts.validation_fragments`
- `counts.validation_program_rules`
- `counts.validation_reason_rules`
- `counts.sampled_payload_offsets`
- `counts.required_payload_offsets`
- `counts.unsupported_payload_offsets`

Detailed phase sections remain:

- `parse`
- `validation`
- `diagnostics`

This surface is the best machine-readable phase spine below `explain`.

## Envelope Surface

Command example:

```bash
cargo run -p gewyc -- envelope /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Grouped fields to prefer:

- `status.has_binding`
- `status.has_diagnostics`
- `status.finding_count`
- `surfaces.binding`
- `surfaces.diagnostics`
- `surfaces.findings`
- `surfaces.stages`

Compatibility fields remain at top level:

- `binding`
- `diagnostics`
- `findings`
- `stages`

The `surfaces` object is the preferred grouped entry point for new consumers.

## IR Surface

Command example:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus ir
```

Grouped fields to prefer:

- `status.has_program_model`
- `status.has_reason_model`
- `status.has_model_compare`
- `counts.program_rules`
- `counts.reason_rules`
- `analysis.model_compare`
- `analysis.history_snapshot`

Legacy fields remain:

- `template_id`
- `program_model`
- `reason_model`
- `model_compare`
- `history_snapshot`

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
  "ok": true,
  "summary": { "...": "..." },
  "focused_report": null,
  "frontend": { "...": "..." },
  "binding": { "...": "..." },
  "validation": { "...": "..." },
  "diagnostics": { "...": "..." },
  "findings": { "...": "..." }
}
```

Prefer these grouped fields in `summary`:

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

### `jq`: gate on `explain.summary.stage_status`

Use this when you want one command that decides whether the source is healthy
enough to continue.

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json \
  | jq '.summary.stage_status'
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
  | jq '.focused_report.excerpts.parse_source'
```

Preferred read:

- `focused_report.status.ok`
- `focused_report.analysis.finding`
- `focused_report.excerpts.parse_source`

Failure fixture:

- [docs/fixtures/gewyc_explain_parse_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_parse_failure.json)

### `jq`: pull the first validation coverage issue

Use this when a tool wants payload-offset posture instead of general findings.

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --focus validation \
  | jq '.focused_report.excerpts.validation'
```

Preferred read:

- `focused_report.status.ok`
- `focused_report.shape_notes.validation`
- `focused_report.excerpts.validation`

Failure fixture:

- [docs/fixtures/gewyc_explain_validation_failure.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/gewyc_explain_validation_failure.json)

### `jq`: pull frontend authoring context

Use this when a review tool wants the documentation posture without reading the
entire frontend graph.

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json \
  | jq '.report.authoring'
```

Preferred read:

- `report.status.present`
- `report.authoring.module_doc`
- `report.authoring.template_doc`
- `report.authoring.documented_functions`

### Editor / diagnostics adapter

For a lightweight editor integration, the practical sequence is:

1. run `gewyc explain <path> --json`
2. read `summary.stage_status`
3. if parse failed, rerun with `--focus parse`
4. if validation failed, rerun with `--focus validation`
5. if diagnostics failed, rerun with `--focus diagnostics`
6. only show `report` detail panes after the stage gate is green

That keeps the editor behavior progressive:

- syntax and authoring failures first
- coverage failures second
- semantic rule-support failures third

### Lese / panel consumer

For a panel-oriented consumer such as `leserpent`, a good default mapping is:

- top strip:
  - `explain.summary.stage_status`
  - `explain.summary.next_step`
- authoring card:
  - `explain.summary.analysis.authoring_context`
- lowering card:
  - `explain.summary.analysis.lowered_binding_summary`
  - `explain.summary.analysis.frontend_lowering_delta`
- diagnostics card:
  - `explain.summary.shape_notes`
  - `explain.summary.excerpts`
- drilldown tabs:
  - `frontend.report`
  - `binding`
  - `validation`
  - `diagnostics`
  - `focused_report`

The important design rule is:

- use grouped objects for panel summaries
- use legacy flat fields only when rendering exact detail blocks

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
