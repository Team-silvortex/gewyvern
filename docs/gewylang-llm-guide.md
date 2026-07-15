# GewyLang Guide For Humans And Language Models

This is the shortest authoritative context for generating or reviewing current
GewyLang (`.gewy`) source. Read this page before sampling arbitrary files from
`dsl/` or `protocols/`.

All generated source must follow the
[GewyLang Canonical Style Standard](gewylang-style.md).

## Language Identity

GewyLang is a narrow, declarative binding language. It selects and
parameterizes prebuilt gewyvern runtime capabilities. It does not generate
eBPF bytecode and is not a general-purpose programming language.

The compile boundary is:

```text
.gewy source -> frontend module -> TemplateBinding -> validated runtime IR
```

## Generation Contract

When generating new source, obey these rules:

1. Emit exactly one `template :id` entry head.
2. Put one pipeline call on each line.
3. Prefer parenless calls unless named arguments improve readability.
4. Put reusable behavior in pure `fn name(...) =` units before the template.
5. Use `let` only inside a function unit; bindings are immutable and local.
6. Use `$name` for parameters and local bindings.
7. Use `include` for source composition and `use` for function application.
8. Use positional arguments before named arguments; never reverse that order.
9. Do not invent pipeline step names, fragment ids, parameter keys, signal ids,
   or built-in narratives.
10. Run `gewyc envelope` before treating generated source as valid.

Canonical style:

```gewy compile
//! Minimal UDP observation package.

/// Reusable capability and rule bundle.
fn udp_rules(model_name: atom, dedupe: bool = true) =
  let transport = "datagram_observed:udp"
  |> fragment :udp_packet_meta_fragment
  |> fragment :route_meta_fragment
  |> fragment :sock_lineage_fragment
  |> operation :datagram_exchange
  |> program_model $model_name
  |> program_rule pred: :process_bound, stage: :process_bound, narr: :process_bound, dedupe: $dedupe
  |> program_rule pred: $transport, stage: :datagram_observed, narr: :udp_datagram_observed, dedupe: $dedupe

/// Single package entry.
template :udp_observer
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason :udp_datagram_l1
|> use :udp_rules, :udp_observer_model
|> param :sock_lineage_fragment.capture_comm, true
```

## Canonical Syntax

```text
fn function_name(required: atom, optional: bool = true) =
  let local_name = :value
  |> step :single_argument
  |> step positional, named: value

template :template_id
|> window(duration_ms: 5000, lateness_ms: 200)
|> use :function_name, :argument, optional: false
```

Accepted source values include:

- atoms: `:identifier`
- strings: `"text"`
- booleans: `true`, `false`
- unsigned integers: `5000`
- placeholders: `$name`
- named arguments: `name: value`

Comments:

- `# comment`
- `/* block comment */`
- `//! module documentation`
- `/// next function or template documentation`

## Pipeline Steps

These are the complete current step names:

| Step | Purpose |
| --- | --- |
| `template` | Declare the single entry template. |
| `window` | Select a window profile or set `duration_ms` and `lateness_ms`. |
| `reason` | Select a built-in reason profile. |
| `fragment` | Add a prebuilt fragment capability. |
| `program_model` | Name the program-flow model. |
| `reason_model` | Name the declarative reason model. |
| `operation` | Name the program operation. |
| `program_rule` | Add one program-flow rule. |
| `reason_rule` | Add one reason rule. |
| `param` | Bind one declared fragment parameter. |
| `evidence` | Override one evidence tier. |
| `include` | Merge another source file before lowering. |
| `use` | Apply a declared function unit. |

Unknown steps are compile errors. GewyLang has no generic assignment,
condition, loop, class, import, macro, or mutation statement.

## Rule Calls

Prefer named rule fields:

```text
|> program_rule pred: PREDICATE, stage: SIGNAL, narr: NARRATIVE, dedupe: BOOL, mod: :module, phase: :phase
|> reason_rule pred: PREDICATE, event: SIGNAL, narr: NARRATIVE, dedupe: BOOL, mod: :module, phase: :phase
```

Aliases are canonical and mean:

- `pred` -> `predicate`
- `narr` -> `narrative`
- `mod` -> `module`
- `event` -> `key_event` for `reason_rule`

The first four fields may be positional, but named fields are safer for model
generation. `mod` and `phase` are optional.

## Function And Package Rules

- Function units are composition helpers, not independently executed modules.
- Defaults must be trailing parameters.
- A call may use positional arguments, named arguments, or positional followed
  by named arguments.
- Duplicate, unknown, missing, or extra arguments are compile errors.
- Recursive `use` and cyclic `include` graphs are compile errors.
- An included helper file should not declare its own `template` head.
- A package normally contains `gewy.pkg`, `main.gewy`, and optional helpers.

Minimal manifest:

```ini
name=udp_observer
version=0.1.0
entry=main.gewy
```

## Stable Value Vocabulary

Do not infer vocabulary from names alone. Use the exact tables in
[DSL Reference](dsl-reference.md), especially for:

- predicates and payload qualifiers
- signal/stage and reason-event ids
- narrative templates
- built-in fragments and fragment parameters
- evidence tiers

Custom operation, model, module, and phase ids are allowed where their
respective reference sections say so. Custom pipeline steps and built-in
fragment ids are not allowed.

## Legacy Input Boundary

The parser may accept syntax outside the canonical standard for migration.
Never emit those forms and never infer style from parser acceptance. When old
input must be converted, use [the migration guide](gewylang-migration.md).

## Validation Loop

For generated or modified source, use this order:

```bash
cargo run -p gewyc -- envelope path/to/main.gewy --json
cargo run -p gewyc -- findings path/to/main.gewy --json
cargo run -p gewyc -- frontend path/to/main.gewy --focus graph
cargo run -p gewyc -- path/to/main.gewy --json
```

Interpretation:

1. `envelope` gives the shortest overall status and next step.
2. `findings` gives stable machine-readable failures.
3. `frontend --focus graph` verifies include and use composition.
4. normal compile confirms the validated `TemplateBinding`.

Never claim a generated file is valid only because it resembles another
protocol file.

## High-Value Failure Checklist

Before compilation, check:

- exactly one template head
- no pipeline call split across lines
- no positional argument after a named argument
- no missing `$` on function parameters or local bindings
- all `use` targets declared or included
- fragment parameters belong to the selected fragment
- rule predicates, signals, narratives, and booleans use valid value families
- TCP payload rules select both `tcp_state_fragment` and
  `tcp_packet_meta_fragment`
- UDP payload rules select `udp_packet_meta_fragment`
- process rules select `sock_lineage_fragment`
- route rules select `route_meta_fragment`

The compiler and planner remain the authority when these heuristics disagree.

## Source Priority

When sources conflict, use this priority:

1. [the canonical style standard](gewylang-style.md), compiler behavior, and tests
2. this guide and [the EBNF](gewylang.ebnf)
3. [syntax guide](dsl-syntax.md) and [vocabulary reference](dsl-reference.md)
4. package tutorial and examples
5. migration inputs only when conversion is the task; never use them as
   generation authority

For deeper context, continue with [the GewyLang module](modules/gewylang.md).
