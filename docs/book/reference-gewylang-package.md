# Reference: gewylang Package And Module Model

Use this page when you need exact lookup for the current package/module shape
used by `gewylang`.

This is not a tutorial. For a first guided walkthrough, start with
[docs/book/tutorial-gewylang-package.md](tutorial-gewylang-package.md).

## Scope

This page covers:

- `gewy.pkg`
- `main.gewy`
- `include(...)`
- function units
- `use(...)`
- the current safe package/module subset

For the broader language surface, see
[docs/dsl.md](../dsl.md).

For the syntax-first companion shelf, see
[docs/dsl-syntax.md](../dsl-syntax.md).
For the normative maintained-source contract, see
[docs/gewylang-style.md](../gewylang-style.md).
For the exact DSL vocabulary shelf, see
[docs/dsl-reference.md](../dsl-reference.md).
Legacy input belongs only in
[docs/gewylang-migration.md](../gewylang-migration.md).

## Package Shape

The current preferred package shape is:

1. one package root directory
2. one `gewy.pkg` manifest
3. one `main.gewy` entry
4. optional included helper modules

The package is compiled as one merged binding surface. It is not a collection
of independently executed runtime modules.

## Required Files

### `gewy.pkg`

Purpose:

- declares the package root
- points to the main entry
- provides dependency and source resolution context

Practical role:

- package resolution boundary for `gewyc`
- stable root for `include(...)` and package-relative loading

The `entry` value must be a normalized relative path to a regular file inside
the package root. It cannot be absolute, contain `.` or `..` components, or
traverse symlinks. Explicit protocol catalog scans report these violations
directly instead of silently treating a malformed package as absent.

### `main.gewy`

Purpose:

- top-level pipeline entry for the package
- final source file compiled to the binding

Recommended shape:

- one template id
- one small pipeline
- one focused operation/program model story

## `include(...)`

`include(...)` expands another `.gewy` source into the package frontend merge.

Current intended uses:

- split helper functions out of `main.gewy`
- keep one package readable
- reuse local module content without turning the package into many separate
  runtime entrypoints

### Local Include Form

Example:

```text
|> include "./network_rules.gewy"
```

Meaning:

- resolve a local file from the package/source root

### Package Dependency Include Form

Example:

```text
|> include "shared_udp:helpers/network_rules.gewy"
```

Meaning:

- resolve through a named package dependency

The compiler now surfaces this provenance in frontend reports as
`include_sources`, including:

- original request
- resolved path
- include kind
- dependency name when present

## Function Units

Function units are the current reusable module form.

Canonical form:

```text
fn udp_process_rules(model_name, op_name = :datagram_exchange) =
  |> fragment :udp_packet_meta_fragment
  |> fragment :route_meta_fragment
  |> operation $op_name
  |> program_model $model_name
```

Current characteristics:

- expression/pipeline oriented
- pure composition helper
- no cross-file mutable global state
- merged into the final entry-level frontend module

## `use(...)`

`use(...)` invokes a function unit from the merged frontend module.

### Positional Arguments

```text
|> use :udp_process_rules, :udp_process_debug_model
```

### Tail Default Arguments

Function units may declare trailing defaults:

```text
fn udp_process_rules(model_name, op_name = :datagram_exchange) =
  |> operation $op_name
  |> program_model $model_name
```

Valid call:

```text
|> use :udp_process_rules, :udp_process_debug_model
```

### Named Arguments

Valid named call:

```text
|> use :udp_process_rules, model_name: :udp_process_debug_model, op_name: :custom_exchange
```

Valid mixed call:

```text
|> use :udp_process_rules, :udp_process_debug_model, op_name: :custom_exchange
```

### Current Call Rules

Allowed:

- positional arguments
- trailing default arguments
- named arguments
- positional first, then named

Rejected:

- unknown named arguments
- duplicate assignment of the same parameter
- positional arguments after named arguments
- non-trailing default parameters followed by required parameters

## Inferred Parameter Kinds

`gewylang` now infers a light parameter kind from use position and applies
hard validation for the most safety-relevant kinds. A function signature may
also declare a lightweight kind explicitly, such as `model_name: atom` or
`dedupe_flag: bool = true`.

Current inferred kinds include:

- `atom`
- `bool`
- `u64`
- `predicate`
- `narrative`
- `stage`
- `key_event`
- `phase`

### Validation Intent

This is not a full static type system.

The current goal is narrower:

- stop common high-risk miscalls at `use(...)`
- make reusable function units safer
- keep the stable subset small and predictable

### Current Validation Boundary

#### `atom`

Must look like a stable identifier-shaped atom.

#### `bool`

Must be a boolean value.

#### `u64`

Must be a non-negative integer value accepted by the current numeric parser.

#### `predicate`

Must parse as a real predicate accepted by the current predicate parser.

#### `narrative`

Must be:

- a built-in narrative template, or
- an explicit `static:...` value

#### `stage`

Must match the current stable stage names used by rule construction.

#### `key_event`

Must match the current stable key-event names used by reason rules.

#### `phase`

Must be a stable lower-case `snake_case` phase name.

## Frontend Provenance Surfaces

When you inspect a package with:

```bash
cargo run -p gewyc -- frontend dsl/http_request_path.gewy --json
```

the most relevant package/module provenance fields are:

- `include_sources`
- `function_nodes`
- `use_edges`
- `graph_nodes`
- `graph_edges`

These are the easiest current reference surfaces for:

- where included sources came from
- which function units exist
- how the entry pipeline depends on them

## Current Safe Subset

The current preferred package/module subset is:

- one package
- one `main.gewy`
- local helper files through `include(...)`
- function reuse through `use(...)`
- no cross-file mutable state
- no attempt to turn package files into independently executed runtime modules

If a package design feels more complicated than that, it is usually a sign that
the package should be made smaller first.

## Stability Notes

Treat the following as the current durable package/module reference:

- `gewy.pkg` as the package root manifest
- `main.gewy` as the final entry source
- `include(...)` for source expansion
- function units plus `use(...)` as the main reuse mechanism
- the inferred parameter-kind validations listed above

Do not currently treat the exact frontend graph rendering text as frozen.
Prefer the documented JSON/provenance concepts instead.
