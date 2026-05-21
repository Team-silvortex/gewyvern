# Walkthrough

This walkthrough follows one concrete path through `gewyvern`:

```text
.gewy file
  -> TemplateBinding
  -> planner diagnostics
  -> runtime session
  -> export/debug summary
```

It uses the built-in UDP process-aware example:

- [dsl/udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy)

## Step 1: Start With A `.gewy` File

The example DSL file is:

```text
fn udp_process_rules() =
  let transport_predicate = "datagram_observed:udp"
  let route_narrative = "static:program resolved a route for this network flow"
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
  |> program_rule(predicate: ${transport_predicate}, stage: :datagram_observed, narrative: "static:program emitted or received a UDP datagram", dedupe: true)
  |> program_rule(predicate: :route_resolved, stage: :route_resolved, narrative: ${route_narrative}, dedupe: true)

template(:udp_process_debug)
|> window(duration_ms: 5000, lateness_ms: 200)
|> reason(:udp_datagram_l1)
|> use(:udp_process_rules)
|> param(:sock_lineage_fragment.capture_comm, true)
```

What this means:

- use UDP packet evidence
- use route/path evidence
- use process lineage evidence
- materialize a `datagram_exchange` program flow
- keep process names visible in lineage facts

Important boundary:

- this file does not generate eBPF
- it selects and parameterizes existing fragment templates
- it does so through the preferred stable-subset pipeline surface rather than
  the legacy key/value form

## Step 2: Compile To A Binding

The `.gewy` file compiles into a `TemplateBinding`.

That binding contains:

- template id: `udp_process_debug`
- fragment set:
  - `udp_packet_meta_fragment`
  - `route_meta_fragment`
  - `sock_lineage_fragment`
- window:
  - `duration_ms=5000`
  - `lateness_ms=200`
- reason profile: `udp_datagram_l1`
- program operation: `datagram_exchange`
- fragment params:
  - `sock_lineage_fragment.capture_comm=true`

In code, this boundary lives in:

- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs)

## Step 3: Inspect Planner Diagnostics

Before starting a runtime session, you can inspect whether the selected fragment
set actually supports the declared rules:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --diagnostics --json
```

Current output:

```json
{"template_id":"udp_process_debug","fragments":["udp_packet_meta_fragment","route_meta_fragment","sock_lineage_fragment"],"program_model":{"model":"udp_process_debug_dsl_model","rules":[{"rule_index":0,"tier":"optional_enhancement","supported":true,"required_facts":["sock_lineage"],"supporting_fragments":["sock_lineage_fragment"],"missing_facts":[]},{"rule_index":1,"tier":"core_requirement","supported":true,"required_facts":["packet_meta"],"supporting_fragments":["udp_packet_meta_fragment"],"missing_facts":[]},{"rule_index":2,"tier":"core_requirement","supported":true,"required_facts":["route_decision"],"supporting_fragments":["route_meta_fragment"],"missing_facts":[]}]},"reason_model":null}
```

How to read that:

- rule `0` depends on `sock_lineage`
- that evidence comes from `sock_lineage_fragment`
- it is currently treated as an `optional_enhancement`
- rule `1` depends on `packet_meta`
- rule `2` depends on `route_decision`
- those two are `core_requirement` evidence in this binding

This is the first place where the system proves that IR declarations are
grounded in real fragment inventory rather than wishful rules.

## Step 4: Start A Runtime Session

Once the binding is accepted, the runtime does five important things:

1. builds an `AttachPlan`
2. collects attach outcomes
3. gates ingest using those outcomes
4. materializes transport flows
5. lifts evidence into program flows and reason chains

This path lives across:

- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [src/loader.rs](/Users/Shared/chroot/dev/gewyvern/src/loader.rs)
- [src/runtime.rs](/Users/Shared/chroot/dev/gewyvern/src/runtime.rs)
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs)
- [src/reason.rs](/Users/Shared/chroot/dev/gewyvern/src/reason.rs)

For the built-in CLI demo path, the runtime feeds a small deterministic fact set
through this binding and then exports the session.

## Step 5: Look At The Runtime Result

You can ask the CLI for the lightweight session summary:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --summary-only
```

Current output:

```json
{"demo":"dsl_demo","template_id":"udp_process_debug","fragments_loaded":3,"hookpoints_failed":0,"accepted_facts":3,"rejected_facts":0,"flows":1,"reasons":1,"degraded":false}
```

That tells you:

- all three selected fragments loaded
- no hookpoints failed
- three facts were accepted
- nothing was rejected
- one transport flow was reconstructed
- one reason chain was produced
- the session was not degraded

For more operator-facing runs, modern `summary_json` output also carries:

- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`

Those fields are more useful when the session is diagnosing a real failure than
this tiny healthy UDP baseline. See
[docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md) for
practical report-reading examples.

## Step 6: Understand What Was Materialized

Under that summary, the runtime has already constructed several different views:

### Transport Flow

Evidence-layer view of one network flow:

- packet evidence
- route evidence
- optional process binding

### Program Flow

Higher-level view of what the program was doing:

- operation: `datagram_exchange`
- stages:
  - process bound
  - UDP datagram observed
  - route resolved

### Reason Chain

Deterministic explanatory view:

- UDP datagram seen
- route change/resolution
- narrative lines derived from the current reason profile

## Step 7: Export And Replay

The session can also be exported as full JSON:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

That full export preserves:

- attach plan
- attach report
- binding diagnostics
- fragment params
- evidence overrides
- facts
- rejected-fact audit
- flows
- program flows
- reasons

Replay uses that bundle to reconstruct the same debugger-visible materialized
state, not just the raw fact list.

## Why This Walkthrough Matters

This example shows the intended system boundary for the current line:

- DSL chooses a runtime shape
- fragment templates provide the actual observable capabilities
- planning proves the requested rules are supportable
- runtime gates and materializes evidence
- export preserves the resulting debugger state

That is the foundation for the future protocol-agnostic direction: new
protocols should arrive primarily as new fragment templates, params, and DSL
rule combinations, not as a DSL that directly emits new kernel bytecode.
