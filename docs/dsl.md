# DSL Guide

This document describes the current `.gewy` DSL used by `gewyvern`.

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

## File Extension

`gewyvern` DSL files use the `.gewy` extension.

Examples in this repository:

- [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy)
- [dsl/udp_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_debug.gewy)
- [dsl/udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy)
- [dsl/dns_udp_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy)
- [dsl/https_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy)

## Current Shape

Each non-empty, non-comment line is a single `key=value` pair.

Comments start with `#`.

Example:

```text
template=udp_process_debug
window.duration_ms=5000
window.lateness_ms=200
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
rule=datagram_observed:udp;datagram_observed;static:program emitted or received a UDP datagram;true
rule=route_resolved;route_resolved;static:program resolved a route for this network flow;true
param=sock_lineage_fragment.capture_comm=true
```

## Top-Level Keys

Current supported keys are:

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

Compile and run a DSL-driven demo:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --summary-only
```

Run a socket session from a DSL file:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --unix-socket /tmp/gewyvern.sock --json
```

## Current Limits

- The DSL is still intentionally small
- It compiles into `TemplateBinding`, not into new fragment descriptors
- It does not generate eBPF bytecode
- Window profiles and reason profiles are still selected from built-in ids
- Narrative rendering is still intentionally simple

## Related Files

- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs)
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs)
- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [tests/dsl_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/dsl_tdd.rs)
