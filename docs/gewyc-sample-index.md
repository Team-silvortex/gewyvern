# gewyc Sample Index

Use this page when you want the shortest path from a `gewyc` surface question
to a real JSON sample.

This page is a lookup shelf for sample assets, not the place where field
semantics are fully explained.

For the actual surface contract, use:

- [docs/gewyc-json.md](docs/gewyc-json.md)

## Why This Page Exists

The fixture directory now contains enough `gewyc` examples that browsing by
filename alone is no longer the easiest reading path.

This page answers:

- which command produced the sample?
- is the sample a success path or a failure path?
- which surface family does it represent?
- which sample should a UI, script, or docs page point at first?

## Reading Rule

Choose samples in this order:

1. success path for shape discovery
2. focused success path for narrowed consumers
3. parse failure for source-local diagnostics
4. validation failure for coverage/rule-support diagnostics

## Surface Samples

### Frontend Success

- file:
  [docs/fixtures/gewyc_frontend_udp_process_debug.json](docs/fixtures/gewyc_frontend_udp_process_debug.json)
- command:
  `cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --json`
- input:
  [dsl/udp_process_debug.gewy](dsl/udp_process_debug.gewy)
- meaning:
  Healthy pipeline/frontend summary with graph, `use(...)` edge, expansion
  preview, and grouped `authoring` / `counts`.
- use this when:
  You are wiring a frontend graph panel, package-review view, or authoring
  summary card.

### Stages Success

- file:
  [docs/fixtures/gewyc_stages_udp_process_debug.json](docs/fixtures/gewyc_stages_udp_process_debug.json)
- command:
  `cargo run -p gewyc -- stages dsl/udp_process_debug.gewy --json`
- input:
  [dsl/udp_process_debug.gewy](dsl/udp_process_debug.gewy)
- meaning:
  Healthy phase spine with grouped `summary` / `status` / `counts` plus full
  `parse`, `validation`, and `diagnostics` sections.
- use this when:
  You want a phase-gating view before opening `explain`.

### Envelope Success

- file:
  [docs/fixtures/gewyc_envelope_udp_process_debug.json](docs/fixtures/gewyc_envelope_udp_process_debug.json)
- command:
  `cargo run -p gewyc -- envelope dsl/udp_process_debug.gewy --json`
- input:
  [dsl/udp_process_debug.gewy](dsl/udp_process_debug.gewy)
- meaning:
  Healthy aggregate compiler view with grouped `summary`, grouped nested
  `surfaces`, and compatibility mirrors for `binding`, `diagnostics`,
  `findings`, and `stages`.
- use this when:
  You want one top-level routing sample before deciding which nested surface to
  open.

### Explain Validation Focus Success

- file:
  [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](docs/fixtures/gewyc_explain_validation_udp_process_debug.json)
- command:
  `cargo run -p gewyc -- explain dsl/udp_process_debug.gewy --json --focus validation`
- input:
  [dsl/udp_process_debug.gewy](dsl/udp_process_debug.gewy)
- meaning:
  Healthy umbrella explain output plus a focused validation view.
- use this when:
  You want the canonical success-path example for grouped `summary`,
  `focused_report`, `shape_notes`, and `excerpts`.

## Failure Samples

### Explain Parse Failure

- file:
  [docs/fixtures/gewyc_explain_parse_failure.json](docs/fixtures/gewyc_explain_parse_failure.json)
- source shape:
  Temporary malformed input with `fn broken( =`
- command family:
  `cargo run -p gewyc -- explain <bad.gewy> --json`
- meaning:
  Parse-stage failure with `summary.stage_status.parse = false` and a concrete
  `parse_source` excerpt/marker.
- use this when:
  You are building editor diagnostics, parse failure banners, or pre-commit
  DSL checks.

### Findings Parse Failure

- file:
  [docs/fixtures/gewyc_findings_parse_failure.json](docs/fixtures/gewyc_findings_parse_failure.json)
- source shape:
  Minimal malformed pipeline with `|> oops(:true)`
- command family:
  `cargo run -p gewyc -- findings <bad.gewy> --json`
- meaning:
  Standalone findings failure with grouped `summary.finding_count`,
  grouped `summary.next_step`, and one exact parse finding record.
- use this when:
  You want the smallest machine-facing finding sample without the larger
  `explain` shell.

### Explain Validation Failure

- file:
  [docs/fixtures/gewyc_explain_validation_failure.json](docs/fixtures/gewyc_explain_validation_failure.json)
- source shape:
  Minimal `snmp_query` example that asks for unsupported payload offset `42`
- command family:
  `cargo run -p gewyc -- explain <bad.gewy> --json`
- meaning:
  Validation-stage failure with:
  `summary.stage_status.validation = false`,
  grouped `shape_notes.validation`,
  grouped `excerpts.validation`,
  and matching diagnostics detail.
- use this when:
  You are building payload-coverage hints, rule-support drilldowns, or
  “why did validation fail?” operator panels.

## Suggested Consumer Mapping

### Shell / jq

Start with:

- envelope success
- stages success
- findings parse failure
- explain parse failure
- explain validation failure

These three give the smallest reliable set for:

- aggregate routing
- phase gating
- standalone findings routing
- source-local parse diagnostics
- payload coverage diagnostics

### Editor / IDE

Start with:

- explain parse failure
- explain validation failure
- frontend success

This gives one source-local failure sample, one semantic/coverage failure
sample, and one healthy authoring/graph sample.

### Lese / panel UI

Start with:

- envelope success
- explain validation focus success
- stages success
- frontend success

This gives one aggregate summary shape, one umbrella summary shape, one phase
spine, and one detailed authoring graph shape.

## Maintenance Rule

When adding a new `gewyc` fixture:

1. prefer one sample per distinct consumer need
2. prefer naming by surface plus scenario
3. add it here in the same patch
4. link it from [docs/gewyc-json.md](docs/gewyc-json.md)
   if it helps explain a contract branch

Do not add near-duplicate success samples unless they introduce a genuinely new
surface pattern.
