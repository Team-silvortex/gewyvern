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

- atoms: `:identifier` (or a step-specific identifier path such as `:fragment.field`)
- strings: `"text"`
- booleans: `true`, `false`
- unsigned integers: `5000`
- placeholders: `$name`
- named arguments: `name: value`

Quoted strings decode exactly five escapes: `\"`, `\\`, `\n`, `\r`, and `\t`.
All other backslash escapes are invalid; do not emit JSON-style `\u` escapes.
Never emit raw control characters inside strings; use `\t`, `\n`, or `\r`.
Parentheses inside quoted strings are literal text. Outside strings, a
parenthesized call has exactly one outer `(` ... `)` pair and cannot nest calls.
Inline `window` accepts only `duration_ms` and `lateness_ms`, exactly once each.
Never emit duplicate keyword fields; duplicate rule aliases are also invalid.
Argument lists cannot contain leading, trailing, or repeated commas.
Every named argument requires a value after `:`.
`let` bindings use exactly one unquoted `=`; parameter defaults use at most one.
Keep each source file at or below 256 KiB; split larger programs with packages and includes.

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

For automatic repair, branch on `findings[].code`, not the English message:

| Finding code | Repair action |
| --- | --- |
| `GEWYC-PARSE-UNKNOWN-PIPELINE-STEP` | Replace the step with one from the Pipeline Steps table. |
| `GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION` | Declare or include the named function before `use`. |
| `GEWYC-PARSE-UNKNOWN-PARAMETER-KIND` | Use one of `atom`, `bool`, `u64`, `predicate`, `narrative`, `stage`, `key_event`, or `phase`. |
| `GEWYC-PARSE-PARAMETER-KIND-CONFLICT` | Align the annotation with every use-site, or split the parameter. |
| `GEWYC-PARSE-ARGUMENT-TYPE-MISMATCH` | Supply a value from the parameter's reported value family. |
| `GEWYC-PARSE-UNKNOWN-PLACEHOLDER` | Replace `$name` with a reported in-scope parameter or local binding. |
| `GEWYC-PARSE-INVALID-PLACEHOLDER` | Replace braced or malformed syntax with one complete `$name` placeholder. |
| `GEWYC-PARSE-INVALID-LITERAL` | Use an atom, quoted string, boolean, unsigned decimal integer, or `$name`. |
| `GEWYC-PARSE-STRING-INTERPOLATION` | Move `$name` outside the quoted string and pass it as a standalone value. |
| `GEWYC-PARSE-UNCLOSED-PLACEHOLDER` | Remove the incomplete braced form and use `$name`. |
| `GEWYC-PARSE-PLACEHOLDER-EXPANSION-LIMIT` | Break the reported transitive placeholder chain into concrete values. |
| `GEWYC-PARSE-UNKNOWN-NAMED-ARGUMENT` | Use a parameter name from the function signature. |
| `GEWYC-PARSE-DUPLICATE-ARGUMENT` | Supply each function parameter exactly once. |
| `GEWYC-PARSE-ARGUMENT-ORDER` | Move every positional argument before the first named argument. |
| `GEWYC-PARSE-FUNCTION-ARITY` | Add or remove arguments to match the reported function signature. |
| `GEWYC-PARSE-INVALID-FUNCTION-SIGNATURE` | Rebuild every `fn` declaration as `fn name(params) =`; never omit the final `=`. |
| `GEWYC-PARSE-INVALID-FUNCTION-NAME` | Use an ASCII identifier beginning with a letter or `_`. |
| `GEWYC-PARSE-INVALID-ATOM` | Use `:identifier` or a dot-separated identifier path with no whitespace or empty segments. |
| `GEWYC-PARSE-INVALID-KEYWORD-NAME` | Replace the field name with one bare identifier before checking whether that field is supported. |
| `GEWYC-PARSE-DUPLICATE-FUNCTION` | Rename or remove one function declaration; includes may not redefine functions. |
| `GEWYC-PARSE-DUPLICATE-PARAMETER` | Keep each parameter name once in the function signature. |
| `GEWYC-PARSE-DUPLICATE-LOCAL-BINDING` | Rename or remove the repeated local `let` binding. |
| `GEWYC-PARSE-INVALID-PARAMETER-ORDER` | Move every required parameter before parameters with defaults. |
| `GEWYC-PARSE-MISSING-PARAMETER-DEFAULT` | Add a value after `=`, or remove `=`. |
| `GEWYC-PARSE-INVALID-PARAMETER-NAME` | Use a bare ASCII identifier beginning with a letter or `_`; never prefix parameter or `let` names with `:`. |
| `GEWYC-PARSE-UNCLOSED-STRING` | Close the reported string with `"`; use `\"` for an embedded quote. |
| `GEWYC-PARSE-INVALID-STRING-ESCAPE` | Replace the escape with one of `\"`, `\\`, `\n`, `\r`, or `\t`. |
| `GEWYC-PARSE-INVALID-STRING-CHARACTER` | Replace the raw control character with `\t`, `\n`, or `\r`, or remove it. |
| `GEWYC-PARSE-INVALID-LET-BINDING` | Rebuild the local binding as `let name = value`. |
| `GEWYC-PARSE-INVALID-PIPELINE-CALL` | Close the call and keep one complete pipeline call on the line. |
| `GEWYC-PARSE-UNKNOWN-RULE-FIELD` | Replace the field with a documented rule field or alias. |
| `GEWYC-PARSE-DUPLICATE-RULE-FIELD` | Keep only one canonical field or alias per rule value. |
| `GEWYC-PARSE-DUPLICATE-WINDOW-FIELD` | Keep exactly one `duration_ms` and one `lateness_ms` field. |
| `GEWYC-PARSE-UNKNOWN-WINDOW-FIELD` | Remove the field or replace it with `duration_ms` or `lateness_ms`. |
| `GEWYC-PARSE-UNKNOWN-REASON-PROFILE` | Select a reason profile from the protocol registry. |
| `GEWYC-PARSE-UNKNOWN-STAGE` | Replace the stage with a registered signal kind, or use `none` where allowed. |
| `GEWYC-PARSE-UNKNOWN-KEY-EVENT` | Replace the key event with a registered signal kind, or use `none`. |
| `GEWYC-PARSE-UNKNOWN-EVIDENCE-FACT-KIND` | Select a fact kind from the protocol registry. |
| `GEWYC-PARSE-UNKNOWN-EVIDENCE-TIER` | Use `core_requirement` or `optional_enhancement`. |
| `GEWYC-PARSE-INVALID-FRAGMENT-PARAM-TARGET` | Rewrite the target as `fragment_id.parameter_key`. |
| `GEWYC-PARSE-UNKNOWN-WINDOW-PROFILE` | Use `default_5s`, or provide both `duration_ms` and `lateness_ms`. |
| `GEWYC-PARSE-INVALID-BOOLEAN` | Replace the value with the unquoted literal `true` or `false`. |
| `GEWYC-PARSE-INVALID-INTEGER` | Replace the value with a non-negative decimal integer in range. |
| `GEWYC-PARSE-INVALID-STEP-ARITY` | Match the argument count stated by the finding for that pipeline step. |
| `GEWYC-PARSE-MALFORMED-ARGUMENT` | Rewrite the argument as `name: value` and ensure the value is present. |
| `GEWYC-PARSE-EMPTY-ARGUMENT` | Remove the extra comma or provide the missing argument. |
| `GEWYC-PARSE-UNCLOSED-BLOCK-COMMENT` | Add the closing `*/` after the block comment. |
| `GEWYC-PARSE-MULTIPLE-ASSIGNMENT-SEPARATORS` | Quote the value if `=` is data, or remove the extra assignment separator. |
| `GEWYC-PARSE-RULE-PHASE-WITHOUT-MODULE` | Add `module: value`, or remove `phase`. |
| `GEWYC-PARSE-MISSING-TEMPLATE-HEAD` | Add exactly one `template` head to the entry pipeline. |
| `GEWYC-PARSE-INVALID-TEMPLATE-HEAD` | Rebuild the declaration as `template :identifier` with exactly one value. |
| `GEWYC-PARSE-SOURCE-TOO-LARGE` | Split the source into smaller package files, each no larger than 256 KiB. |
| `GEWYC-PARSE-DUPLICATE-TEMPLATE-HEAD` | Keep one entry-level `template` head; included modules must not declare one. |
| `GEWYC-PARSE-MISSING-PIPELINE-PREFIX` | Prefix each step after the template with `|>`. |
| `GEWYC-PARSE-INCLUDE-CYCLE` | Remove one include edge from the reported cycle. |
| `GEWYC-PARSE-USE-CYCLE` | Remove one function `use` edge from the reported cycle. |
| `GEWYC-PARSE-UNKNOWN-PACKAGE-SOURCE` | Declare the source in `gewy.pkg`, or correct its name. |
| `GEWYC-PARSE-INVALID-SOURCE-DEPENDENCY` | Rewrite it as `source:<name>/<package>`. |
| `GEWYC-PARSE-INCLUDE-ESCAPES-PACKAGE` | Move the file under the package or dependency root and include that path. |
| `GEWYC-PARSE-UNRESOLVED-INCLUDE` | Resolve includes through a filesystem-backed package entry before lowering. |
| `GEWYC-PARSE-UNSUPPORTED-SYNTAX` | Rewrite the input using the pipeline stable subset. |
| `GEWYC-PARSE-UNKNOWN-TRANSPORT-PROTOCOL` | Use `tcp`, `udp`, or a numeric IP protocol value. |
| `GEWYC-PARSE-UNKNOWN-PREDICATE` | Replace the predicate with one from the Predicate Vocabulary table. |
| `GEWYC-PARSE-MISSING-PREDICATE-QUALIFIER` | Add the qualifier named by the finding before regenerating the rule. |
| `GEWYC-PARSE-INVALID-PREDICATE-QUALIFIER` | Replace the reported qualifier with a valid port, type, width, or suffix value. |

Unknown future codes must remain fatal to generation; do not guess a repair
from message text and then claim successful validation.

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
