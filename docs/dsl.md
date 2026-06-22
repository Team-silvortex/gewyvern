# DSL Guide

Use this page when you need the stable map of `.gewy` itself.

This page is now the entry shelf for the language, not the place where every
syntax rule and compatibility detail is inlined.

Read this page when the question is:

- what does `gewylang` compile to?
- what is the current preferred authoring shape?
- which companion pages should I read next?

Use the companion shelves when the question becomes more exact:

- syntax and authoring shape:
  [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
- exact key surface, predicates, and parameter/reference lookup:
  [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
- package layout and `include(...)` / `use(...)` rules:
  [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
- compiler JSON and machine-facing report shapes:
  [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
- lowering contract and IR-facing explanation:
  [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  and
  [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)

If you want the reading order for the whole language shelf, start with
[docs/gewylang-system.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-system.md).

## Goal

The DSL does not compile into eBPF bytecode.

Its compile target is `TemplateBinding`, which carries:

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
template(:structured_udp_process_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:structured_udp_process_debug_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :structured_udp_process_debug, phase: :bind)
```

The pipeline parser first merges files and function units into a single
pipeline/front-end IR, then lowers that IR into the current compiler surface.

Function units support both `${name}` and shorthand `$name` placeholders, so
parameterized pipelines can stay concise without changing their lowering model.

## Durable Source Shelves

`gewyvern` DSL files use the `.gewy` extension.

The repository has two durable source shelves for language usage:

- [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
  Canonical registry packages and runtime-facing package entries.
- [dsl](/Users/Shared/chroot/dev/gewyvern/dsl)
  Underlying protocol-path source files and compiler/debug baselines.

Anchor examples:

- debug/compiler baselines:
  [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy),
  [dsl/pipeline_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy),
  [dsl/structured_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy)
- transport and proxy paths:
  [dsl/tls_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy),
  [dsl/quic_stream_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy),
  [dsl/http3_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy),
  [dsl/hy2_tcp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy)
- stateful request/auth/session paths:
  [dsl/http_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy),
  [dsl/postgres_query_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_session.gewy),
  [dsl/mysql_query_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy),
  [dsl/redis_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/redis_session_path.gewy),
  [dsl/mqtt_publish_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_publish_path.gewy),
  [dsl/sip_invite_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/sip_invite_path.gewy),
  [dsl/ldap_directory_sync_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy)

## Stable Subset

The current recommended stable subset is:

- one package entry file with exactly one `template(...)` head
- one pipeline call per line
- pure function units declared with `fn ... =`, `fn ... =>`, or block form
- positional and positional-then-named `use(...)` application
- trailing default parameters for function units
- local immutable `let` bindings inside function units
- `include(...)` for package/file composition
- keyword-style `program_rule(...)` and `reason_rule(...)`

This is the best target if you want DSLs that are likely to remain stable
through the current hardening path.

## Frontend And Explain Surfaces

If you only want the package/front-end shape, use `gewyc frontend`.

Typical examples:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus expansion
```

If you want one human-oriented debugging surface above parse/front-end,
binding, IR, validation, diagnostics, and findings, use `gewyc explain`.

Typical examples:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus ir
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation
```

Use these pages for the exact companion contract:

- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Reading Paths

### First-Time Package Author

Read in this order:

1. [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
2. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
3. [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
4. [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)

### Compiler-Oriented Contributor

Read in this order:

1. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
2. [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
3. [docs/gewylang-evolution.md](/Users/Shared/chroot/dev/gewyvern/docs/gewylang-evolution.md)
4. [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
5. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
6. [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)

### Safety-Oriented Reviewer

Read in this order:

1. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
2. [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
3. [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
4. [docs/book/explanation-gewylang-lightweight-types.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-lightweight-types.md)

## Companion Shelves

Use these as peers rather than replacements:

- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
  for pipeline shape, package shape, idioms, and EBNF
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
  for legacy key surface, predicates, stages, narratives, and fragment
  parameter schema
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
  for exact `include(...)` / `use(...)` lookup rules
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  for the compiler's lowered contract candidate

## Implementation Anchors

If you are changing the language or debugging compiler behavior, these are the
most relevant implementation shelves:

- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
- [src/dsl/pipeline.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl/pipeline.rs)
- [src/dsl/predicate.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl/predicate.rs)
- [src/dsl/package.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl/package.rs)
- [src/dsl/frontend.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl/frontend.rs)
- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs)
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs)
- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [src/gewyc/frontend.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/frontend.rs)
- [src/gewyc/explain.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc/explain.rs)
- [tests/dsl_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/dsl_tdd.rs)
