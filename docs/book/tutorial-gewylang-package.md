# Tutorial: Your First gewylang Package

This tutorial is the shortest path from “I want to write a `.gewy` module” to
“I understand the current package shape and function composition model”.

It assumes:

- you already have the repository
- you can run `cargo`
- you want to learn the preferred stable subset, not every historical form

## What You Will Learn

By the end of this tutorial, you will have:

1. created a minimal gewy package
2. understood the role of `gewy.pkg`, `main.gewy`, and `include(...)`
3. written a small reusable function unit
4. compiled it with `gewyc`
5. inspected the frontend shape

## Step 1: Scaffold A Package

Use the dedicated compiler CLI:

```bash
cargo run -p gewyc -- init /tmp/my_gewy_app
```

That creates the current package skeleton:

- `gewy.pkg`
- `main.gewy`
- an included helper module

This is the current preferred package shape:

- one package
- one entry file
- reusable behavior factored into function units and included modules

## Step 2: Understand The Package Files

### `gewy.pkg`

This is the package manifest.

It gives the package a stable root and lets `gewyc` resolve:

- the main entry
- local dependencies
- named sources
- the lock snapshot

### `main.gewy`

This is the entry file that ultimately compiles to the binding.

The current mental model is:

- one package has one main entry
- other files support that entry
- the final compile target is the merged binding, not multiple independent
  runtime modules

## Step 3: Write One Reusable Function Unit

The preferred style is pipeline-oriented and functional.

Example:

```text
fn udp_process_rules(model_name, op_name = :datagram_exchange) =
  |> fragment :udp_packet_meta_fragment
  |> fragment :route_meta_fragment
  |> fragment :sock_lineage_fragment
  |> operation $op_name
  |> program_model $model_name
```

Important characteristics:

- function units are pure composition helpers
- there is no cross-file global mutable state
- defaults are allowed on trailing parameters

## Step 4: Use The Function In The Entry Pipeline

The entry file can then compose it:

```text
template :udp_process_debug
|> window :default_5s
|> reason :udp_datagram_l1
|> use :udp_process_rules, :udp_process_debug_model
|> param :sock_lineage_fragment.capture_comm, true
```

This is the modern `gewylang` shape:

- `template ...`
- piped calls
- `use(...)` for function reuse
- `include(...)` for file expansion

## Step 5: Compile The Package

Inspect the lowered result:

```bash
cargo run -p gewyc -- /tmp/my_gewy_app/main.gewy --json
```

If you want the shortest aggregated troubleshooting view first:

```bash
cargo run -p gewyc -- envelope /tmp/my_gewy_app/main.gewy --json
```

If you want a more frontend-oriented view:

```bash
cargo run -p gewyc -- frontend /tmp/my_gewy_app/main.gewy
```

Or as JSON:

```bash
cargo run -p gewyc -- frontend /tmp/my_gewy_app/main.gewy --json
```

This is one of the best ways to confirm that:

- includes resolved where you expected
- function units were found
- `use(...)` edges point where you think they do

For the current `1.2.0`-line compiler surfaces, a practical read order is:

1. check `envelope.payload.summary.finding_count`
2. read `envelope.payload.summary.next_step`
3. if needed, open `stages` or `findings`
4. only then drill into `frontend` or `binding` detail

## Step 6: Inspect The Frontend Graph

The frontend graph is the current easiest way to see module composition:

```bash
cargo run -p gewyc -- frontend /tmp/my_gewy_app/main.gewy --focus graph
```

Look for:

- `include_sources`
- `function_nodes`
- `use_edges`
- `graph_nodes`
- `graph_edges`

This is where the newer module provenance work starts to pay off: you can see
not just that something was used, but where it came from and how it was
expanded.

If compilation posture looks off, use the umbrella troubleshooting view:

```bash
cargo run -p gewyc -- explain /tmp/my_gewy_app/main.gewy --json
```

Start with:

- `payload.summary.stage_status`
- `payload.summary.next_step`

## Step 7: Know The Current Safe Subset

The preferred `gewylang` subset for the current line is intentionally narrow:

- pipeline-driven composition
- local immutable `let`
- function-unit reuse via `use(...)`
- package composition via `include(...)`
- no cross-file mutable global state

This is not a general-purpose programming language. It is a structured binding
language for selecting and parameterizing existing runtime capabilities.

## Step 8: Understand The Safety Angle

Recent module reuse improvements now also enforce a light parameter boundary.

Depending on usage position, `use(...)` calls now validate inferred kinds such
as:

- `atom`
- `bool`
- `u64`
- `predicate`
- `narrative`
- `stage`
- `key_event`
- `phase`

That means many high-risk mistakes are rejected at the DSL/compiler layer,
before they drift deeper into the runtime.

## Where To Go Next

- For the full language guide:
  [docs/dsl.md](../dsl.md)
- For JSON output details:
  [docs/gewyc-json.md](../gewyc-json.md)
- For the broader runtime story:
  [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)
