# DSL Guide

Use this page when you need the current stable shape of the `.gewy` language
itself.

This page is intentionally a durable language guide. It describes:

- what `.gewy` compiles to
- what the current pipeline/package surface looks like
- which constructs are considered stable
- which parameter boundaries are intentionally enforced

This page is not the best first stop for:

- your first end-to-end run
- your first package walkthrough
- exact package/module lookup
- compiler JSON surface details

For those, use:

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)

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

That boundary is intentional. The DSL is for selecting and parameterizing
existing fragment templates, not for generating arbitrary kernel programs.

## File Extension And Source Shelves

`gewyvern` DSL files use the `.gewy` extension.

The repository has two durable source shelves for current language usage:

- [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
  Canonical registry packages and runtime-facing package entries.
- [dsl](/Users/Shared/chroot/dev/gewyvern/dsl)
  Underlying protocol-path source files and compiler/debug baselines.

If you want a few anchor examples instead of browsing the whole tree, these are
good starting points:

- debug/compiler baselines:
  [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy),
  [dsl/pipeline_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy),
  [dsl/structured_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy)
- transport and proxy paths:
  [dsl/tls_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy),
  [dsl/quic_stream_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy),
  [dsl/http3_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy),
  [dsl/hy2_tcp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy)
- request/auth/session paths:
  [dsl/http_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy),
  [dsl/postgres_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy),
  [dsl/mysql_query_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy),
  [dsl/ldap_directory_sync_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy)
- infrastructure control protocols:
  [dsl/ssh_channel_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssh_channel_session_path.gewy),
  [dsl/socks5_auth_connect_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_connect_denied_path.gewy),
  [dsl/ftp_active_retr_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_retr_path.gewy),
  [dsl/smtp_data_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_denied_path.gewy),
  [dsl/imap_select_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/imap_select_path.gewy),
  [dsl/pop3_list_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pop3_list_path.gewy),
  [dsl/kerberos_tgs_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_tgs_path.gewy),
  [dsl/rtsp_setup_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_setup_path.gewy)

## Current Shape

`gewylang` now uses a single pipeline-driven surface inspired by Elixir. All
maintained protocol DSL files in this repository compile through that stable
subset.

The language direction is intentionally functional:

- one package has one main entry file
- included files do not carry global mutable state
- reusable behavior is expressed as pure function units
- the final compile target is the entry file's merged AST/binding, not
  independently executed modules

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
pipeline module IR, then lowers that IR into the current compiler surface. It
does not generate eBPF bytecode directly.

For QUIC-family protocols, `quic_frame_observed` now accepts
`frame:crypto`, `frame:ack`, `frame:stream`, `frame:datagram`, and
`frame:connection_close`. It also accepts `byte_at:<offset>:<mask>:<value>`
and `bytes_at:<offset>:<byte>,<byte>,...`, which lets DSLs express both
stream-oriented and datagram-oriented QUIC modules without falling back to raw
UDP payload offsets.

Pipeline projects can resolve through a `gewy.pkg` manifest with one
`main.gewy` entry and `include("...")` expansion. That merged front-end IR is
also reflected in compiler-facing reports, so `gewyc stages` can surface:

- function counts
- merged step counts
- resolved `include(...)` sources
- a minimal front-end graph for entry/file/function identities
- `include()` and `use()` edges, including the source line that produced them

The best companion pages for those surfaces are:

- [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- [docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md)
- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)

## Frontend And Explain Surfaces

If you only want the front-end shape without the full staged compiler report,
`gewyc frontend` renders that pipeline/package summary directly:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus expansion
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --compact
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit frontend --json --out /tmp/udp-process-frontend.json
```

The text form is intentionally optimized for human inspection. It prints
`include_sources`, `function_nodes`, `use_edges`, `graph_nodes`, and
`graph_edges` as separate multi-line sections instead of a single compact line.
When you only want one part, `--focus functions|includes|graph` keeps the top
summary and expands just that section.
When you just want a quick terminal scan, `--compact` keeps the same summary
surface but compresses it into a much shorter text form.

If you want one human-oriented debugging surface that narrates the compiler
state from parse/front-end through validation, diagnostics, and findings,
`gewyc explain` now sits above the lower-level report surfaces:

```bash
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus binding
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus ir
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --compact
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit explain --json --out /tmp/udp-process-explain.json
```

`explain` is intentionally advisory: it now adds a `next_step` hint so parse
failures steer you toward `gewyc frontend`, validation failures steer you
toward `unsupported_payload_offsets`, and healthy bindings steer you toward
runtime/demo verification.

For parse failures, `explain` also includes a lightweight `source_excerpt`
surface so you can see the failing source line and caret marker next to the
reported `line:column`.

For validation failures around payload coverage, `explain` now also includes a
lightweight `validation_excerpt` that points at the first failing model/rule
and its unsupported offsets, so the next debugging step is less guessy.
It now also adds a short `validation_note` that explains, in plain terms, why
the first failing rule is outside current fragment coverage.

For diagnostics/rule-support failures, `explain` now includes a matching
`diagnostics_excerpt` with the first unsupported rule, its missing facts or
unsupported offsets, and the fragments that are currently supporting it.
It also adds a short `diagnostics_note` that explains why that first rule still
is not supportable with the current fragment set.

That keeps `explain --focus diagnostics` useful even when the full diagnostics
report is large: you get one concrete rule-sized starting point instead of
having to scan the whole model first.

When you want to inspect the lowered declarative shape that sits between the
pipeline front-end and runtime-facing reasoning, `explain --focus ir` gives a
purpose-built IR view. It surfaces:

- program models and their operations
- lowered program rules with `module`, `phase`, and `phase_kind`
- rule support such as required facts, supporting fragments, missing facts, and
  unsupported payload offsets
- reason models as either `builtin_reason_profile` or
  `declarative_reason_model`
- a structured `ir_delta` that compares front-end graph shape against lowered
  rule counts, support counts, modules, phases, and phase kinds, while also
  summarizing the lowered `program_model` and `reason_model`

That makes `ir` the most direct protocol-authoring and IR-evolution view. It
is especially useful when you want to answer "what did this package really
lower into?" without reading the full binding, diagnostics, and findings
reports together.

For the exact lowering contract candidate behind that view, including the
current `program_model` / `reason_model` summary surface and
`ir_lowering_delta` expectations, see
[docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md).

When you only want one layer, `explain --focus parse|frontend|binding|ir|validation|diagnostics|findings`
keeps the top summary but expands just that section, and
`frontend --focus functions|includes|graph|expansion` does the same for the
pipeline front-end view. The `expansion` focus is the quickest way to answer
"what do entry and functions actually expand into?" without reading the full
graph first.

The `binding` focus is the matching lowered-side shortcut: it gives a very
light summary of what the pipeline actually compiled into, including fragment
count, window/reason/program-model presence, program rule count, fragment
params, and evidence overrides, before showing the full binding report. It now
also includes a very-light `frontend -> lowered` delta so you can compare
function/step/use/include counts against the final fragment/rule shape without
reading two full reports side by side. It also adds a short `binding_note`
explaining the most common reason the lowered shape looks larger or more
collapsed than the frontend shape.

The `ir` focus sits one layer closer to protocol authorship than `binding`.
`binding` is the best compact view of the whole compiled surface; `ir` is the
best focused view when you care about lowered rules, phases, support facts, and
reason-model provenance.
When you only want the high-level answer, `--compact` keeps `explain` readable
in a short terminal view without changing the JSON schema.

If you are wiring these surfaces into an editor, script, or lightweight IDE
tool, the small JSON shape guide lives in
[docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md).

When pipeline/package parsing fails, `gewyc findings` and `gewyc stages` now
surface more specific parse codes for front-end errors such as unknown
`use(:fn)` targets, unknown package dependencies, invalid function bodies,
unclosed function blocks, and `include(...)` calls that are not backed by a
filesystem package entry.

## Pipeline Shape

Top-level pipeline files start with:

```text
template(:template_id)
```

Then extend the binding with Elixir-style pipeline steps:

- `|> window(:default_5s)`
- `|> window(duration_ms: 5000, lateness_ms: 200)`
- `|> reason(:udp_datagram_l1)`
- `|> fragment(:udp_packet_meta_fragment)`
- `|> program_model(:example_model)`
- `|> reason_model(:example_reason)`
- `|> operation(:datagram_exchange)`
- `|> param(:sock_lineage_fragment.capture_comm, true)`
- `|> evidence(:sock_lineage, :core_requirement)`
- `|> program_rule(...)`
- `|> reason_rule(...)`
- `|> include("./module.gewy")`
- `|> use(:network_module)`
- `|> use(:network_module, :demo_app_model, :datagram_exchange)`

Current parser rule: one pipeline call per line.

Function units can be declared in two equivalent forms.

The more FP-like preferred form is an expression-style definition:

```text
fn network_module() =
  let module_name = :udp_module
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> operation(:datagram_exchange)
```

`=>` is accepted as an alias:

```text
fn network_module(model_name, op_name) =>
  let default_phase = :bind
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})
```

Tail parameters may also carry defaults:

```text
fn network_module(model_name, op_name = :datagram_exchange) =>
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})
```

That lets `use(:network_module, :demo_model)` override the first parameter while
still falling back to the default operation.

`use(...)` may also pass named arguments:

```text
fn network_module(model_name, op_name = :datagram_exchange) =>
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:demo_app)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:network_module, op_name: :stream_exchange, model_name: :demo_model)
```

The current rule is intentionally narrow:

- the first `use(...)` argument is still the function name
- positional arguments may come first
- named arguments may follow
- positional arguments may not appear after named arguments
- named arguments must match declared parameter names

Function parameters also carry a lightweight inferred kind surface. `gewylang`
does not implement a full global type system, but it does infer parameter
intent from how placeholders are used inside a function body.

Current inferred kinds are:

- `atom`
- `bool`
- `u64`
- `predicate`
- `narrative`

That inference is surfaced through `gewyc frontend`, `gewyc explain`, and the
JSON report types so function summaries can show each parameter's expected
role.

Example:

```text
fn udp_core(model_name, op_name = :datagram_exchange, dedupe_flag = true, duration_ms = 5000) =>
  |> window(duration_ms: ${duration_ms}, lateness_ms: 200)
  |> operation(${op_name})
  |> program_model(${model_name})
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: ${dedupe_flag}, module: :frontend_summary, phase: :bind)
```

In that function:

- `model_name` infers as `atom`
- `op_name` infers as `atom`
- `dedupe_flag` infers as `bool`
- `duration_ms` infers as `u64`

The current validation boundary is intentionally narrow:

- inferred `bool` parameters are validated at `use(...)` application time
- inferred `u64` parameters are validated at `use(...)` application time
- other inferred kinds are advisory/reporting-only for now

This keeps the language lightweight while still making reusable modules easier
to understand and safer to call.

The original block form is still supported for compatibility:

```text
fn network_module() {
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> operation(:datagram_exchange)
}
```

Block functions can also be parameterized:

```text
fn network_module(model_name, op_name) {
|> fragment(:udp_packet_meta_fragment)
|> operation(${op_name})
|> program_model(${model_name})
}
```

And then applied from the entry pipeline:

```text
template(:demo_app)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:network_module)
```

Current semantics:

- functions are pure DSL composition units
- local `let` bindings are immutable and scoped to one function unit
- `let` values may reference earlier parameters or earlier local bindings via
  `${name}` placeholders
- trailing function parameters may provide defaults, and omitted call-site
  arguments will fall back to those defaults
- expression-style functions consume the following `|>` lines until the next
  top-level declaration
- they may not define `template(...)`
- `include(...)` merges function definitions and steps into the single package
  entry compile path
- nested `use(:other_function)` composition is supported
- `use(:fn_name, ...)` supports positional arguments for parameterized function units
- `use(:fn_name, key: value, ...)` supports named arguments for parameterized function units
- there is no cross-file global variable state

## Stable Subset

The current recommended stable subset for `gewylang` is intentionally small:

- one package entry file with exactly one `template(...)` head
- pipeline steps with one call per line
- pure function units declared with either `fn ... =` or `fn ... { ... }`
- positional `use(:fn_name, ...)` function application
- positional-then-named `use(:fn_name, ..., key: value)` function application
- trailing default parameters for function units
- local immutable `let` bindings inside function units
- `include(...)` for file composition
- keyword-style `program_rule(...)` and `reason_rule(...)`

This subset is the best target if you want DSLs that are likely to stay stable
through the `0.9.x` to `1.0` hardening path.

Features that are still legal but should be thought of as transitional or
lower-preference surfaces:

- large hand-written inline entry pipelines without reusable function units

## Pipeline Idioms

Recommended `gewylang` style is intentionally small and regular:

- prefer expression-style `fn ... =` for short reusable modules
- use `let` for local names that improve readability, not for building deep
  mini-scope trees
- keep one conceptual action per `|>` line
- pass variability in through function parameters, then derive local aliases
  with `let`
- keep `template(...)` heads shallow and move reusable behavior into function
  units
- prefer `use(:module_name, ...)` composition over repeating the same fragment
  and rule bundle inline

Example:

```text
fn udp_client(model_name) =
  let module_name = :udp_client
  let op_name = :datagram_exchange
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: ${module_name}, phase: :bind)

template(:demo_app)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client, :demo_app_model)
```

Pipeline program rules use keyword arguments:

```text
|> program_rule(predicate: "datagram_observed:udp:local_to_remote", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: :example_module, phase: :send_request)
```

Pipeline reason rules use `key_event:` instead of `stage:`:

```text
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true, module: :example_module, phase: :bind)
```

Atoms like `:udp_datagram_l1` lower to plain DSL identifiers, while quoted
strings are kept for values that contain punctuation or spaces.

## Pipeline EBNF

The formalized grammar surface is the pipeline DSL.

The canonical draft grammar now also lives in
[docs/gewylang.ebnf](/Users/Shared/chroot/dev/gewyvern/docs/gewylang.ebnf).

```ebnf
pipeline_file        = { blank_line | comment | function_decl }, template_head,
                       { pipeline_step | blank_line | comment } ;

function_decl        = function_block_decl | function_expr_decl ;
function_block_decl  = "fn", ws, ident, "(", [ param_list ], ")", ws, "{",
                       newline,
                       { function_binding | function_step | blank_line | comment },
                       "}" ;
function_expr_decl   = "fn", ws, ident, "(", [ param_list ], ")", ws,
                       ( "=" | "=>" ), newline,
                       { function_binding | function_step | blank_line | comment } ;

template_head        = "template", "(", value, ")" ;
pipeline_step        = "|>", ws, call ;
function_step        = "|>", ws, call ;
function_binding     = "let", ws, ident, ws, "=", ws, value ;

call                 = ident, "(", [ arg_list ], ")" ;
arg_list             = arg, { ",", ws, arg } ;
arg                  = value | keyword_arg ;
keyword_arg          = ident, ":", ws, value ;

param_list           = ident, { ",", ws, ident } ;
value                = atom | string | placeholder | raw_token ;
atom                 = ":", ident ;
placeholder          = "${", ident, "}" ;

ident                = ? non-empty identifier token ? ;
string               = ? double-quoted string literal ? ;
raw_token            = ? unquoted token consumed by the current pipeline step ? ;
comment              = "#", ? rest of line ? ;
blank_line           = "" ;
ws                   = { " " | "\t" } ;
newline              = "\n" ;
```

Operational notes:

- exactly one `template(...)` head is allowed per pipeline entry
- `include(...)` is resolved before lowering
- `use(...)` applies a pure function unit by positional arguments
- `let` introduces a local immutable binding inside a function unit
- one pipeline call still occupies one line
- expression-style functions end when the parser reaches the next non-comment,
  non-empty line that does not start with `|>` or `let `

## Package Shape

Minimal gewy packages use:

```text
gewy.pkg
main.gewy
module.gewy
```

Example `gewy.pkg`:

```text
name=demo_app
version=0.1.0
entry=main.gewy
source.local=../registry
dep.std=../stdlib
```

Example `main.gewy`:

```text
template(:demo_app)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:network_module)
```

Example `module.gewy`:

```text
fn network_module() {
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_app_model)
}
```

Included files are merged into the package entry compile path before final
lowering. Current expected shape for included files is pure pipeline function
definitions or pipeline steps, without their own `template(...)` head.

Dependency packages can be resolved from either a direct path or a named source
root. A package can include files from a dependency with:

```text
|> include("std:udp_module.gewy")
```

Where either of these is declared in `gewy.pkg`:

```text
dep.std=../stdlib
```

or:

```text
source.local=../registry
dep.std=source:local/udp_stdlib
```

`gewyc` can also materialize a resolved lock snapshot for a package:

```text
gewyc lock .
```

By default this writes `gewy.lock` next to the resolved package entry.

For exact package/module lookup rules, see
[docs/book/reference-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-gewylang-package.md).

## Legacy Key Surface

The current preferred `gewylang` surface is the pipeline DSL. The flat
top-level key form remains supported as a compatibility surface for existing
fixtures, migration work, and older bindings.

Legacy supported keys are:

- `template`
- `window`
- `window.duration_ms`
- `window.lateness_ms`
- `reason`
- `reason_model`
- `reason.rule`
- `fragment`
- `program_model`
- `operation`
- `rule`
- `param`
- `evidence`

### `template`

String template id for the compiled binding.

Example:

```text
template=udp_process_debug
```

### `window`

Currently supported values:

- `default_5s`

The DSL also supports inline window declarations:

```text
window.duration_ms=5000
window.lateness_ms=200
```

When both inline fields are present, `window=` is optional.

### `reason`

Currently supported values:

- `handshake_l1`
- `udp_datagram_l1`

`reason` is still the simplest way to select a built-in reason profile.

### `reason_model`

Optional string id for a declarative reason model.

If omitted while `reason.rule` lines are present, the compiler synthesizes
`<template>_reason_model`.

### `reason.rule`

Declarative reason-rule format:

```text
reason.rule=<predicate>;<key_event>;<narrative>;<dedupe>
```

Declarative reason rules also support optional trailing `module` and `phase`
fields:

```text
reason.rule=route_resolved;route_changed;route_changed;true;postgres_connect_path;resolve
```

Example:

```text
reason.rule=process_bound;process_identified;process_bound;true
reason.rule=datagram_observed:udp;udp_datagram_seen;udp_datagram_observed;true
reason.rule=route_resolved;route_changed;route_changed;true
```

If one or more `reason.rule` lines are present, the DSL compiles them into a
declarative reason model instead of using a built-in reason profile id.

### `fragment`

Adds one fragment to the binding.

Current built-in fragment ids include:

- `tcp_state_fragment`
- `tcp_packet_meta_fragment`
- `udp_packet_meta_fragment`
- `route_meta_fragment`
- `sock_lineage_fragment`

### `program_model`

String id for the compiled program model.

This is metadata for the runtime/program-flow layer.

If `program_model` is omitted and you provide `operation` or `rule` lines, the
compiler synthesizes an id as `<template>_dsl_model`.

If `program_model`, `operation`, and `rule` are all omitted, the compiler falls
back to the default program model for the selected `reason` profile.

### `operation`

Program-flow operation id.

Built-in values include:

- `connect_flow`
- `datagram_exchange`
- `unknown`

Custom values are also allowed, for example:

```text
operation=dns_lookup
```

### `rule`

Rule format:

```text
rule=<predicate>;<stage>;<narrative>;<dedupe>
```

Fields:

- `predicate`
- `stage`
- `narrative`
- `dedupe`

Optional trailing fields:

- `module`
- `phase`

Example:

```text
rule=datagram_observed:udp;datagram_observed;static:program emitted or received a UDP datagram;true
rule=route_resolved;route_resolved;static:program resolved an upstream route;true;dns_lookup_path;resolve
```

`datagram_observed` also supports an optional direction suffix:

```text
rule=datagram_observed:udp:egress;datagram_observed;static:program emitted a DNS request datagram;true
rule=datagram_observed:udp:ingress;datagram_observed;static:program observed a UDP reply datagram;true
```

The preferred direction names now mirror the flow IR:

```text
rule=datagram_observed:udp:local_to_remote;datagram_observed;udp_datagram_sent;true
rule=datagram_observed:udp:remote_to_local;datagram_observed;udp_datagram_received;true
```

Legacy aliases `egress` and `ingress` are still accepted.

`datagram_observed` also supports optional datagram qualifiers after the
protocol and direction:

- `local:<port|name>`
- `remote:<port|name>`
- `sport:<port|name>`
- `dport:<port|name>`
- `min_len:<u32>`
- `byte0_mask:<u8>:<u8>`
- `prefix2:<u16>`
- `prefix4:<u32>`
- `byte_at:<offset>:<u8>:<u8>`
- `bytes_at:<offset>:<u8>,<u8>,...`

These qualifiers can be combined in suffix order. Example:

```text
rule=datagram_observed:udp:remote:snmp:local_to_remote:byte0_mask:0xff:0x30:byte_at:13:0xff:0xa0;datagram_observed;udp_datagram_sent;true
```

Or with a contiguous byte sequence:

```text
rule=datagram_observed:udp:remote:snmp:bytes_at:8:0x30,0x82,0x01;datagram_observed;udp_datagram_sent;true
```

Current fragment sampling exposes a small default set of payload offsets to
this generic matcher: `0`, `1`, `4`, `5`, `9`, `10`, and `13`. The DSL surface
is now generic even though the underlying fragment templates still define which
offsets are materialized.

Templates can extend the sampled set for a fragment binding with:

```text
param=udp_packet_meta_fragment.sample_payload_offsets=8
```

or:

```text
|> param(:udp_packet_meta_fragment.sample_payload_offsets, "8,12")
```

When a rule uses `byte_at` or `bytes_at` outside the currently sampled
offsets, compiler diagnostics mark that rule as unsupported and include the
unsupported offsets explicitly in the diagnostics report. Validation/findings
surfaces also distinguish this from generic missing-evidence failures, so
unsupported offsets can be reported with a dedicated compiler-facing error
code.

QUIC now also has a parallel structured predicate surface:

- `quic_packet_observed:remote:quic:local_to_remote:min_len:1200:long_header:true:type:initial`
- `quic_packet_observed:remote:quic:remote_to_local:long_header:true:type:handshake`
- `quic_frame_observed:remote:quic:local_to_remote:type:initial:frame:crypto`
- `quic_frame_observed:remote:quic:remote_to_local:type:handshake:frame:crypto`

Supported QUIC qualifiers are:

- `local:<port|name>`
- `remote:<port|name>`
- `sport:<port|name>`
- `dport:<port|name>`
- `min_len:<u32>`
- `long_header:true|false`
- `type:initial|0rtt|handshake|retry`
- `frame:crypto|ack|stream|datagram|connection_close`
- `byte_at:<offset>:<mask>:<value>`
- `bytes_at:<offset>:<byte>,<byte>,...`

This QUIC predicate family is intentionally parallel to the generic
`datagram_observed` surface, so QUIC packet typing does not have to be modeled
as ad hoc UDP byte-offset rules. `quic_frame_observed` builds on a parallel
`QuicMetaFact` surface rather than guessing frame positions from sampled packet
offsets, which keeps QUIC frame matching structurally separate from generic
payload-byte matching.

Named ports currently include:

- `http`
- `https`
- `quic`
- `coap`
- `ntp`
- `stun`
- `dhcp`
- `dhcp_client`
- `dhcp_server`
- `bootpc`
- `bootps`
- `wireguard`
- `mdns`
- `ssdp`
- `postgres`
- `mysql`
- `memcached`
- `amqp`
- `redis`
- `mqtt`
- `radius`
- `smtp`
- `snmp`

`socket_state_observed` also supports an optional destination-port suffix:

```text
rule=socket_state_observed:https;socket_state_transition;static:https socket progress observed;false
rule=socket_state_observed:443;socket_state_transition;static:https socket progress observed;false
```

### `param`

Fragment parameter binding format:

```text
param=<fragment_id>.<key>=<value>
```

Examples:

```text
param=sock_lineage_fragment.capture_comm=false
param=udp_packet_meta_fragment.min_len=80
```

### `evidence`

Template-local evidence tier override format:

```text
evidence=<fact_kind>:<tier>
```

Examples:

```text
evidence=sock_lineage:core_requirement
evidence=packet_meta:optional_enhancement
```

This does not change what the underlying fragment template emits. It only
changes how the compiled binding classifies that evidence in planner
diagnostics. That lets two templates interpret the same fragment evidence with
different priority while still reusing the same stable eBPF fragment templates.

## Predicates

Current predicates are:

- `process_bound`
- `socket_state_observed`
- `route_resolved`
- `datagram_observed:<proto>`
- `all(...)`
- `any(...)`

Examples:

```text
process_bound
datagram_observed:udp
all(process_bound,datagram_observed:udp)
any(route_resolved,socket_state_observed)
```

`all(...)` and `any(...)` operate over flow-local evidence, not only a single
fact.

This predicate vocabulary is now shared by both `rule=` program-flow rules and
`reason.rule=` declarative reason rules, so the DSL only has one flow-evidence
predicate language to learn.

Internally, both now compile into the same shared rule skeleton: predicate +
optional signal + narrative template + dedupe.

For UDP-family protocol modeling, the important point is that
`datagram_observed` is no longer just "some UDP packet happened". It can now
express a bounded protocol fingerprint over:

- transport direction
- local or remote service port
- minimum payload length
- masked first-byte checks
- fixed two-byte prefixes
- fixed four-byte prefixes
- generic byte-at-offset checks over sampled payload offsets

That lets the DSL drive existing fragment templates into useful protocol-path
models without turning the DSL into an eBPF code generator.

`packet_observed` now supports the same direction aliases plus a compact TCP
payload fingerprint surface:

- `local:<port|name>`
- `remote:<port|name>`
- `sport:<port|name>`
- `dport:<port|name>`
- `byte0_mask:<u8>:<u8>`
- `prefix4:<u32>`
- `byte4_mask:<u8>:<u8>`
- `byte_at:<offset>:<u8>:<u8>`

Example:

```text
rule=packet_observed:tcp:remote:redis:local_to_remote:byte0_mask:0xff:0x2a;packet_observed;transport_payload_sent;true
rule=packet_observed:tcp:remote:redis:remote_to_local:prefix4:0x2b504f4e;packet_observed;transport_payload_received;true
rule=packet_observed:tcp:remote:53:remote_to_local:byte4_mask:0x80:0x80;packet_observed;transport_payload_received;true
rule=packet_observed:tcp:remote:53:remote_to_local:byte_at:4:0x80:0x80;packet_observed;transport_payload_received;true
```

The DSL compiler also validates that the selected fragment set can actually
produce the evidence each rule depends on. A rule that references
`process_bound`, for example, now fails at compile time unless the binding
includes a fragment that emits `sock_lineage`.

Planner diagnostics also classify rules into:

- `core_requirement`
- `optional_enhancement`
- `unsupported`

By default these tiers come from the selected fragment descriptors, but a
template can override them with `evidence=...` lines when a specific network
module view wants to treat the same evidence differently.

## Stages

Current stage values are:

- `none`
- `process_bound`
- `socket_state_transition`
- `datagram_observed`
- `route_resolved`

These stage ids now live in the same shared signal vocabulary as declarative
reason key events.

## Narrative Values

Current narrative forms are:

- `none`
- `process_bound`
- `tcp_state_transition`
- `route_changed`
- `udp_datagram_observed`
- `udp_datagram_sent`
- `udp_datagram_received`
- `transport_payload_sent`
- `transport_payload_received`
- `static:<text>`

This narrative vocabulary is shared by both `rule=` and `reason.rule=`. The
same IR template can be materialized differently in program-flow and reason
views, but it is declared only once in the DSL.

Likewise, `reason.rule=<predicate>;<key_event>;...` now accepts the shared
signal ids directly. For example, `process_bound`, `datagram_observed`, and
`route_resolved` can be used as declarative reason key events and will be
materialized into the appropriate reason-chain event forms.

Examples:

```text
none
process_bound
udp_datagram_sent
static:program resolved a route for this network flow
```

DSL narrative templates do not add new kernel behavior. They only shape how the
runtime interprets facts emitted by the selected fragment templates.

## Dedupe

The fourth rule field is a boolean:

- `true`
- `false`

When `true`, the rule only contributes once per program flow.

## Fragment Parameter Schema

Fragment parameters are statically validated against fragment descriptor schema
at DSL compile time and again when building `SessionConfig`.

Current built-in parameters are:

- `sock_lineage_fragment.capture_comm: bool`
- `udp_packet_meta_fragment.min_len: u64`

Examples:

- valid:

```text
param=sock_lineage_fragment.capture_comm=false
param=udp_packet_meta_fragment.min_len=80
```

- invalid key:

```text
param=sock_lineage_fragment.not_a_real_param=true
```

- invalid type:

```text
param=udp_packet_meta_fragment.min_len=false
```

## CLI Usage

The most common command families are:

- compile a `.gewy` file into a binding
- inspect the front-end/package surface
- inspect diagnostics/findings/stages
- run a DSL-backed runtime session

Typical examples:

- compile and run a DSL-backed session:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --summary-only
```

- compile a `.gewy` file without starting the runtime:

```bash
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --out /tmp/udp-process-binding.json
```

- inspect the front-end/package surface:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation
```

- inspect diagnostics/findings/stages:

```bash
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

The `validation` section in `stages` now summarizes payload offset-matcher
coverage for the selected fragment set:

- `sampled_payload_offsets`
- `required_payload_offsets`
- `unsupported_payload_offsets`

If parse or validation fails, `stages` still records that failure as a
stage-local finding, so frontends can inspect partial compiler state without
falling back to an unstructured error string. Only outer file read failures stay
outside the staged report surface.

- run a socket session from a DSL file:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --unix-socket /tmp/gewyvern.sock --json
```

For task-oriented compiler and runtime validation flows, prefer:

- [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

## Current Limits

- The DSL is still intentionally small
- It compiles into `TemplateBinding`, not into new fragment descriptors
- It does not generate eBPF bytecode
- Window profiles and reason profiles are still selected from built-in ids
- Narrative rendering is still intentionally simple
- UDP-family protocol recognition is still based on compact flow evidence
  fingerprints, not full parser completeness
- `gewyc` is currently a separate workspace crate that still reuses
  `gewyvern`'s shared DSL/compiler library surface

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
