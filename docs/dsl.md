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
- [dsl/pipeline_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy)
- [dsl/structured_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy)
- [dsl/dns_udp_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy)
- [dsl/https_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy)
- [dsl/http_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy)
- [dsl/http_server_response_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy)
- [dsl/http3_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy)
- [dsl/http3_server_response_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy)
- [dsl/hy2_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy)
- [dsl/hy2_tcp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy)
- [dsl/hy2_udp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy)
- [dsl/tls_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy)
- [dsl/quic_client_initial_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy)
- [dsl/quic_crypto_handshake_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy)
- [dsl/quic_stream_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy)
- [dsl/quic_bidi_stream_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy)
- [dsl/stun_binding_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy)
- [dsl/coap_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy)
- [dsl/ntp_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy)
- [dsl/dhcp_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy)
- [dsl/wireguard_handshake_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy)
- [dsl/mdns_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy)
- [dsl/ssdp_discovery_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy)
- [dsl/postgres_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy)
- [dsl/postgres_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy)
- [dsl/postgres_simple_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy)
- [dsl/postgres_query_error_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy)
- [dsl/mysql_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy)
- [dsl/mysql_simple_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy)
- [dsl/mysql_query_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy)
- [dsl/mysql_query_error_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy)
- [dsl/memcached_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy)
- [dsl/memcached_set_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy)
- [dsl/amqp_connection_start_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy)
- [dsl/amqp_basic_publish_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy)
- [dsl/amqp_publish_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy)
- [dsl/redis_ping_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy)
- [dsl/mqtt_connect_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy)
- [dsl/radius_access_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy)
- [dsl/gtpu_echo_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy)
- [dsl/smtp_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy)
- [dsl/sip_register_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy)
- [dsl/ldap_bind_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy)
- [dsl/ldap_search_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy)
- [dsl/ldap_modify_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy)
- [dsl/ldap_modify_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy)
- [dsl/ldap_modify_constraint_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy)
- [dsl/ldap_directory_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_session.gewy)
- [dsl/ldap_directory_write_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_write_session.gewy)
- [dsl/ldap_directory_sync_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy)
- [dsl/snmp_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy)
- [dsl/dns_tcp_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy)

## Current Shape

The preferred shape is now a pipeline-driven syntax inspired by Elixir. The
older structured block syntax and legacy `key=value` shape are both still
supported for compatibility and all existing protocol DSL files continue to
compile.

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

The pipeline parser now first merges files and function units into a single
pipeline module IR, then lowers that IR into the same compiler surface as the
structured and legacy forms; it does not generate eBPF bytecode directly.

For QUIC-family protocols, `quic_frame_observed` now accepts
`frame:crypto`, `frame:ack`, `frame:stream`, `frame:datagram`, and
`frame:connection_close`. It also accepts `byte_at:<offset>:<mask>:<value>`
and `bytes_at:<offset>:<byte>,<byte>,...`, which lets DSLs express both
stream-oriented and datagram-oriented QUIC modules without falling back to raw
UDP payload offsets.

That merged front-end IR is now also reflected in compiler-facing reports, so
`gewyc stages` can surface function counts, merged step counts, and resolved
`include(...)` sources for a package entry, along with a minimal structured
front-end graph whose nodes cover entry/file/function identities and whose
edges capture both `include()` and `use()` relationships, including the source
line that produced each edge.
Pipeline projects can also resolve through a `gewy.pkg` manifest with one
`main.gewy` entry and `include("...")` expansion.

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

Function units are declared with pure blocks:

```text
fn network_module() {
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> operation(:datagram_exchange)
}
```

They can also be parameterized:

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
- they may not define `template(...)`
- `include(...)` merges function definitions and steps into the single package
  entry compile path
- nested `use(:other_function)` composition is supported
- `use(:fn_name, ...)` supports positional arguments for parameterized function units
- there is no cross-file global variable state

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

## Structured Blocks

Structured blocks remain supported as a compatibility layer.

Top-level structured files start with:

```text
template <template_id> {
  ...
}
```

Supported top-level fields and blocks:

- `window <profile>` or `window.duration_ms <n>` plus `window.lateness_ms <n>`
- `reason <built_in_reason_profile>`
- `fragment <fragment_id>` or a `fragments { ... }` block
- `param <fragment>.<key> <value>`
- `evidence <fact_kind>:<tier>`
- `program_model <id> { ... }`
- `reason_model <id> { ... }`

Structured program rules use named fields instead of positional semicolons:

```text
program_model example_model {
  operation datagram_exchange

  rule {
    predicate datagram_observed:udp:local_to_remote
    stage datagram_observed
    narrative udp_datagram_sent
    dedupe true
    module example_module
    phase send_request
  }
}
```

Structured reason rules use the same predicate vocabulary but name the reason
key event explicitly:

```text
reason_model example_reason {
  rule {
    predicate datagram_observed:udp:local_to_remote
    key_event udp_datagram_seen
    narrative udp_datagram_sent
    dedupe true
    module example_module
    phase send_request
  }
}
```

The `predicate`, `stage`/`key_event`, `narrative`, and `dedupe` fields are
required inside each structured rule. `module` and `phase` remain optional, but
`phase` requires `module` so transition findings can stay module-scoped.

## Legacy Key/Value Shape

Each non-empty, non-comment line in the legacy form is a single `key=value`
pair. This remains fully supported so older `.gewy` files do not need to move
all at once.

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

Compile and run a DSL-driven demo:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --summary-only
```

Compile a `.gewy` file without starting the runtime:

```bash
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --out /tmp/udp-process-binding.json
```

Inspect the full compiler envelope through the shared front-end surface:

```bash
cargo run -p gewyc -- envelope /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- envelope /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit envelope --json --out /tmp/udp-process-envelope.json
```

Inspect binding diagnostics through the dedicated compiler surface:

```bash
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit diagnostics --json --out /tmp/udp-process-diagnostics.json
```

Inspect compiler findings through the dedicated compiler surface:

```bash
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
```

Inspect staged compiler output through the dedicated compiler surface:

```bash
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
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
- UDP-family protocol recognition is still based on compact flow evidence
  fingerprints, not full parser completeness
- `gewyc` is currently a separate workspace crate that still reuses
  `gewyvern`'s shared DSL/compiler library surface

## Related Files

- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
- [src/template.rs](/Users/Shared/chroot/dev/gewyvern/src/template.rs)
- [src/program.rs](/Users/Shared/chroot/dev/gewyvern/src/program.rs)
- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
- [tests/dsl_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/dsl_tdd.rs)
