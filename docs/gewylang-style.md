# GewyLang Canonical Style Standard

This page defines the single source style for maintained `.gewy` files,
generated packages, examples, and model output. It is normative for `dsl/` and
`protocols/`.

Parser compatibility is broader than this standard. A form being accepted by
the parser does not make it canonical.

## Complete File Shape

```gewy compile
# gewyvern stable-subset protocol path

fn demo_rules(model_name: atom, dedupe: bool = true) =
  let module_name = :demo_path
  let payload = "datagram_observed:udp"
  |> fragment :udp_packet_meta_fragment
  |> fragment :sock_lineage_fragment
  |> operation :demo_exchange
  |> program_model $model_name
  |> program_rule pred: :process_bound, stage: :process_bound, narr: :process_bound, dedupe: $dedupe, mod: $module_name, phase: :bind
  |> program_rule pred: $payload, stage: :datagram_observed, narr: :udp_datagram_observed, dedupe: $dedupe, mod: $module_name, phase: :observe

template :demo_path
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason :udp_datagram_l1
|> use :demo_rules, :demo_path_model
|> param :sock_lineage_fragment.capture_comm, true
```

Every maintained entry follows this order:

1. optional module or line documentation
2. one or more reusable function units
3. exactly one template head
4. entry pipeline steps

## Function Units

The only canonical declaration form is:

```text
fn name(parameters) =
  let local = value
  |> step arguments
```

Rules:

- use `=` rather than `=>`
- do not use braces
- defaults appear only after required parameters
- add explicit kinds when a reusable boundary would otherwise be ambiguous
- keep function units pure and local to composition
- use lower `snake_case` for function, parameter, and local names

## Calls

Use parenless calls for ordinary pipeline steps:

```text
template :demo_path
|> fragment :udp_packet_meta_fragment
|> use :demo_rules, :demo_model
```

Use the parenthesized form only for `window` because its two named numeric
fields form one structured value:

```text
|> window(duration_ms: 5000, lateness_ms: 200)
```

There is one call per physical line. Arguments stay on that line.

## Values And Bindings

Canonical values are:

- stable ids and enum-like values: `:snake_case_atom`
- complex predicates: `"packet_observed:tcp:remote:https"`
- custom narratives: `"static:human-readable explanation"`
- booleans: `true` or `false`
- unsigned integers: decimal integers
- placeholders: `$name`

Use `let` when a value is repeated or carries domain meaning:

```text
let module_name = :http_request_path
let request_predicate = "packet_observed:tcp:remote:https:local_to_remote"
```

Do not use legacy `${name}` placeholders or unquoted complex colon-delimited
predicates in maintained source.

## Rules

Program and reason rules always use named compact fields:

```text
|> program_rule pred: $predicate, stage: :packet_observed, narr: :transport_payload_sent, dedupe: true, mod: $module_name, phase: :send_request
|> reason_rule pred: $predicate, event: :packet_observed, narr: :transport_payload_sent, dedupe: true, mod: $module_name, phase: :send_request
```

Required field order is:

1. `pred`
2. `stage` for program rules or `event` for reason rules
3. `narr`
4. `dedupe`
5. optional `mod`
6. optional `phase`

Do not use positional rule fields. Pair program and reason rules when both
runtime materialization and explanation need the same evidence.

## Capability Coherence

Rules must be backed by selected fragments:

| Rule evidence | Required capability |
| --- | --- |
| process binding | `sock_lineage_fragment` |
| TCP state | `tcp_state_fragment` |
| TCP payload | `tcp_state_fragment` and `tcp_packet_meta_fragment` |
| UDP payload | `udp_packet_meta_fragment` |
| route resolution | `route_meta_fragment` |

The compiler and registry validator are authoritative for exact coverage.

## Naming

- template: `<protocol>_<behavior>_path` or an established session/model suffix
- function: template stem plus `_rules`
- program model: template id plus `_model`
- reason model: template id plus `_reason`
- module: stable template or sub-flow identity
- phase: lower `snake_case` verb or state, such as `send_request`

Do not rename established public template ids merely for stylistic symmetry.

## Package And Mirror Model

Every `protocols/**/main.gewy` is a complete canonical entry. Include-only
entry aliases are not allowed.

When a package template id has a matching `dsl/<template_id>.gewy`, the two
files are maintained as byte-identical mirrors. A small number of package-only
protocol entries may live only under `protocols/`; they still follow every
syntax and style rule on this page.

`include` remains valid for user package composition, but repository protocol
entries are deliberately self-contained so tools and language models can read
one file without resolving hidden aliases.

## Comments And Documentation

- use `#` for short implementation context
- use `//!` for package/module documentation surfaced by the frontend
- use `///` for function or template documentation surfaced by the frontend
- use `/* ... */` only when a real multi-line explanation is necessary

Comments explain intent or constraints, not syntax visible from the line.

## Forbidden In Maintained Sources

- flat `key=value` legacy DSL
- `template(...)`
- parenthesized calls other than `window(...)`
- `fn name(...) =>`
- `fn name(...) { ... }`
- `fn name(...) = { ... }`
- `${name}` placeholders
- positional rule fields
- unquoted complex predicates
- include-only protocol entries
- multiple template heads

## Validation

Run at least:

```bash
cargo test --test gewylang_docs_tdd
cargo test --test dsl_tdd
cargo run --bin gewyvern_validate -- registry
```

The first command enforces this style, documentation examples, vocabulary
coverage, links, and mirror consistency. The other commands prove compiler and
registry behavior.
