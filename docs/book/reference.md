# Reference

This track is for exact lookup. Use it when you need syntax, schema, or a
stable contract, not a walkthrough.

If you only need runnable commands or the right validation script instead of an
exact contract page, start with:

- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)

If the question is about release-line direction rather than a stable contract,
start with:

- [ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md)
- [docs/history/v0.15-to-v1-roadmap.md](/Users/Shared/chroot/dev/gewyvern/docs/history/v0.15-to-v1-roadmap.md)

## Book Path

This section works best as Part-by-part lookup, not as one long list.

A good order is:

1. language and compiler
2. protocol registry and family shelves
3. runtime and export contracts
4. semantics
5. extensibility

If you are in the protocol volume specifically, start with:

- [docs/book/reference-protocol-volume.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-volume.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)

## Language And Compiler

- [docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md)
  System map for the `gewylang` documentation shelves and reading order.
- [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
  Implementation roadmap for the language/frontend/lowering/runtime spine.
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
  Current package shape, `gewy.pkg`, `include(...)`, function units, and
  `use(...)` parameter boundary rules.
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Entry map for the stable `gewylang` shelf.
- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
  Stable syntax, package shape, function units, and pipeline structure.
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
  Exact DSL vocabulary, compatibility surface, predicates, and parameter
  schema.
- [docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf)
  Draft formal grammar.
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
  `gewyc` frontend/explain JSON output.
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  Current lowering contract candidate for `program_model`, `reason_model`, and
  `ir_lowering_delta`.
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Registry and CLI contract candidate for canonical protocol families,
  entries, aliases, and default resolution.
- [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
  Higher-level grouping index for choosing the right protocol family shelf.
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
  Directory for the narrower built-in protocol family shelves such as Redis,
  FTP, SMTP, MQTT, LDAP, and PostgreSQL.

## Protocol Reference Volume

- [docs/book/reference-protocol-volume.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-volume.md)
  Front door for choosing whether you need contract, examples, commands,
  validation, operator triage, or release judgment first.
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Start here for canonical family, entry, alias, and default-entry rules.
- [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
  Use this when you know the traffic shape but not the exact family hub yet.
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
  Directory of exact family hubs and narrower family subpages.
- [docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)
  Reading spine for choosing whether to continue into contract, explanation,
  package-debug, or runtime-validation material next.
- [docs/book/reference-protocol-validation-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-validation-paths.md)
  Family-to-script map for choosing the right validation command once you
  already know the protocol shelf you care about.
- [docs/book/reference-protocol-example-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-example-paths.md)
  Family-to-sample map for jumping from protocol hubs into the nearest real
  `.gewy` examples and walkthroughs.
- [docs/book/reference-protocol-command-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-command-paths.md)
  Family-to-command map for jumping from hubs into the shortest real CLI and
  `--serve` validation routes.
- [docs/book/reference-protocol-operator-playbook.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-operator-playbook.md)
  Operator/release triage playbook for turning family suspicion into runtime,
  grouped-validation, and release-confidence checks.
- [docs/book/reference-protocol-reading-companions.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-companions.md)
  Machine-readable jump contract for surfaces that should send the reader into a
  companion protocol/entry shelf next.
- [docs/book/reference-protocol-release-handbook.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-release-handbook.md)
  `0.15.x` protocol-facing minor-line handbook for deciding when family,
  runtime, packaged, and cross-project confidence is strong enough to ship.
- [docs/book/reference-protocol-alias-index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-alias-index.md)
  Generated-style alias lookup for the current built-in registry surface.
- [docs/book/reference-dot-overlay.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dot-overlay.md)
  Compact overlay reading path for DNS-over-TLS on top of the DNS TCP and TLS shelves.
- [docs/book/reference-doh-overlay.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-doh-overlay.md)
  Compact overlay reading path for DNS-over-HTTPS on top of the HTTP request shelf.

## Runtime And Export Contracts

- [docs/book/reference-runtime-config.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-config.md)
  Config file search order, supported sections, override precedence, and
  legacy fallback behavior for the `0.15.x` line.
- [docs/book/reference-runtime-certificate-policy.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-certificate-policy.md)
  Stable runtime certificate policy statuses, reason codes, and operator-facing
  interpretation rules for the certificate shelf.
- [docs/book/reference-runtime-certificate-state.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-certificate-state.md)
  Runtime-managed certificate rotation and revocation shelf layout plus the
  certificate-state API contract.
- [docs/book/reference-runtime-events.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-events.md)
  Stable runtime event names, structured log shape, and event-naming contract
  candidates for the `0.16.x` tightening line.
- [docs/book/reference-runtime-layout.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-runtime-layout.md)
  Standard config/data/state/cache/share roots, environment overrides, and
  legacy upgrade compatibility rules for the `0.15.x` line.
- [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)
  Exact meanings for `primary_failure_*`, guidance, and ambiguity fields.
- [docs/book/reference-training-dataset-consumption.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-training-dataset-consumption.md)
  Exact fetch order, stable IDs, split hints, and consumption rules for the
  training dataset and sample export surfaces.
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
  Export bundle structure.
- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
  Machine-facing JSON and API contract candidate.
- [docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md)
  Stable versus intentionally unstable surfaces.

For the broader durable behavior and trust notes that sit beside these exact
contracts, also keep nearby:

- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)

## Semantics

- [docs/failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/failure-semantics.md)
  Failure labels, basis, and confidence.
- [docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md)
  Process-network profile meaning.
- [docs/fragments.md](/Users/Shared/chroot/dev/gewyvern/docs/fragments.md)
  Fragment capabilities and attach semantics.

## Extensibility

- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)
  External analysis interface.
- [docs/fixtures/external_engine_input_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_input_example.json)
  Example external-engine input.
- [docs/fixtures/external_engine_output_example.json](/Users/Shared/chroot/dev/gewyvern/docs/fixtures/external_engine_output_example.json)
  Example external-engine output.

## Future Shape

As the current `0.15.x` line continues, new exact-lookup material should prefer
this shelf instead of adding more ad hoc “format note” pages at the top level.

By contrast, runnable command collections and operator script routing should
prefer the top-level lookup shelves:

- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
