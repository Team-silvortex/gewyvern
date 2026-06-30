# Reference

This track is for exact lookup. Use it when you need syntax, schema, or a
stable contract, not a walkthrough.

If you only need runnable commands or the right validation script instead of an
exact contract page, start with:

- [docs/cli-recipes.md](docs/cli-recipes.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)

If the question is about release-line direction rather than a stable contract,
start with:

- [ROADMAP.md](ROADMAP.md)
- [docs/history/v0.17.x.md](docs/history/v0.17.x.md)
- [docs/history/v0.17.x-midline-checklist.md](docs/history/v0.17.x-midline-checklist.md)
- [docs/history/v0.15-to-v1-roadmap.md](docs/history/v0.15-to-v1-roadmap.md)

## Book Path

This section works best as Part-by-part lookup, not as one long list.

A good order is:

1. language and compiler
2. protocol registry and family shelves
3. runtime and export contracts
4. semantics
5. extensibility

If you are in the protocol volume specifically, start with:

- [docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

## Language And Compiler

- [docs/gewylang-system.md](docs/gewylang-system.md)
  System map for the `gewylang` documentation shelves and reading order.
- [docs/gewylang-evolution.md](docs/gewylang-evolution.md)
  Implementation roadmap for the language/frontend/lowering/runtime spine.
- [docs/book/reference-gewylang-package.md](docs/book/reference-gewylang-package.md)
  Current package shape, `gewy.pkg`, `include(...)`, function units, and
  `use(...)` parameter boundary rules.
- [docs/dsl.md](docs/dsl.md)
  Entry map for the stable `gewylang` shelf.
- [docs/dsl-syntax.md](docs/dsl-syntax.md)
  Stable syntax, package shape, function units, and pipeline structure.
- [docs/dsl-reference.md](docs/dsl-reference.md)
  Exact DSL vocabulary, compatibility surface, predicates, and parameter
  schema.
- [docs/gewylang.ebnf](docs/gewylang.ebnf)
  Draft formal grammar.
- [docs/gewyc-json.md](docs/gewyc-json.md)
  `gewyc` frontend/explain JSON output.
- [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
  Field-by-field bless/compat/evolving contract shelf for `gewyc` JSON.
- [docs/gewyc-contract-matrix.md](docs/gewyc-contract-matrix.md)
  Matrix from grouped contract shelves to the primary real fixture files.
- [docs/gewyc-freeze-checklist.md](docs/gewyc-freeze-checklist.md)
  Short release-line freeze ritual for tightening grouped fields without losing compatibility discipline.
- [docs/gewyc-sample-index.md](docs/gewyc-sample-index.md)
  Sample-first index for real success/failure `gewyc` JSON fixtures.
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
  Current lowering contract candidate for `program_model`, `reason_model`, and
  `ir_lowering_delta`.
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
  Registry and CLI contract candidate for canonical protocol families,
  entries, aliases, and default resolution.
- [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
  Higher-level grouping index for choosing the right protocol family shelf.
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
  Directory for the narrower built-in protocol family shelves such as Redis,
  FTP, SMTP, MQTT, LDAP, and PostgreSQL.

## Protocol Reference Volume

- [docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
  Front door for choosing whether you need contract, examples, commands,
  validation, operator triage, or release judgment first.
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
  Start here for canonical family, entry, alias, and default-entry rules.
- [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
  Use this when you know the traffic shape but not the exact family hub yet.
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
  Directory of exact family hubs and narrower family subpages.
- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)
  Reading spine for choosing whether to continue into contract, explanation,
  package-debug, or runtime-validation material next.
- [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
  Family-to-script map for choosing the right validation command once you
  already know the protocol shelf you care about.
- [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)
  Family-to-sample map for jumping from protocol hubs into the nearest real
  `.gewy` examples and walkthroughs.
- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
  Family-to-command map for jumping from hubs into the shortest real CLI and
  `--serve` validation routes.
- [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
  Operator/release triage playbook for turning family suspicion into runtime,
  grouped-validation, and release-confidence checks.
- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)
  Machine-readable jump contract for surfaces that should send the reader into a
  companion protocol/entry shelf next.
- [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)
  Protocol-facing minor-line handbook for deciding when family,
  runtime, packaged, and cross-project confidence is strong enough to ship.
- [docs/book/reference-protocol-alias-index.md](docs/book/reference-protocol-alias-index.md)
  Generated-style alias lookup for the current built-in registry surface.
- [docs/book/reference-dot-overlay.md](docs/book/reference-dot-overlay.md)
  Compact overlay reading path for DNS-over-TLS on top of the DNS TCP and TLS shelves.
- [docs/book/reference-doh-overlay.md](docs/book/reference-doh-overlay.md)
  Compact overlay reading path for DNS-over-HTTPS on top of the HTTP request shelf.

## Runtime And Export Contracts

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
  Config file search order, supported sections, override precedence, and
  legacy fallback behavior for the `0.15.x` line.
- [docs/runtime-config-contract.md](docs/runtime-config-contract.md)
  Narrow machine-facing contract candidate for config search order, section/key surface, and compatibility fallback.
- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)
  Stable runtime certificate policy statuses, reason codes, and operator-facing
  interpretation rules for the certificate shelf.
- [docs/runtime-certificate-policy-contract.md](docs/runtime-certificate-policy-contract.md)
  Narrow machine-facing contract candidate for policy status words, reason codes, and additive evolution posture.
- [docs/book/reference-runtime-certificate-state.md](docs/book/reference-runtime-certificate-state.md)
  Runtime-managed certificate rotation and revocation shelf layout plus the
  certificate-state API contract.
- [docs/book/reference-runtime-events.md](docs/book/reference-runtime-events.md)
  Stable runtime event names, structured log shape, and event-naming contract
  candidates for the `0.16.x` tightening line.
- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
  Standard config/data/state/cache/share roots, environment overrides, and
  legacy upgrade compatibility rules for the `0.15.x` line.
- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
  Exact meanings for `primary_failure_*`, guidance, and ambiguity fields.
- [docs/book/reference-training-dataset-consumption.md](docs/book/reference-training-dataset-consumption.md)
  Exact fetch order, stable IDs, split hints, and consumption rules for the
  training dataset and sample export surfaces.
- [docs/export-format.md](docs/export-format.md)
  Export bundle structure.
- [docs/export-format-contract.md](docs/export-format-contract.md)
  Narrow machine-facing contract candidate for replay-critical bundle fields and summary-vs-replay discipline.
- [docs/machine-contract.md](docs/machine-contract.md)
  Machine-facing JSON and API contract candidate.
- [docs/machine-surface-freeze.md](docs/machine-surface-freeze.md)
  Shared freeze ritual for compiler, runtime-config, certificate, and export machine surfaces.
- [docs/surface-stability.md](docs/surface-stability.md)
  Stable versus intentionally unstable surfaces.

For the broader durable behavior and trust notes that sit beside these exact
contracts, also keep nearby:

- [docs/service-behavior.md](docs/service-behavior.md)
- [docs/security-posture.md](docs/security-posture.md)

## Semantics

- [docs/failure-semantics.md](docs/failure-semantics.md)
  Failure labels, basis, and confidence.
- [docs/process-profiles.md](docs/process-profiles.md)
  Process-network profile meaning.
- [docs/fragments.md](docs/fragments.md)
  Fragment capabilities and attach semantics.

## Extensibility

- [docs/external-engine-contract.md](docs/external-engine-contract.md)
  External analysis interface.
- [docs/fixtures/external_engine_input_example.json](docs/fixtures/external_engine_input_example.json)
  Example external-engine input.
- [docs/fixtures/external_engine_output_example.json](docs/fixtures/external_engine_output_example.json)
  Example external-engine output.

## Future Shape

As the active `0.17.x` line closes its second half, new exact-lookup material should prefer
this shelf instead of adding more ad hoc “format note” pages at the top level.

By contrast, runnable command collections and operator script routing should
prefer the top-level lookup shelves:

- [docs/cli-recipes.md](docs/cli-recipes.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
