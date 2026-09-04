# DSL Guide

Use this page when you need the stable map of `.gewy` itself.

If you are providing context to a language model or want the shortest precise
generation contract, start with
[GewyLang Guide For Humans And Language Models](gewylang-llm-guide.md).
The normative repository source style is
[GewyLang Canonical Style Standard](gewylang-style.md).

This page is now the entry shelf for the language, not the place where every
syntax rule and compatibility detail is inlined.

Read this page when the question is:

- what does `gewylang` compile to?
- what is the current preferred authoring shape?
- which companion pages should I read next?

Use the companion shelves when the question becomes more exact:

- syntax and authoring shape:
  [docs/dsl-syntax.md](dsl-syntax.md)
- exact key surface, predicates, and parameter/reference lookup:
  [docs/dsl-reference.md](dsl-reference.md)
- package layout and `include(...)` / `use(...)` rules:
  [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)
- compiler JSON and machine-facing report shapes:
  [docs/gewyc-json.md](gewyc-json.md)
- lowering contract and IR-facing explanation:
  [docs/gewylang-contract.md](gewylang-contract.md),
  [docs/book/reference-ir-lowering.md](book/reference-ir-lowering.md)
  and
  [docs/book/explanation-gewylang-to-ir.md](book/explanation-gewylang-to-ir.md)

If you want the reading order for the whole language shelf, start with
[the GewyLang module](modules/gewylang.md).

## Goal

The DSL does not compile into eBPF bytecode.

Its semantic compile target is Binding IR v1, represented by
the public `gewylang_ir::BindingReport` contract and materialized for execution
as `TemplateBinding`, which carries:

- template identity
- fragment selection
- window profile
- reason profile
- program model
- fragment parameter bindings
- evidence tier overrides

That boundary is intentional. The DSL selects and parameterizes existing
fragment templates. It is not a general-purpose kernel-program authoring
surface.

## Current Shape

`gewylang` uses a pipeline-driven surface inspired by Elixir. The maintained
subset in this repository is intentionally small:

- one package has one main entry file
- reusable behavior is expressed as pure function units
- included files are merged into the package compile path
- the final artifact is the entry file's merged binding surface
- safety checks focus on narrow, high-value parameter boundaries

Comments start with `#`.

Example:

```text
template :structured_udp_process_debug
|> window :default_5s
|> reason :udp_datagram_l1
|> fragment :udp_packet_meta_fragment
|> fragment :route_meta_fragment
|> fragment :sock_lineage_fragment
|> operation :datagram_exchange
|> program_model :structured_udp_process_debug_model
|> program_rule pred: :process_bound, stage: :process_bound, narr: :process_bound, dedupe: true, mod: :structured_udp_process_debug, phase: :bind
```

The pipeline parser first merges files and function units into Expanded AST
v1, then lowers that structure into Binding IR v1. Analysis IR v1 is the
separate diagnostics-enriched inspection projection. Both public value shapes
live in the product-independent `gewylang-ir` crate; runtime materialization and
registry-aware production remain explicit Gewyvern adapters. The compiler calls
those adapters only through `SemanticHost` and `BindingMaterializer`; source
parsing, canonical lowering, and end-to-end host dispatch stay in the
product-independent `gewylang-compiler` crate. Binding, Diagnostics, and
Analysis report production crosses back through
`gewylang_ir::CompilerProjectionHost`, with coherent stage orchestration owned
by `gewylang-ir` and only field mapping owned by Gewyvern.

Function units reference parameters and local bindings with `$name`, so
parameterized pipelines stay concise without changing their lowering model.
Single-argument pipeline calls use the parenless stable form, such as
`template :demo`, `|> include "./module.gewy"`, and `|> program_model :demo_model`.

## Durable Source Shelves

`gewyvern` DSL files use the `.gewy` extension.

The repository has two durable source shelves for language usage:

- [protocols](../protocols)
  Canonical registry packages and runtime-facing package entries.
- [dsl](../dsl)
  Underlying protocol-path source files and compiler/debug baselines.

Every protocol package entry is self-contained. When a package template has a
matching `dsl/<template_id>.gewy` baseline, the two source files must remain
byte-identical. Package-only entries follow the same canonical style.

Anchor examples:

- debug/compiler baselines:
  [dsl/handshake_debug.gewy](../dsl/handshake_debug.gewy),
  [dsl/pipeline_udp_process_debug.gewy](../dsl/pipeline_udp_process_debug.gewy),
  [dsl/structured_udp_process_debug.gewy](../dsl/structured_udp_process_debug.gewy)
- transport and proxy paths:
  [dsl/tls_client_path.gewy](../dsl/tls_client_path.gewy),
  [dsl/quic_stream_session_path.gewy](../dsl/quic_stream_session_path.gewy),
  [dsl/http3_request_path.gewy](../dsl/http3_request_path.gewy),
  [dsl/hy2_tcp_relay_path.gewy](../dsl/hy2_tcp_relay_path.gewy)
- stateful request/auth/session paths:
  [dsl/http_request_path.gewy](../dsl/http_request_path.gewy),
  [dsl/postgres_query_session.gewy](../dsl/postgres_query_session.gewy),
  [dsl/mysql_query_session.gewy](../dsl/mysql_query_session.gewy),
  [dsl/redis_session_path.gewy](../dsl/redis_session_path.gewy),
  [dsl/mqtt_publish_path.gewy](../dsl/mqtt_publish_path.gewy),
  [dsl/sip_invite_path.gewy](../dsl/sip_invite_path.gewy),
  [dsl/ldap_directory_sync_session.gewy](../dsl/ldap_directory_sync_session.gewy)

## Stable Subset

The current recommended stable subset is:

- one package entry file with exactly one `template ...` head
- one pipeline call per line
- pure function units declared with `fn ... =`
- positional and positional-then-named `use(...)` application
- trailing default parameters for function units
- local immutable `let` bindings inside function units
- `include(...)` for package/file composition
- named-field `program_rule` and `reason_rule` calls

This is the best target if you want DSLs that are likely to remain stable
through the current hardening path.

## Frontend And Explain Surfaces

If you only want the package/front-end shape, use `gewyc frontend`.

Typical examples:

```bash
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --focus expansion
```

If you want one human-oriented debugging surface above parse/front-end,
binding, IR, validation, diagnostics, and findings, use `gewyc explain`.

Typical examples:

```bash
cargo run -p gewyc -- explain dsl/udp_process_debug.gewy
cargo run -p gewyc -- explain dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- ir dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- explain dsl/udp_process_debug.gewy --focus ir
cargo run -p gewyc -- explain dsl/udp_process_debug.gewy --focus validation
```

Use these pages for the exact companion contract:

- [docs/dsl-syntax.md](dsl-syntax.md)
- [docs/dsl-reference.md](dsl-reference.md)
- [docs/gewylang-contract.md](gewylang-contract.md)
- [docs/gewyc-json.md](gewyc-json.md)
- [docs/book/reference-ir-lowering.md](book/reference-ir-lowering.md)

## Reading Paths

### First-Time Package Author

Read in this order:

1. [docs/book/tutorial-gewylang-package.md](book/tutorial-gewylang-package.md)
2. [docs/dsl.md](dsl.md)
3. [docs/dsl-syntax.md](dsl-syntax.md)
4. [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)

### Compiler-Oriented Contributor

Read in this order:

1. [docs/dsl.md](dsl.md)
2. [docs/dsl-reference.md](dsl-reference.md)
3. [docs/gewylang-evolution.md](gewylang-evolution.md)
4. [docs/book/explanation-gewylang-to-ir.md](book/explanation-gewylang-to-ir.md)
5. [docs/book/reference-ir-lowering.md](book/reference-ir-lowering.md)
6. [docs/gewyc-json.md](gewyc-json.md)

### Safety-Oriented Reviewer

Read in this order:

1. [docs/dsl.md](dsl.md)
2. [docs/dsl-reference.md](dsl-reference.md)
3. [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)
4. [docs/book/explanation-gewylang-lightweight-types.md](book/explanation-gewylang-lightweight-types.md)

## Companion Shelves

Use these as peers rather than replacements:

- [docs/dsl-syntax.md](dsl-syntax.md)
  for pipeline shape, package shape, idioms, and EBNF
- [docs/dsl-reference.md](dsl-reference.md)
  for exact key surface, predicates, stages, narratives, and fragment
  parameter schema
- [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)
  for exact `include(...)` / `use(...)` lookup rules
- [docs/book/reference-ir-lowering.md](book/reference-ir-lowering.md)
  for the compiler's lowered contract candidate

## Implementation Anchors

If you are changing the language or debugging compiler behavior, these are the
most relevant implementation shelves:

- [src/dsl.rs](../src/dsl.rs)
- [src/dsl/pipeline.rs](../src/dsl/pipeline.rs)
- [src/dsl/semantic_host.rs](../src/dsl/semantic_host.rs)
- [src/dsl/materializer.rs](../src/dsl/materializer.rs)
- [src/dsl/semantic_values.rs](../src/dsl/semantic_values.rs)
- [src/gewyc/projection_host.rs](../src/gewyc/projection_host.rs)
- [crates/gewylang-compiler/src/lib.rs](../crates/gewylang-compiler/src/lib.rs)
- [crates/gewylang-compiler/src/lowering.rs](../crates/gewylang-compiler/src/lowering.rs)
- [crates/gewylang-ir/src/lib.rs](../crates/gewylang-ir/src/lib.rs)
- [crates/gewylang-ir/src/binding.rs](../crates/gewylang-ir/src/binding.rs)
- [crates/gewylang-ir/src/analysis.rs](../crates/gewylang-ir/src/analysis.rs)
- [crates/gewylang-ir/src/projection.rs](../crates/gewylang-ir/src/projection.rs)
- [crates/gewylang-syntax/src/lib.rs](../crates/gewylang-syntax/src/lib.rs)
- [crates/gewylang-syntax/src/package.rs](../crates/gewylang-syntax/src/package.rs)
- [crates/gewylang-syntax/src/frontend.rs](../crates/gewylang-syntax/src/frontend.rs)
- [src/dsl/predicate.rs](../src/dsl/predicate.rs)
- [src/template.rs](../src/template.rs)
- [src/program.rs](../src/program.rs)
- [src/fragment.rs](../src/fragment.rs)
- [src/gewyc/frontend.rs](../src/gewyc/frontend.rs)
- [src/gewyc/explain.rs](../src/gewyc/explain.rs)
- [tests/dsl_tdd.rs](../tests/dsl_tdd.rs)
