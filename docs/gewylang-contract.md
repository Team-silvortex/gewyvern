# GewyLang Language And IR Contract v1

This document is the normative stage and version contract for GewyLang.
Grammar details remain in [the canonical EBNF](gewylang.ebnf), while this page
defines the boundaries between syntax, compiler representations, and runtime
projections.

The machine-readable contract stamp and source bounds are owned by the
zero-product-dependency
[`gewylang-contract`](../crates/gewylang-contract) crate and described by
[`gewylang-language-contract-v1.schema.json`](contracts/gewylang-language-contract-v1.schema.json).
`gewyvern::dsl` re-exports the same Rust items for source compatibility, but
external generators and inspection tools can consume the contract without
linking the Gewyvern runtime.

The product-independent [`gewylang-syntax`](../crates/gewylang-syntax) crate
consumes that contract and owns bounded source loading, package/include graph
expansion, the canonical syntax AST and parser, and frontend summaries. Its only
normal dependency is `gewylang-contract`, so editors, generators, and static
inspection tools can parse GewyLang without linking Gewyvern. The
product-independent [`gewylang-compiler`](../crates/gewylang-compiler) crate
then owns function expansion, parameter substitution, canonical rule lowering,
and direct string/file compiler facades. Its `SemanticHost` trait is the only
place where product-specific values enter lowering; its `BindingMaterializer`
trait is the only completion boundary from canonical assignments into a host
binding. The `gewyvern::dsl` facade preserves the established API and error
shape while one explicit host implements both protocols and produces
`TemplateBinding`.
The product-independent [`gewylang-ir`](../crates/gewylang-ir) crate owns the
stable Binding IR and Analysis IR report values, diagnostics projections, model
comparison, history snapshots, deterministic stage fingerprints, and the
`CompilerProjectionHost` protocol. Its product-independent workspace dependency
closure contains only `gewylang-contract`; serialization and SHA-256 libraries
are external implementation details. It coordinates all three stable report
projections and preserves host diagnostic failures. Gewyvern remains the
concrete adapter that maps runtime bindings and registry diagnostics into those
values.

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
| `expanded_ast` | `1` | Expanded package composition, declarations, provenance, and `use` graph. | `gewylang-syntax` |
| `binding_ir` | `1` | Stable materialized-binding projection represented by `gewylang_ir::BindingReport`; execution uses Gewyvern's `TemplateBinding` adapter. | `gewylang-ir` / `gewyc binding` |
| `analysis_ir` | `1` | Diagnostics-enriched program and reason model projection represented by `gewylang_ir::IrReport`. | `gewylang-ir` / Gewyvern analysis adapter / `gewyc ir` |

The stage names are protocol identifiers. Human-facing prose may use
"Expanded AST", "Binding IR", and "Analysis IR", but serialized output must
use the exact lowercase identifiers in the table.

## Pipeline

```text
GewyLang Syntax v1
  -> gewylang-syntax
  -> Expanded AST v1
  -> gewylang-compiler + explicit SemanticHost
  -> canonical assignments
  -> gewylang-compiler + explicit BindingMaterializer
  -> Gewyvern binding adapter
  -> Binding IR v1
  -> validation and supportability analysis
  -> gewylang-ir + explicit CompilerProjectionHost
  -> Analysis IR v1
  -> runtime planning and export projections
```

The private parser structs, canonical assignment units, and Gewyvern host
implementations are implementation details. The public host traits may evolve
compatibly without a stage-version bump when the observable syntax and stage
contracts remain unchanged. Public `gewylang-ir` values are stage-contract
types and must follow the corresponding stage version.

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
- `gewy.pkg` is a regular file containing at most `65536` bytes, with no
  duplicate identity, entry, source-alias, or dependency-alias fields
- one compilation consumes at most `256` source files, including the entry
- filesystem includes nest at most `32` levels below the entry
- the entry and every included source consume at most `4194304` bytes together

Reads are bounded by actual content rather than trusting an earlier metadata
length. Manifest symbolic links are rejected, and Unix source opens use
no-follow semantics on the resolved file to close the final-component
metadata/open race. Entry-only `include "..."` compatibility files and canonical pipeline
`include(...)` steps share the same cycle detection, path confinement, and
budgets. Local paths resolve from the containing source while remaining inside
the package root; dependency paths remain inside their declared dependency
root. Entry, alias, and included module sources also share the same
layout-preserving comment normalization; comment-free sources stay borrowed.

These are fail-closed resource and path rules for Syntax v1, not new syntax or
new IR fields. Changing the private loader without changing these observable
limits does not require an IR stage-version bump.

Semantic expansion has an independent budget: one compilation may consume at
most `16384` expanded calls, function use may nest at most `64` levels, and one
value after placeholder substitution may contain at most `262144` bytes. These
limits apply to acyclic graphs too, preventing repeated function use or local
bindings from producing exponential CPU or memory growth.

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

## Canonical IR Fingerprints

`gewylang-ir` provides deterministic fingerprints for `BindingReport`,
`IrReport`, and each `IrModelReport`. The text representation is:

```text
sha256:v1:<64 lowercase hexadecimal characters>
```

The corresponding `gewyc` JSON field is an object with `algorithm`,
`encoding_version`, and `digest`. Binding and direct Analysis IR outputs use
`payload.fingerprint` and covers the complete public `BindingReport`
projection. An archival history snapshot uses
`payload.source_ir_fingerprint`, because that value covers the complete source
Analysis IR rather than only the compact snapshot fields.

Fingerprint encoding v1 is domain-separated by language id, syntax version,
stage id, stage version, and projection kind. It uses explicit variant and
optional-value markers, fixed-width integers, and length-prefixed UTF-8 fields.
It covers ordered Binding IR values and every ordered Analysis IR rule field,
including supportability data. This makes same-shape changes to predicates,
narratives, dedupe behavior, or evidence support visible to release and cache
tooling.

Fingerprints identify exact IR content. They are not signatures and do not
prove who produced an artifact. Authenticity still belongs to signed release
provenance. A change to the canonical fingerprint encoding increments its
`encoding_version`; it does not by itself change Syntax v1 or an IR stage
version. The crate pins encoding v1 with golden vectors so implementation drift
fails tests instead of silently changing artifact identities.

## Structural IR Invariants

`gewylang-ir` validates complete values without consulting the Gewyvern
runtime or fragment registry. Invariant contract v1 rejects malformed identity
fields, duplicate set-like values, fragment references outside the declared
fragment set, unknown evidence/diagnostic tiers, invalid model-kind and
operation shapes, non-contiguous rule indices, incomplete phase-kind context,
missing facts that were not required, and support flags that disagree with
missing facts or unsupported payload offsets.

`CompilerStageProjections::validate_invariants` additionally checks that
Binding IR, available diagnostics, and Analysis IR agree on template identity,
fragment order, model presence, model identity, operation, rule count, and
rule-level supportability fields. A host diagnostics error remains a host
error; the validator does not invent diagnostics or mistake an unavailable
projection for malformed IR.

`validate_binding_analysis_ir` applies the same structural and cross-stage
checks to an independently loaded Binding/Analysis pair without requiring a
diagnostics host. Snapshot consumers must use this boundary before treating two
stage documents as one compile.

Every violation carries a stable `GEWYLANG-IR-*` code, a field path, and a
detail capped at `512` UTF-8 bytes. Validation accumulates at most `256`
deterministic violations and marks a report when more were omitted.
`project_compiler_stages_checked` converts that report into
`IrValidationErrors` and releases no projections when any invariant fails.
`gewyc` uses this checked entry point and reports projection drift as a
validation finding rather than panicking, normalizing the value, or exposing
an inconsistent partial IR set.

## Standalone IR Wire v1

The independent crate also owns a strict JSON exchange envelope for complete
Binding IR and Analysis IR values. Its normative schema is
[`gewylang-ir-wire-v1.schema.json`](contracts/gewylang-ir-wire-v1.schema.json).
This codec is deliberately separate from the presentation-oriented outer
`gewyc` JSON envelope. The following shape abbreviates the stage-specific
payload and is not itself a complete decodable document.

```json
{
  "wire_format": "gewylang-ir-json",
  "wire_version": 1,
  "language_contract": {
    "language": "gewylang",
    "syntax_version": 1,
    "stage": "analysis_ir",
    "stage_version": 1
  },
  "fingerprint": {
    "algorithm": "sha256",
    "encoding_version": 1,
    "digest": "<64 lowercase hexadecimal characters>"
  },
  "payload": {}
}
```

Use `encode_binding_ir_json` / `decode_binding_ir_json` or
`encode_analysis_ir_json` / `decode_analysis_ir_json`. Encoders first validate
the source value and emit deterministic compact JSON. Decoders fail closed in
this order:

1. Reject documents larger than `16777216` bytes before JSON parsing.
2. Reject malformed JSON, unknown fields, duplicate fields, missing fields,
   and incompatible scalar types.
3. Match wire format/version and all four language-contract fields exactly.
4. Match fingerprint algorithm/encoding and recompute the complete payload
   digest.
5. Apply the stage's structural invariants to the fingerprint-verified content.

The size budget limits untrusted decoder allocation; it is independent of the
source graph budget. Wire v1 preserves all ordered IR fields and does not
silently fill, reorder, or normalize values. A Binding envelope cannot be read
as Analysis IR. The embedded digest detects accidental or unkeyed content
drift but remains a fingerprint, not an authenticity signature. Wire error
details are capped at `512` UTF-8 bytes so hostile values cannot become
unbounded log records.

## Typed IR Diff v1

Diff contract v1 turns exact fingerprint inequality into deterministic typed
changes. `diff_binding_ir` and `diff_analysis_ir` compare in-memory stage
values; `diff_binding_ir_json` and `diff_analysis_ir_json` first apply the full
strict wire decoder, including contract matching, fingerprint recomputation,
and structural validation. `diff_compiler_ir` accepts two Binding/Analysis
pairs and rejects either side unless `validate_binding_analysis_ir` proves that
the pair is coherent.

Every retained `IrChange` carries a stage, fixed field path, change kind
(`added`, `removed`, `modified`, or `reordered`), semantic impact, and bounded
before/after previews. The report derives one maximum-severity compatibility
class:

| Class | Meaning |
|---|---|
| `identical` | Every compared field is equal; both stage fingerprints match. |
| `analysis_only` | Only narrative, reason-model, evidence, or supportability information changed; executable content is unchanged. |
| `execution_change` | Stable identities remain comparable, but executable configuration or program-rule semantics changed. |
| `incompatible` | Template/model identity or a top-level optional/variant shape changed; consumers must not correlate or reuse the snapshots automatically. |

Binding fragments, window values, program operation/rule count, and fragment
parameters are execution-impacting. Evidence overrides and declarative reason
rule counts are analysis-impacting. In Analysis IR, program predicates,
signals, dedupe, module, and phase fields are execution-impacting; narratives
and all supportability fields are analysis-impacting. Reason-model rule content
is analysis-impacting, while model identity and kind remain incompatible.

Each stage retains at most `256` changes and each value preview at most `256`
UTF-8 bytes. Preview serialization stops at the limit rather than allocating a
complete untrusted value first. Classification still examines every field
after the retained list fills, so truncation cannot hide a later, more severe
change. A future field missed by the v1 visitor falls back to `incompatible`
instead of being reported as identical.

Diff classification is payload policy, not provenance, authorization, or
semantic-version negotiation. It does not alter Syntax v1, Binding IR v1,
Analysis IR v1, fingerprint encoding v1, or wire JSON v1 bytes.

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
