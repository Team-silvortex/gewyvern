# Migrating Legacy GewyLang Source

This page exists only for converting old input. It is not a generation guide.
New source follows [the canonical style standard](gewylang-style.md).

## Syntax Conversion

| Legacy input | Canonical source |
| --- | --- |
| `template(:demo)` | `template :demo` |
| `|> fragment(:udp_packet_meta_fragment)` | `|> fragment :udp_packet_meta_fragment` |
| `fn rules() =>` | `fn rules() =` |
| `fn rules() { ... }` | `fn rules() =` followed by pipeline lines |
| `fn rules() = { ... }` | `fn rules() =` followed by pipeline lines |
| `${model_name}` | `$model_name` |
| positional rule fields | named `pred`, `stage`/`event`, `narr`, `dedupe` fields |
| unquoted complex atom predicate | quoted predicate string or `$binding` |
| include-only protocol entry | complete self-contained canonical entry |

`window(duration_ms: ..., lateness_ms: ...)` remains parenthesized in canonical
source because it is the single structured call form.

## Legacy Flat DSL

Convert flat entries such as:

```text
template=demo
window=default_5s
fragment=udp_packet_meta_fragment
operation=datagram_exchange
```

to:

```text
fn demo_rules() =
  |> fragment :udp_packet_meta_fragment
  |> operation :datagram_exchange

template :demo
|> window :default_5s
|> use :demo_rules
```

Then compile and inspect findings. Do not perform a text-only migration for
rules: predicates, signals, narratives, and fragment coverage must be
validated semantically.

## Required Validation

```bash
cargo run -p gewyc -- envelope path/to/main.gewy --json
cargo run -p gewyc -- findings path/to/main.gewy --json
cargo run -p gewyc -- frontend path/to/main.gewy --focus graph
```

For repository-maintained sources, also run:

```bash
cargo test --test gewylang_docs_tdd
cargo run --bin gewyvern_validate -- registry
```
