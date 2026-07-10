# Reference: IR Lowering Contract

Use this page when you need the current contract candidate for how `gewylang`
input is lowered into `gewy` compiler IR, and how that lowered shape is
reported by `gewyc explain --focus ir`.

This page is not a tutorial. It is the exact lookup shelf for:

- what the lowering step is responsible for
- which lowered model surfaces exist today
- which fields are intended to be stable enough for tooling and review
- how to read `ir_lowering_delta` and per-model lowered summaries

Read this alongside:

- [docs/dsl.md](docs/dsl.md)
- [docs/gewyc-json.md](docs/gewyc-json.md)
- [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)

## What Lowering Means Here

In the current pipeline, lowering is the step that turns front-end package and
function structure into explicit rule-bearing models.

At a high level:

```text
.gewy package
  -> parse tree
  -> expanded frontend module graph
  -> TemplateBinding
  -> lowered IR models
  -> diagnostics / runtime planning / export-facing behavior
```

Lowering does not mean:

- generating eBPF
- inventing new fragment capabilities
- bypassing fragment-support validation

Lowering does mean:

- selecting the effective template/program shape
- materializing explicit program rules
- materializing or selecting the reason-model shape
- preserving enough structure to explain modules, phases, and support state

## Current Lowered Model Surfaces

Today the focused IR report can expose two lowered model surfaces:

### `program_model`

The lowered program-facing rule model.

This is the main answer to:

- what behavior did this package lower into?
- how many program rules exist?
- which module and phase names were materialized?

Expected shape:

- `id`
- `kind`
- `rules`
- `operations`
- rule-level `module`
- rule-level `phase`
- rule-level `phase_kind`
- support state from diagnostics

### `reason_model`

The lowered reason-facing explanatory model.

This may be either:

- `builtin_reason_profile`
- `declarative_reason_model`

This is the main answer to:

- where explanatory narratives are coming from
- whether reasoning is built-in or declarative
- whether the lowered reason path tracks the same module/phase surface as the
  program path

## Lowering Responsibilities

The current lowering layer is expected to preserve these properties:

1. Front-end structure remains explainable after lowering.
2. Lowered rules remain attributable to a module and phase story.
3. Diagnostics can still answer whether each lowered rule is supportable from
   the chosen fragment set.
4. Program and reason surfaces can be inspected separately.

The lowering layer is not expected to preserve every source-level spelling or
editor-oriented formatting detail.

## `gewyc explain --focus ir`

For exact inspection, use:

```bash
cargo run -p gewyc -- explain dsl/http_request_path.gewy --focus ir
```

For machine-facing inspection, use:

```bash
cargo run -p gewyc -- explain dsl/http_request_path.gewy --focus ir --json
```

For the deliberately compact archival form, use:

```bash
cargo run --bin gewyc_ir_snapshot -- dsl/http_request_path.gewy --json
```

The focused IR view is the preferred surface when you need:

- protocol review
- IR evolution work
- lowered rule-count comparisons
- supportability inspection without reading the entire diagnostics report first

## Text Contract Candidate

The text form is intended to stay structured enough for human review and light
shell parsing.

Important current line families include:

- `program_model=...`
- `program_model_operation=...`
- `reason_model=...`
- `phase=... phase_kind=...`
- `ir_delta.frontend_*`
- `ir_delta.lowered_*`
- `ir_delta.model.program_model.*`
- `ir_delta.model.reason_model.*`

The new per-model lowered summary lines intentionally give a compact contract
for each lowered model:

- `ir_delta.model.<label>.id`
- `ir_delta.model.<label>.kind`
- `ir_delta.model.<label>.rules`
- `ir_delta.model.<label>.supported_rules`
- `ir_delta.model.<label>.unsupported_rules`
- `ir_delta.model.<label>.modules`
- `ir_delta.model.<label>.phases`

Where `<label>` is currently one of:

- `program_model`
- `reason_model`

## JSON Contract Candidate

The JSON form is the better choice for editor tooling and review automation.

The focused IR report currently centers on:

- `program_model`
- `reason_model`
- `ir_lowering_delta`
- `model_compare`
- `history_snapshot`
- `ir_shape_note`

Within `ir_lowering_delta`, the current compact compare view includes:

- front-end counts
- lowered rule counts
- supported and unsupported rule counts
- lowered module names
- lowered phase names
- lowered phase kinds
- `lowered_models`

`lowered_models` is the structured per-model summary list. Each entry currently
contains:

- `label`
- `id`
- `kind`
- `rule_count`
- `supported_rule_count`
- `unsupported_rule_count`
- `modules`
- `phases`

This makes it possible to answer both of these questions without scanning the
full rule list:

1. Did the front-end lower into the model shape I expected?
2. Did the lowered `program_model` and `reason_model` stay aligned?

### `protocol_ir`

Runtime export bundles now include a compact protocol-facing IR summary named
`protocol_ir`.

This is derived from the lowered program flow operation and the current
protocol-surface registry. It lets downstream tools ask "which protocol family
did this lowered flow become?" without re-reading package manifests or
reconstructing alias rules.

Each `protocol_ir` entry currently keeps:

- `operation`
- `protocol`
- `entry`
- `default_entry`
- `selected_is_default`
- `sibling_entries`
- `cluster_key`
- `cluster_label`
- `shelf_key`
- `shelf_label`
- `semantics_category`
- `operator_focus`
- `typical_signal`

The field is additive and optional for older exported JSON. New exports should
preserve it through JSON round-trips and replay so orchestration, UI panels,
and release snapshots can consume the same protocol classification surface.

### `history_snapshot`

`history_snapshot` is the deliberately compact archival form of the focused IR
surface.

It is meant for:

- minor-line release notes
- contract review diffs
- future snapshot tooling that wants a smaller, version-record-friendly shape

Its current shape mirrors the stable parts of the IR view:

- `template_id`
- `operation`
- `program_model`
- `reason_model`
- `model_compare`

Each model snapshot currently keeps:

- `id`
- `kind`
- `rule_count`
- `supported_rule_count`
- `unsupported_rule_count`
- `modules`
- `phases`

This is intentionally narrower than the full rule list. It exists so later
historical records can say "this was the lowered shape of the line" without
copying every rule-level detail into the archival layer.

If you want a Markdown-ready block for a release-history page, the repository
also carries:

```bash
scripts/history/render_minor_line_ir_snapshot.sh \
  --title "v0.15.x IR Baseline" \
  amqp-publish=protocols/amqp/publish/main.gewy
```

## Stability Guidance

For the `0.13.*` baseline and the current `0.20.x` maturity track, the intended
practical stability is:

- the existence of focused IR inspection is deliberate
- the distinction between `program_model` and `reason_model` is deliberate
- module/phase/phase-kind visibility is deliberate
- `ir_lowering_delta` as a compact compare surface is deliberate
- `model_compare` as the direct program-vs-reason alignment summary is deliberate
- `history_snapshot` as the archival lowered-shape summary is deliberate
- `lowered_models` as a per-model summary surface is deliberate
- `protocol_ir` as the runtime/export protocol classification surface is deliberate

Still evolving:

- exact note wording in `ir_shape_note`
- incidental ordering if new additive summaries are introduced
- how much auxiliary metadata each full rule entry carries

Tooling should prefer:

- explicit JSON fields
- explicit text keys beginning with `ir_delta.`

Tooling should avoid depending on:

- prose wording around the summaries
- whole-line ordering when additive fields would still preserve key names

## Review Checklist

When reviewing a protocol or IR change, start here:

1. Did the expected `program_model` appear?
2. Did the expected `reason_model` appear?
3. Do the lowered `modules` and `phases` match the intended package shape?
4. Are supported and unsupported counts what the fragment inventory implies?
5. If the reason model is declarative, does it still track the same lowered
   path shape as the program model?

If one of those answers is unclear, the change is usually not yet documented
or explained well enough.

## Non-Goals For This Page

This page does not attempt to replace:

- the full `gewylang` syntax reference
- the JSON field-by-field `gewyc` output guide
- the runtime/export contract notes

It exists to keep the lowered IR shape legible as a first-class project
surface instead of leaving it spread across parser notes, diagnostics notes,
and protocol examples.
