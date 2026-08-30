# GewyLang Language And IR Contract v1

This document is the normative stage and version contract for GewyLang.
Grammar details remain in [the canonical EBNF](gewylang.ebnf), while this page
defines the boundaries between syntax, compiler representations, and runtime
projections.

The machine-readable contract stamp is defined by
[`src/dsl/contract.rs`](../src/dsl/contract.rs) and described by
[`gewylang-language-contract-v1.schema.json`](contracts/gewylang-language-contract-v1.schema.json).

## Contract Identity

Every public compiler-stage payload carries this shape:

```json
{
  "language": "gewylang",
  "syntax_version": 1,
  "stage": "analysis_ir",
  "stage_version": 1
}
```

The fields have independent jobs:

- `language` prevents a generic consumer from confusing GewyLang with another
  language or IR family.
- `syntax_version` identifies the accepted canonical source-language contract.
- `stage` identifies the semantic compiler boundary represented by the payload.
- `stage_version` identifies the contract for that specific boundary.

Consumers must match all four fields before interpreting stage-specific data.

## Canonical Stages

| Stage | Version | Meaning | Primary surface |
| --- | --- | --- | --- |
| Source syntax | `1` | Canonical `.gewy` grammar and source semantics. | [`gewylang.ebnf`](gewylang.ebnf) |
| `expanded_ast` | `1` | Expanded package composition, declarations, provenance, and `use` graph. | `gewyc frontend` |
| `binding_ir` | `1` | Executable semantic compile target represented in Rust by `TemplateBinding`. | `gewyc binding` |
| `analysis_ir` | `1` | Diagnostics-enriched program and reason model projection represented by `IrReport`. | `gewyc ir` |

The stage names are protocol identifiers. Human-facing prose may use
"Expanded AST", "Binding IR", and "Analysis IR", but serialized output must
use the exact lowercase identifiers in the table.

## Pipeline

```text
GewyLang Syntax v1
  -> private parser representation
  -> Expanded AST v1
  -> Binding IR v1
  -> validation and supportability analysis
  -> Analysis IR v1
  -> runtime planning and export projections
```

The private parser structs, canonical assignment units, and helper types are
implementation details. They may change without a stage-version bump when the
observable syntax and stage contracts remain unchanged.

## Source Syntax v1

Syntax v1 is the source contract defined by:

- [canonical EBNF](gewylang.ebnf)
- [syntax guide](dsl-syntax.md)
- [canonical style](gewylang-style.md)
- [vocabulary reference](dsl-reference.md)

Source files do not require a version pragma. The compiler assigns the current
syntax contract during parsing and reports it on later stage surfaces.

Accepted migration-only spellings are not part of Syntax v1. They may be read
for compatibility, but generators and maintained repository sources must emit
the canonical grammar.

## Source Graph Safety Contract

Binding compilation and Expanded AST inspection use the same source-graph
loader. The loader applies these limits before lowering:

- each source is a regular file containing at most `262144` bytes
- one compilation consumes at most `256` source files, including the entry
- filesystem includes nest at most `32` levels below the entry
- the entry and every included source consume at most `4194304` bytes together

Reads are bounded by actual content rather than trusting an earlier metadata
length. Entry-only `include "..."` compatibility files and canonical pipeline
`include(...)` steps share the same cycle detection, path confinement, and
budgets. Local paths resolve from the containing source while remaining inside
the package root; dependency paths remain inside their declared dependency
root. Entry, alias, and included module sources also share the same
layout-preserving comment normalization; comment-free sources stay borrowed.

These are fail-closed resource and path rules for Syntax v1, not new syntax or
new IR fields. Changing the private loader without changing these observable
limits does not require an IR stage-version bump.

## Expanded AST v1

Expanded AST is the first public compiler projection. It records the package
after include resolution and function expansion while retaining authoring
provenance and graph structure.

Inspect it with:

```bash
cargo run -p gewyc -- frontend dsl/http_request_path.gewy --json
```

`FrontendReport` is a report model for this stage. The underlying Rust parser
AST is not a stable public ABI and must not be serialized or consumed directly
by external tooling.

## Binding IR v1

Binding IR is the executable semantic compile target. `TemplateBinding` owns
the effective template, fragments, window, parameters, evidence overrides,
program model, and reason profile after source composition has been resolved.

Inspect it with either form:

```bash
cargo run -p gewyc -- binding dsl/http_request_path.gewy --json
cargo run -p gewyc -- dsl/http_request_path.gewy --emit binding --json
```

Binding IR is not eBPF bytecode. It is the stable semantic input to validation,
diagnostics, runtime planning, and later materialization.

## Analysis IR v1

Analysis IR projects Binding IR into explicit program and reason models and
adds supportability facts from diagnostics. It answers what behavior was
materialized and whether the selected fragments can support each rule.

Inspect it directly with either form:

```bash
cargo run -p gewyc -- ir dsl/http_request_path.gewy --json
cargo run -p gewyc -- dsl/http_request_path.gewy --emit ir --json
```

`gewyc explain --focus ir` remains the troubleshooting view that embeds
Analysis IR beside frontend-to-lowering deltas and explanatory notes. It is not
a separate IR stage.

The compact `gewyc_ir_snapshot` output is an archival projection of Analysis
IR v1. It carries the same `analysis_ir` stamp and intentionally omits full
rule detail; it is not a fourth compiler stage.

## Runtime Projections Are Separate

Runtime `attach_plan`, materialized eBPF state, and export `protocol_ir` are
downstream runtime or export projections. They are not GewyLang compiler IR
stages and must not use an `expanded_ast`, `binding_ir`, or `analysis_ir` stamp
unless they are embedding one of those payloads unchanged.

In particular, export `protocol_ir` classifies the protocol-facing result of a
lowered flow. Its name does not make it a replacement for Binding IR or
Analysis IR.

## Versioning Rules

Increase `syntax_version` when a change alters canonical accepted grammar or
the meaning of valid source in a way that requires consumers or generators to
adapt.

Increase only the affected `stage_version` when a change alters that stage's
required fields, invariants, or semantic meaning while source syntax remains
compatible.

Do not increase either version for:

- internal Rust type or module refactoring
- parser or lowering performance work
- additive presentation notes outside the stage contract
- a renderer change that preserves the same machine fields and meanings

Additive machine fields may retain the current stage version only when old
consumers can ignore them safely and all existing required meanings remain
unchanged. Removing, renaming, or reinterpreting a required field needs a stage
version bump.

## Compatibility Rule

A consumer should fail closed on an unknown `language`, `syntax_version`,
`stage`, or `stage_version` unless it has an explicit compatibility adapter.
It may ignore unknown additive fields after the contract stamp has been
accepted.

The outer `gewyc` `schema_hint.schema_version` versions the renderer envelope.
It does not replace the language contract: envelope schema and language-stage
semantics evolve independently.
