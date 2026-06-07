# `gewyc` JSON Surfaces

Use this page when you need the current JSON contract shape for the
human-oriented `gewyc` compiler surfaces.

This page is intentionally a narrow reference. It focuses on the JSON emitted
by the higher-level debugging surfaces most likely to be consumed by editors,
scripts, or lightweight IDE tooling:

- `gewyc frontend --json`
- `gewyc frontend --focus ... --json`
- `gewyc explain --json`
- `gewyc explain --focus ... --json`

It is intentionally narrower than the full compiler envelope. The goal is to
make these higher-level surfaces easy to consume without reverse-engineering
the emitted JSON from source.

This page is not the best place for:

- your first `gewylang` package walkthrough
- the full language surface
- task-oriented validation flows

For those, use:

- [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)

## Stability

These JSON shapes should be treated as:

- current contract candidates for local tooling and editor integration
- small, human-oriented summaries rather than lossless compiler internals
- append-only where practical

Fields may still grow, but consumers should prefer tolerant parsing and ignore
unknown keys.

`--compact` only changes text rendering. It does not change the JSON schema.

## `frontend --json`

Command:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Shape:

```json
{
  "summary": {
    "kind": "pipeline",
    "function_count": 1,
    "merged_step_count": 4,
    "focus": null
  },
  "focused_report": null,
  "report": {
    "kind": "pipeline",
    "function_count": 1,
    "function_nodes": [
      { "name": "network_module", "step_count": 3 }
    ],
    "merged_step_count": 4,
    "include_sources": [],
    "use_edges": [
      { "from": "template", "to": "network_module", "line": 8 }
    ],
    "graph_nodes": [
      { "id": "template", "kind": "template", "step_count": 1 },
      { "id": "network_module", "kind": "function", "step_count": 3 }
    ],
    "graph_edges": [
      { "from": "template", "to": "network_module", "kind": "use", "line": 8 }
    ]
  }
}
```

### `frontend.summary`

- `kind`: current frontend surface kind, currently `pipeline`
- `function_count`: number of declared function units
- `merged_step_count`: steps visible after entry-level pipeline merge
- `focus`: `null` unless `--focus` is used

### `frontend.report`

- `function_nodes`: declared functions with step counts
- `include_sources`: `include(...)` file references
- `use_edges`: `template/use` call edges
- `graph_nodes`: lightweight graph nodes for template/functions/includes
- `graph_edges`: lightweight graph edges for `use`/`include` relationships

## `frontend --focus ... --json`

Command:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph --json
```

`summary` and `report` stay present. `focused_report` becomes a narrowed view.

Example:

```json
{
  "summary": {
    "kind": "pipeline",
    "function_count": 1,
    "merged_step_count": 4,
    "focus": "graph"
  },
  "focused_report": {
    "kind": "graph",
    "graph_nodes": [
      { "id": "template", "kind": "template", "step_count": 1 }
    ],
    "graph_edges": []
  },
  "report": { "...": "full frontend report still present" }
}
```

Supported focus values:

- `functions`
- `includes`
- `graph`

## `explain --json`

Command:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Shape:

```json
{
  "ok": true,
  "summary": {
    "parse_ok": true,
    "validation_ok": true,
    "diagnostics_ok": true,
    "template_id": "udp_process_debug",
    "operation": "datagram_exchange",
    "finding_count": 0,
    "next_step": "binding is healthy; validate with runtime/demo input next",
    "focus": null,
    "parse_source_excerpt": null,
    "validation_excerpt": null,
    "diagnostics_excerpt": null
  },
  "focused_report": null,
  "frontend": { "...": "frontend report" },
  "binding": { "...": "binding report" },
  "validation": { "...": "validation report" },
  "diagnostics": { "...": "diagnostics report" },
  "findings": {
    "findings": []
  }
}
```

### `explain.summary`

- `parse_ok`: parse/front-end status
- `validation_ok`: registry/fragment coverage status
- `diagnostics_ok`: rule-support/diagnostics status
- `template_id`: compiled template id when available
- `operation`: compiled operation when available
- `finding_count`: total compiler findings
- `next_step`: human-oriented recommended next action
- `focus`: `null` unless `--focus` is used
- `parse_source_excerpt`: optional parse failure excerpt
- `validation_excerpt`: optional validation failure excerpt
- `diagnostics_excerpt`: optional diagnostics failure excerpt

### `parse_source_excerpt`

Shape:

```json
{
  "line": 3,
  "column": 9,
  "line_text": "  let broken =",
  "marker": "        ^"
}
```

Used when parse/front-end compilation fails and `gewyc` can point at a concrete
source line.

### `validation_excerpt`

Shape:

```json
{
  "model": "broken_offsets_model",
  "rule_index": 0,
  "unsupported_payload_offsets": [8, 9],
  "supporting_fragments": ["udp_packet_meta_fragment"]
}
```

Used when payload coverage validation fails and `gewyc` can point at the first
failing model/rule.

### `diagnostics_excerpt`

Shape:

```json
{
  "model": "broken_rule_model",
  "rule_index": 0,
  "missing_facts": ["PacketMeta"],
  "unsupported_payload_offsets": [],
  "supporting_fragments": ["sock_lineage_fragment"]
}
```

Used when diagnostics/rule-support fails and `gewyc` can point at the first
unsupported rule-sized unit.

## `explain --focus ... --json`

Command:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation --json
```

`summary` remains present. `focused_report` becomes a narrowed surface-specific
object. The broader reports still remain at top level so tooling can keep a
single code path if it wants.

Supported focus values:

- `parse`
- `frontend`
- `binding`
- `ir`
- `validation`
- `diagnostics`
- `findings`

Example validation focus:

```json
{
  "summary": {
    "focus": "validation"
  },
  "focused_report": {
    "kind": "validation",
    "report": {
      "ok": false,
      "registry": "builtin",
      "unsupported_payload_offsets": [8, 9]
    },
    "validation_excerpt": {
      "model": "broken_offsets_model",
      "rule_index": 0,
      "unsupported_payload_offsets": [8, 9],
      "supporting_fragments": ["udp_packet_meta_fragment"]
    }
  }
}
```

### IR focus

Command:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy --focus ir --json
```

Shape:

```json
{
  "summary": {
    "focus": "ir"
  },
  "focused_report": {
    "kind": "ir",
    "report": {
      "program_models": [
        {
          "id": "amqp_basic_publish_model",
          "kind": "program_model",
          "operation": "amqp_basic_publish",
          "rules": [
            {
              "rule_index": 0,
              "predicate": "packet_observed(l4_proto=6,dir=egress,local_port=none,remote_port=5672,payload_offsets=[10])",
              "signal": "FlowConditionObserved",
              "narrative": "transport_payload_sent",
              "dedupe": true,
              "module": "amqp_publish_sequence",
              "phase": "send_publish",
              "phase_kind": "emit_payload",
              "required_facts": ["PacketMeta"],
              "supporting_fragments": ["tcp_packet_meta_fragment"],
              "missing_facts": [],
              "unsupported_payload_offsets": [],
              "supported": true
            }
          ]
        }
      ],
      "reason_models": [
        {
          "id": "amqp_basic_publish_path_reason",
          "kind": "declarative_reason_model",
          "rules": []
        }
      ]
    }
  }
}
```

`ir` is the best fit when you want a stable, lowered view for:

- protocol authoring and review
- IR evolution work
- debugging `module` / `phase` / `phase_kind`
- checking rule support and reason-model provenance

The focused IR report now also carries:

- `ir_lowering_delta`
  A compact compare view between the front-end module graph and the lowered IR.
  It includes front-end counts plus lowered rule counts, support counts,
  modules, phases, and phase kinds.
- `ir_shape_note`
  A short human-oriented summary of the most important drift pattern.

## Consumer Patterns

### Shell / `jq`: grab the first parse excerpt

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus parse --json \
  | jq '.summary.parse_source_excerpt // .focused_report.parse_source_excerpt'
```

This is a good fit for:

- editor task runners
- pre-commit DSL validation hooks
- tiny shell wrappers that only need `line/column + caret`

### Shell / `jq`: grab the first validation coverage issue

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation --json \
  | jq '.focused_report.validation_excerpt'
```

This is a good fit for tooling that wants:

- the first failing model/rule
- unsupported payload offsets
- the supporting fragments already present

### Editor / IDE quick-inspect pattern

For a lightweight editor integration, a practical flow is:

1. Run `gewyc explain <path.gewy> --focus parse --json`
2. If `summary.parse_ok == false`, read `summary.parse_source_excerpt`
3. Otherwise run `gewyc explain <path.gewy> --focus validation --json`
4. If `summary.validation_ok == false`, read `focused_report.validation_excerpt`
5. Otherwise run `gewyc explain <path.gewy> --focus diagnostics --json`
6. If `summary.diagnostics_ok == false`, read `focused_report.diagnostics_excerpt`

That sequence keeps the UI small and progressive:

- parse gets source-local feedback first
- validation gets payload-coverage feedback second
- diagnostics gets rule-support feedback last

## Surface Selection

- Use `frontend --json` when you want function/include/graph structure.
- Use `frontend --focus graph --json` when you only care about graph shape.
- Use `explain --json` when you want one human-oriented compiler summary.
- Use `explain --focus parse --json` when you are building editor diagnostics.
- Use `explain --focus binding --json` when you want the compact compiled shape.
- Use `explain --focus ir --json` when you want lowered rule/program/reason detail.
- Use `explain --focus validation --json` when you are building coverage/debug tooling.
- Use `explain --focus diagnostics --json` when you want the first unsupported rule-sized entry point.

## Companion References

- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
  Stable language surface and current preferred subset.
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
  Exact package/module lookup rules.
- [docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf)
  Draft formal grammar.
- [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
  Source-layering note for contributors changing compiler internals.
