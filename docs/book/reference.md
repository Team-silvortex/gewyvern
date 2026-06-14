# Reference

This track is for exact lookup. Use it when you need syntax, schema, or a
stable contract, not a walkthrough.

## Book Path

This section works best as Part-by-part lookup, not as one long list.

A good order is:

1. language and compiler
2. protocol registry and family shelves
3. runtime and export contracts
4. semantics
5. extensibility

If you are in the protocol volume specifically, start with:

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
  `gewylang` syntax, function units, pipeline structure, package shape, and
  stable subset.
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

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Start here for canonical family, entry, alias, and default-entry rules.
- [docs/book/reference-protocol-groups.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-groups.md)
  Use this when you know the traffic shape but not the exact family hub yet.
- [docs/book/reference-protocol-family-shelves.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-family-shelves.md)
  Directory of exact family hubs and narrower family subpages.
- [docs/book/reference-protocol-alias-index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-alias-index.md)
  Generated-style alias lookup for the current built-in registry surface.

## Runtime And Export Contracts

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

As the current `0.14.x` line continues, new exact-lookup material should prefer
this shelf instead of adding more ad hoc “format note” pages at the top level.
