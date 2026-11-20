# DSL Reference Surface

Use this page when you need exact lookup for the stable compatibility and
reference surface of `gewylang`: legacy keys, predicate vocabulary, narrative
ids, fragment parameter schema, and safety boundaries.

This page pairs with:

- [docs/dsl.md](dsl.md)
- [docs/dsl-syntax.md](dsl-syntax.md)
- [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)

## Legacy Key Surface

The current preferred `gewylang` surface is the pipeline DSL. The flat
top-level key form remains supported as a compatibility surface for fixtures,
migration work, and older bindings.

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

Optional trailing `module` and `phase` fields are also accepted:

```text
reason.rule=route_resolved;route_changed;route_changed;true;postgres_connect_path;resolve
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

If `program_model` is omitted and you provide `operation` or `rule` lines, the
compiler synthesizes `<template>_dsl_model`.

If `program_model`, `operation`, and `rule` are all omitted, the compiler
falls back to the default program model for the selected `reason` profile.

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

Optional trailing fields:

- `module`
- `phase`

`datagram_observed` supports direction and qualifier suffixes such as:

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

Example:

```text
rule=datagram_observed:udp:remote:snmp:local_to_remote:byte0_mask:0xff:0x30:byte_at:13:0xff:0xa0;datagram_observed;udp_datagram_sent;true
```

QUIC now has a parallel structured surface:

- `quic_packet_observed:remote:quic:local_to_remote:min_len:1200:long_header:true:type:initial`
- `quic_frame_observed:remote:quic:remote_to_local:type:handshake:frame:crypto`

Supported QUIC qualifiers include:

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

Accepted fact kinds are:

- `tcp_state`
- `packet_meta`
- `quic_meta`
- `route_decision`
- `sock_lineage`
- `drop_action`
- `attach_scope`

Accepted tiers are exactly `core_requirement` and `optional_enhancement`.

This changes planner classification priority, not the underlying fragment's
kernel behavior.

## Predicates

Current predicate families are:

- `process_bound`
- `socket_state_observed`
- `socket_state_observed:<port>`
- `socket_state_observed:local:<port>[:established]`
- `socket_state_observed:remote:<port>[:established]`
- `route_resolved`
- `datagram_observed:<proto>`
- `packet_observed:<proto>`
- `rtsp_response_observed:remote:<port|name>:<options_ok|describe_ok|setup_ok|play_ok>`
- `snmp_pdu_observed:remote:<port|name>:<get|get_next|get_bulk|response>`
- `ntp_message_observed:remote:<port|name>:<request|response|time_reply>`
- `stun_binding_observed:remote:<port|name>:<request|response|error>`
- `quic_packet_observed:...`
- `quic_frame_observed:...`
- `all(...)`
- `any(...)`

Examples:

```text
process_bound
socket_state_observed:remote:https:established
datagram_observed:udp
packet_observed:tcp:remote:https:local_to_remote
all(process_bound,datagram_observed:udp)
any(route_resolved,socket_state_observed)
```

This vocabulary is shared by both `rule=` program-flow rules and
`reason.rule=` declarative reason rules.

`stun_binding_observed` recognizes modern UDP Binding candidates within a complete,
contiguous 20..128-byte sampled envelope. It checks the exact Binding message type,
magic cookie, declared length, and padded attribute boundaries. `request` is egress;
`response` and `error` are ingress. Success requires a correctly shaped
XOR-MAPPED-ADDRESS (IPv4 or IPv6); error requires an ERROR-CODE with class 3..6
and number 0..99. Only the first occurrence of those attributes is used. Unknown
attributes and padding contents remain opaque. The collector must support offsets
`0..127`; sparse, truncated and oversized samples are not guessed. This does not
validate FINGERPRINT, message integrity, authentication, transaction correlation,
NAT traversal, ICE or TURN. See [RFC 8489](https://www.rfc-editor.org/rfc/rfc8489.html).

`ntp_message_observed` requires a contiguous 48-byte NTPv3/v4 base header
at payload offset zero on UDP. `request` selects egress mode 3; `response`
selects ingress mode 4, including unsynchronised and Kiss-o'-Death candidates.
`time_reply` additionally requires LI other than 3, stratum 1..15, and nonzero
originate/receive/transmit timestamp fields. It still does not establish request
correlation, accurate time, trusted origin or client clock discipline. Leap bits
do not change request intent; v3/v4 are versions, not query versus sync modes.
Required sample offsets are `0..47`; truncated/sparse samples are unsupported.
Extension/MAC bytes are outside this base-header predicate and are not validated
or authenticated. See [RFC 5905](https://www.rfc-editor.org/rfc/rfc5905).

`snmp_pdu_observed` recognizes bounded UDP community-message candidates, not
transaction outcomes. `get`/`get_next`/`get_bulk` require egress; `response`
requires ingress. SNMPv1/v2c use a variable-length BER community field, not a
fixed PDU byte offset; GETBULK requires v2c. The envelope, PDU fields and varbind
shapes must be contiguous and complete within 128 captured bytes. Definite
short and one/two-byte long lengths are supported, including non-minimal BER
lengths; indefinite lengths, gaps, truncation, trailing sampled bytes, larger
messages and v3 are rejected. Required sample offsets are `0..127`, with
unsupported-offset diagnostics for sparse collectors. A generic response can
contain error-status or exception values: it does not prove successful reads,
GET versus GETNEXT versus GETBULK correlation, authorization, or a full walk.
Only the read packages use this predicate; SET/notifications/v3 remain separate
unvalidated breadth work. Protocol shape follows [RFC 3416](https://www.rfc-editor.org/rfc/rfc3416)
and the community envelope in [RFC 1901](https://www.rfc-editor.org/rfc/rfc1901).

`rtsp_response_observed` is an ingress TCP response candidate, not a dialogue
decoder. It requires a complete RTSP/1.0 `200` header at packet offset zero,
ending within 512 bytes, with a decimal `CSeq`. Header names are case-insensitive
and order-independent; duplicate, folded, malformed or truncated headers are
rejected. It never scans the body or combines packets. Options requires a
`Public` method list, describe requires SDP content type and a positive content
length, setup requires `Session` plus RTP transport, and play requires `Session`
plus `RTP-Info`. Generic `200`, session-only and range-only replies are insufficient.
The predicate declares required payload offsets `0..511`; a sparse collector
reports unsupported offsets rather than claiming live support. A supplied
complete prefix can match a shorter header without requiring body bytes.
This does not correlate requests by CSeq, validate SDP bodies, prove media
delivery, or handle TLS, RTSP/2.0 or TCP stream reassembly.

`packet_observed` supports compact TCP or UDP payload fingerprints. The common
scope qualifiers shared by packet, datagram, and QUIC predicates are:

- `local:<port|name>` or alias `sport:<port|name>`
- `remote:<port|name>` or alias `dport:<port|name>`
- `local_to_remote`
- `remote_to_local`

Its payload qualifier surface includes:

- `local:<port|name>`
- `remote:<port|name>`
- `sport:<port|name>`
- `dport:<port|name>`
- `byte0_mask:<u8>:<u8>`
- `prefix4:<u32>`
- `byte4_mask:<u8>:<u8>`
- `byte13_mask:<u8>:<u8>`
- `byte_at:<offset>:<u8>:<u8>`
- `bytes_at:<offset>:<u8>,<u8>,...`

`quic_frame_observed` requires `frame:<type>` and accepts scope qualifiers,
`type:<packet-type>`, `byte_at`, and `bytes_at`. Use compiler findings rather
than guessing unsupported packet or frame names.

## Stages

Program `stage` and reason `event` values share the current signal vocabulary:

- `none`
- `process_bound`
- `socket_state_transition`
- `packet_observed`
- `datagram_observed`
- `route_resolved`
- `syn_seen`
- `udp_datagram_seen`
- `process_identified`
- `state_change`
- `route_changed`
- `fin_or_rst`

Program rules normally use the evidence-facing values such as
`process_bound`, `socket_state_transition`, `packet_observed`,
`datagram_observed`, and `route_resolved`. Reason rules normally use event
values such as `process_identified`, `state_change`, `packet_observed`,
`udp_datagram_seen`, and `route_changed`.

## Narrative Values

Current narrative forms are:

- `none`
- `process_bound`
- `packet_observed`
- `transport_payload_sent`
- `transport_payload_received`
- `tcp_state_transition`
- `route_changed`
- `udp_datagram_observed`
- `udp_datagram_sent`
- `udp_datagram_received`
- `static:<text>`

Examples:

```text
none
process_bound
udp_datagram_sent
static:program resolved a route for this network flow
```

DSL narrative templates do not add new kernel behavior. They only shape how
the runtime interprets facts emitted by the selected fragment templates.

## Dedupe

The fourth rule field is a boolean:

- `true`
- `false`

When `true`, the rule only contributes once per program flow.

## Fragment Parameter Schema

Fragment parameters are statically validated against fragment descriptor schema
at DSL compile time and again when building `SessionConfig`.

Current built-in parameters are:

- `tcp_packet_meta_fragment.sample_payload_offsets: string`
- `sock_lineage_fragment.capture_comm: bool`
- `udp_packet_meta_fragment.min_len: u64`
- `udp_packet_meta_fragment.sample_payload_offsets: string`

Examples:

```text
param=sock_lineage_fragment.capture_comm=false
param=udp_packet_meta_fragment.min_len=80
param=tcp_packet_meta_fragment.sample_payload_offsets=0,1,4,9
```

`sample_payload_offsets` is a comma-separated string of payload offsets. The
registry validates the value family; runtime diagnostics validate whether the
selected offsets cover the predicates used by the binding.

Invalid examples:

```text
param=sock_lineage_fragment.not_a_real_param=true
param=udp_packet_meta_fragment.min_len=false
```

## Current Limits

- the DSL is intentionally small
- it compiles into `TemplateBinding`, not into new fragment descriptors
- it does not generate eBPF bytecode
- window profiles and reason profiles are still selected from built-in ids
- narrative rendering is intentionally simple
- UDP-family protocol recognition is still based on compact flow-evidence
  fingerprints, not full parser completeness
- `gewyc` is currently a separate workspace crate that still reuses
  `gewyvern`'s shared DSL/compiler library surface

## Companion Shelves

When this page feels too exact, step back to:

- [docs/dsl.md](dsl.md)

When you need authoring structure instead of field-by-field lookup, move to:

- [docs/dsl-syntax.md](dsl-syntax.md)

When you need exact package/module semantics, move to:

- [docs/book/reference-gewylang-package.md](book/reference-gewylang-package.md)

When you need compiler/lowering truth, move to:

- [docs/book/reference-ir-lowering.md](book/reference-ir-lowering.md)
- [docs/gewyc-json.md](gewyc-json.md)
