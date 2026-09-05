# gewyc Contract Matrix

Use this page when you want the shortest bridge between:

- the field contract
- the real fixture files
- the first grouped fields a consumer should read

This page is intentionally compact. It is a routing matrix, not the full
schema explanation.

Use these nearby pages with it:

- [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
- [docs/gewyc-json.md](docs/gewyc-json.md)
- [docs/gewyc-sample-index.md](docs/gewyc-sample-index.md)

## Reading Rule

When adding or reviewing a machine consumer:

1. route with wrapper fields
2. read grouped shelves under `payload` first
3. only then reach for `compat` detail blocks
4. verify the intended path against a real fixture

## Surface Matrix

| Surface | First grouped fields | Primary fixture | Typical consumer |
| --- | --- | --- | --- |
| `frontend` | `payload.report.authoring`, `payload.report.counts` | [docs/fixtures/gewyc_frontend_udp_process_debug.json](docs/fixtures/gewyc_frontend_udp_process_debug.json) | graph panel, authoring review |
| `binding` | `payload.fingerprint`, `payload.status`, `payload.counts` | binding inside [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](docs/fixtures/gewyc_explain_validation_udp_process_debug.json) | lowerer checks, compiler posture, exact identity |
| `diagnostics` | `payload.status`, `payload.counts` | diagnostics inside [docs/fixtures/gewyc_stages_udp_process_debug.json](docs/fixtures/gewyc_stages_udp_process_debug.json) | rule support drilldown |
| `findings` | `payload.findings[]` | findings inside [docs/fixtures/gewyc_explain_validation_failure.json](docs/fixtures/gewyc_explain_validation_failure.json) | editor markers, failure lists |
| `stages` | `payload.status`, `payload.counts` | [docs/fixtures/gewyc_stages_udp_process_debug.json](docs/fixtures/gewyc_stages_udp_process_debug.json) | phase gate, shell checks |
| `envelope` | `payload.status`, `payload.surfaces.*` | envelope branch in `gewyc explain` style docs flow | umbrella routing |
| `explain` | `payload.summary.stage_status`, `payload.summary.analysis`, `payload.summary.shape_notes`, `payload.summary.excerpts` | [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](docs/fixtures/gewyc_explain_validation_udp_process_debug.json) | UI summary, operator panel |
| `ir` | `payload.language_contract`, `payload.fingerprint`, `payload.status`, `payload.counts`, `payload.analysis` | [docs/fixtures/gewyc_ir_udp_process_debug.json](docs/fixtures/gewyc_ir_udp_process_debug.json) | direct model review, supportability inspection, exact identity |
| `ir_history_snapshot` | `payload.language_contract`, `payload.source_ir_fingerprint`, `payload.program_model`, `payload.reason_model`, `payload.model_compare` | IR snapshot path documented in [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md) | archival diff, IR evolution |

## Failure Matrix

| Scenario | Preferred grouped fields | Fixture | Why it matters |
| --- | --- | --- | --- |
| parse failure | `payload.summary.stage_status`, `payload.summary.excerpts.parse_source` | [docs/fixtures/gewyc_explain_parse_failure.json](docs/fixtures/gewyc_explain_parse_failure.json) | editor and pre-commit diagnostics |
| validation failure | `payload.summary.stage_status`, `payload.summary.shape_notes.validation`, `payload.summary.excerpts.validation` | [docs/fixtures/gewyc_explain_validation_failure.json](docs/fixtures/gewyc_explain_validation_failure.json) | payload coverage and rule-support hints |
| healthy explain focus | `payload.summary`, `payload.focused_report` | [docs/fixtures/gewyc_explain_validation_udp_process_debug.json](docs/fixtures/gewyc_explain_validation_udp_process_debug.json) | canonical success-path panel shape |

## Wrapper Checklist

Every machine consumer should verify these first:

- `surface_id`
- `schema_hint.family`
- `schema_hint.surface`
- `schema_hint.schema_version`
- `contract_hint.compatibility`
- `payload`

If these pass, the consumer can move into the grouped surface fields listed in
the matrix above.

## Maintenance Rule

When a grouped field is promoted, tightened, or downgraded:

1. update [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
2. update this matrix if the preferred first-read shelf changes
3. confirm the matching fixture still demonstrates that path
4. keep the fixture regression tests green
