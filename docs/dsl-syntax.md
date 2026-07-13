# DSL Syntax And Package Shape

Use this page when you already know what `gewylang` is and now want the stable
authoring surface: pipeline shape, function-unit style, package layout, and
CLI-facing inspection flow.

This page is the syntax companion to:

- [docs/dsl.md](docs/dsl.md)
- [docs/dsl-reference.md](docs/dsl-reference.md)
- [docs/book/reference-gewylang-package.md](docs/book/reference-gewylang-package.md)

## Pipeline Shape

Top-level pipeline files start with:

```text
template :template_id
```

Single-argument calls can also use the shorter stable form:

```text
template :template_id
```

Then extend the binding with Elixir-style pipeline steps:

- `|> window :default_5s` or `|> window :default_5s`
- `|> window(duration_ms: 5000, lateness_ms: 200)`
- `|> reason :udp_datagram_l1` or `|> reason :udp_datagram_l1`
- `|> fragment :udp_packet_meta_fragment` or `|> fragment :udp_packet_meta_fragment`
- `|> program_model :example_model` or `|> program_model :example_model`
- `|> reason_model :example_reason`
- `|> operation :datagram_exchange` or `|> operation :datagram_exchange`
- `|> param :sock_lineage_fragment.capture_comm, true` or `|> param :sock_lineage_fragment.capture_comm, true`
- `|> evidence :sock_lineage, :core_requirement` or `|> evidence :sock_lineage, :core_requirement`
- `|> program_rule(...)`
- `|> reason_rule(...)`
- `|> include "./module.gewy"` or `|> include "./module.gewy"`
- `|> use :network_module` or `|> use :network_module`
- `|> use :network_module, :demo_app_model, :datagram_exchange` or `|> use :network_module, :demo_app_model, :datagram_exchange`

Rule steps also accept a compact keyword surface for the longest field names:

- `pred` as an alias for `predicate`
- `narr` as an alias for `narrative`
- `mod` as an alias for `module`
- `event` as an alias for `key_event` in `reason_rule(...)`

Rule steps may also use positional shorthand for the four required core fields:

- `program_rule predicate, stage, narrative, dedupe`
- `reason_rule predicate, key_event, narrative, dedupe`
- optional `module` and `phase` stay named, for example:
  `|> program_rule :process_bound, :process_bound, :process_bound, true, mod: :demo, phase: :bind`

Current parser rule: one pipeline call per line.

## Comments

`gewylang` supports lightweight comments intended for real authoring, not just
temporary debugging notes.

Line comments:

```text
template :demo_app # entry binding for the demo
|> window :default_5s # keep the default demo window
```

Block comments:

```text
/*
  Reusable UDP fragment bundle.
  Keep this block small and composable.
*/
fn udp_core() =
  |> fragment :udp_packet_meta_fragment
  |> operation :datagram_exchange
```

Current comment rules:

- `#` starts a line comment outside string literals
- `/* ... */` starts and ends a block comment outside string literals
- comment stripping preserves line layout so compiler line numbers stay stable

## Documentation Comments

`gewylang` also supports lightweight documentation comments for author-facing
surfaces.

Module header docs:

```text
//! UDP demo package
//! Keeps the entry pipeline intentionally small
/// Entry template for the demo package
template :udp_demo
|> window :default_5s
```

Function docs:

```text
/// Reusable UDP rule bundle shared by multiple templates.
fn udp_rules() =
  |> operation :datagram_exchange
  |> program_model :udp_rules_model
```

Current doc rules:

- `//!` appends to the module header doc surface
- `///` attaches to the next `fn ...` declaration
- if `///` appears before the entry `template ...`, it attaches to the entry
  template doc surface
- blank lines do not break pending `///` attachment
- plain `#` comments still break pending `///` attachment

## Function Units

Function units can be declared in two equivalent forms.

Preferred expression-style form:

```text
fn network_module() =
  let module_name = :udp_module
  |> fragment :udp_packet_meta_fragment
  |> fragment :route_meta_fragment
  |> operation :datagram_exchange
```

`=>` is accepted as an alias:

```text
fn network_module(model_name, op_name) =>
  let default_phase = :bind
  |> fragment :udp_packet_meta_fragment
  |> operation $op_name
  |> program_model $model_name
```

Tail parameters may also carry defaults:

```text
fn network_module(model_name, op_name = :datagram_exchange) =>
  |> fragment :udp_packet_meta_fragment
  |> operation $op_name
  |> program_model $model_name
```

That lets `use :network_module, :demo_model` override the first parameter
while still falling back to the default operation.

## Named `use(...)` Arguments

`use(...)` may also pass named arguments:

```text
fn network_module(model_name, op_name = :datagram_exchange) =>
  |> fragment :udp_packet_meta_fragment
  |> operation $op_name
  |> program_model $model_name

template :demo_app
|> window :default_5s
|> reason :udp_datagram_l1
|> use :network_module, op_name: :stream_exchange, model_name: :demo_model
```

Current rule:

- the first `use(...)` argument is still the function name
- parenless `use :fn_name, ...` follows the same argument rules
- positional arguments may come first
- named arguments may follow
- positional arguments may not appear after named arguments
- named arguments must match declared parameter names

## Lightweight Inferred Parameter Kinds

Function parameters carry a lightweight inferred kind surface. `gewylang` does
not implement a full global type system, but it does infer parameter intent
from placeholder usage inside a function body. Placeholders support both the
explicit `$name` form and the shorthand `$name` form.

Current inferred or declared kinds are:

- `atom`
- `bool`
- `u64`
- `predicate`
- `narrative`

Example:

```text
fn udp_core(model_name, op_name = :datagram_exchange, dedupe_flag = true, duration_ms = 5000) =>
  |> window(duration_ms: $duration_ms, lateness_ms: 200)
  |> operation $op_name
  |> program_model $model_name
  |> program_rule :process_bound, :process_bound, :process_bound, $dedupe_flag, mod: :frontend_summary, phase: :bind
```

In that function:

- `model_name` infers as `atom`
- `op_name` infers as `atom`
- `dedupe_flag` infers as `bool`
- `duration_ms` infers as `u64`

Current hard validation is intentionally narrow:

- inferred `bool` parameters are validated at `use(...)` application time
- inferred `u64` parameters are validated at `use(...)` application time
- other inferred kinds are advisory/reporting-only for now

You may also declare a lightweight kind directly in the function signature when
you want the contract to be explicit:

```text
fn udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms: u64 = 5000) =>
  |> window(duration_ms: $duration_ms, lateness_ms: 200)
  |> program_model $model_name
  |> program_rule :process_bound, :process_bound, :process_bound, $dedupe_flag, mod: :frontend_summary, phase: :bind
```

Explicit kinds use the same value-family names and must agree with actual
function-body usage. If they disagree, `gewylang` fails at compile time.

## Block Form

The original block form is still supported for compatibility:

```text
fn network_module() {
|> fragment :udp_packet_meta_fragment
|> fragment :route_meta_fragment
|> operation :datagram_exchange
}
```

Block functions can also be parameterized:

```text
fn network_module(model_name, op_name) {
|> fragment :udp_packet_meta_fragment
|> operation $op_name
|> program_model $model_name
}
```

And then applied from the entry pipeline:

```text
template :demo_app
|> window :default_5s
|> reason :udp_datagram_l1
|> include "./module.gewy"
|> use :network_module
```

## Stable Subset

The current recommended stable subset is intentionally small:

- one package entry file with exactly one `template ...` head
- pipeline steps with one call per line
- pure function units declared with either `fn ... =` or `fn ... { ... }`
- positional `use :fn_name, ...` function application
- positional-then-named `use :fn_name, ..., key: value` application
- trailing default parameters for function units
- local immutable `let` bindings inside function units
- `include(...)` for file composition
- keyword-style `program_rule(...)` and `reason_rule(...)`
- compact rule aliases such as `pred`, `narr`, `mod`, and `event`
- positional rule shorthand for the four required core fields

Features that are still legal but should be treated as transitional or
lower-preference:

- large hand-written inline entry pipelines without reusable function units

## Pipeline Idioms

Recommended `gewylang` style is intentionally small and regular:

- prefer expression-style `fn ... =` for short reusable modules
- use `let` for local names that improve readability, not for deep mini-scope
  trees
- keep one conceptual action per `|>` line
- pass variability in through function parameters, then derive local aliases
  with `let`
- keep `template ...` heads shallow and move reusable behavior into function
  units
- prefer `use :module_name, ...` composition over repeating the same fragment
  and rule bundle inline

Example:

```text
fn udp_client(model_name) =
  let module_name = :udp_client
  let op_name = :datagram_exchange
  |> fragment :udp_packet_meta_fragment
  |> fragment :route_meta_fragment
  |> fragment :sock_lineage_fragment
  |> operation $op_name
  |> program_model $model_name
  |> program_rule :process_bound, :process_bound, :process_bound, true, mod: $module_name, phase: :bind

template :demo_app
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_client, :demo_app_model
```

## Pipeline EBNF

The canonical draft grammar also lives in
[docs/gewylang.ebnf](docs/gewylang.ebnf).

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

param_list           = param_decl, { ",", ws, param_decl } ;
param_decl           = ident, [ ":", ws, kind_name ], [ ws, "=", ws, value ] ;
value                = atom | string | placeholder | raw_token ;
atom                 = ":", ident ;
placeholder          = "${", ident, "}" | "$", ident ;
kind_name            = "atom" | "bool" | "u64" | "predicate" | "narrative" | "stage" | "key_event" | "phase" ;
```

Operational notes:

- exactly one `template ...` head is allowed per pipeline entry
- `include(...)` is resolved before lowering
- `use(...)` applies a pure function unit
- `let` introduces a local immutable binding inside a function unit
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
template :demo_app
|> window :default_5s
|> reason :udp_datagram_l1
|> include "./module.gewy"
|> use :network_module
```

Example `module.gewy`:

```text
fn network_module() {
|> fragment :udp_packet_meta_fragment
|> fragment :route_meta_fragment
|> fragment :sock_lineage_fragment
|> operation :datagram_exchange
|> program_model :demo_app_model
}
```

Included files are merged into the package entry compile path before final
lowering. Included files should define pure pipeline function definitions or
steps, without their own `template ...` head.

Dependency packages can be resolved from either a direct path or a named
source root. A package can include files from a dependency with:

```text
|> include "std:udp_module.gewy"
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

For exact package/module lookup rules, use
[docs/book/reference-gewylang-package.md](docs/book/reference-gewylang-package.md).

## CLI Usage

The most common command families are:

- compile a `.gewy` file into a binding
- inspect the front-end/package surface
- inspect diagnostics/findings/stages
- run a DSL-backed runtime session

Typical examples:

```bash
cargo run -- --dsl dsl/udp_process_debug.gewy --json --summary-only
cargo run -p gewyc -- dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- explain dsl/udp_process_debug.gewy --focus validation
cargo run -p gewyc -- diagnostics dsl/udp_process_debug.gewy
cargo run -p gewyc -- findings dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- stages dsl/udp_process_debug.gewy --json
cargo run -- --dsl dsl/udp_process_debug.gewy --unix-socket /tmp/gewyvern.sock --json
```

For task-oriented validation flows, prefer:

- [docs/book/tutorial-gewylang-package.md](docs/book/tutorial-gewylang-package.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)
- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
