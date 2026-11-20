# Source Module Boundaries

This page is the durable source-layering note for `gewyvern`.

It describes the current `src/` layering as the project converges on a more
stable release line.

For the complete workspace, clients and Leselang extraction boundary, start at
the [repository map](monorepo-stack.md). This page describes Gewyvern's product
runtime; it is not the owner map for every language crate.

The goal is simple:

- keep `main.rs` as an entry and orchestration layer
- keep reporting and diagnosis policy out of the CLI entrypoint
- keep long-lived socket service behavior isolated from short-lived CLI runs
- keep shared formatting helpers small and reusable

This is not a public stability guarantee yet, but it is the intended internal
shape we want to preserve while the project matures.

## Companion Shelves

Read this page alongside:

- [docs/architecture-blueprint-modules.md](docs/architecture-blueprint-modules.md)
  for the source-cluster dependency picture
- [docs/system.md](docs/system.md)
  for the broader layered system map
- [docs/architecture.md](docs/architecture.md)
  for runtime semantics rather than file placement

## Top-Level Shape

The current top-level runtime split is:

- `src/main.rs`
  CLI parsing, mode selection, top-level execution orchestration, and wiring
  between the major runtime subsystems.
- `src/external_analysis.rs`
  Runtime hook for append-only external analysis engines, bounded process
  execution, augmentation parsing, and integration into analysis snapshots.
- `src/data_api.rs`
  The latest-snapshot read-only API service, target route handling, target path
  encoding/decoding, and HTTP response generation for operator-facing data
  access.
- `src/serve_runtime.rs`
  Long-lived `--serve` lifecycle handling, socket session loops, scan/session
  branching, and output/API snapshot refresh orchestration.
- `src/report_runtime.rs`
  Summary, findings, scan report, HTML report, and HTTP transaction rendering.
- `src/diagnosis_runtime.rs`
  Process profile synthesis, failure semantics, competing hypotheses, and
  target-level primary diagnosis derivation.
- `src/render_utils.rs`
  Small shared formatting/render helpers used across reporting and API layers.

Alongside those entry-facing modules, the core modeling and runtime substrate is
still centered on:

- `src/dsl.rs`
  Gewyvern semantic host, materialization and compatibility entrypoints for
  GewyLang. Generic parsing/package loading belongs to `gewylang-syntax`;
  expansion and host-generic lowering belong to `gewylang-compiler`.
- `src/ir.rs`
  Product runtime predicates, signals, phase kinds, observation scope,
  transport matching, and required fact dependencies. Stable compiler-stage
  Binding/Analysis IR exchange belongs to `gewylang-ir`, not this module.
- `src/runtime.rs`
  Runtime event interpretation, finding generation, and flow-state evaluation.
- `src/fragment.rs`
  Fragment capability modeling and diagnostics support.
- `src/export.rs`
  Export/import JSON contracts for facts, flows, findings, and report data.
- `src/flow.rs`
  Flow-level abstractions, module-kind mapping, and stage identity helpers.
- `src/program.rs`
  Program-flow assembly and stage materialization.
- `src/protocol_profiles.rs`
  Protocol registry discovery, alias handling, and built-in protocol selection.
- `src/socket_input.rs`
  Advisory socket ingest, bounded fact collection, and listener setup helpers.
- `src/packet_capture.rs`
  Product-owned bounded UDP observation-to-fact conversion for IPv4/IPv6.
  It validates normalized framing and observed-prefix bounds, not probe
  attachment, raw IP parsing, checksums or producer authentication.
- `src/live_capture.rs`, `src/live_capture/linux/`, `ebpf/production/udp.bpf.c`
  Product-owned cgroup UDP collector, bounded ABI, libbpf ownership, loss
  accounting and detach/drain lifecycle. It has no validation-harness dependency.
- `src/main/capture_cli.rs`, `src/main/capture_runtime.rs`
  Explicit CLI admission and independent service-window consumption. Existing
  `serve_runtime` owns API/output publication; `export/capture_codec.rs` owns
  optional strict capture metadata and replay preservation.
- `crates/gewyvern-install-contract/src/capture.rs`, `src/gewyvern_install/capture.rs`
  Typed opt-in installation plan and the installed TLS service adapter. The
  adapter reuses the CLI capture loop rather than owning another collector;
  generation binding, private roots and matching-live-capture readiness remain
  installer responsibilities. Leserpent plan selection is a separate adapter.
- `src/http.rs`
  HTTP/HTTP3 transaction composition helpers.

## Boundary Rules

### Field Validation Is A Consumer

`linux_tcp_capture/linux/udp_dns.rs` shares one private CoreDNS target/client
fixture across validation and production modes. Its `production.rs` child starts
the formal CLI and only checks the resulting export against independent client
bytes. It neither imports the validation BPF producer nor reconstructs missing
facts from the oracle. This is a consumer of product capture, not another
collector implementation.

`linux_tcp_capture/linux/product_udp.rs` owns the shared formal-CLI lifecycle,
full bounded packet-byte oracle and phase-to-client-evidence checks. DNS reuses
its lifecycle/packet check; SNMP, NTP and STUN add protocol-specific expectations
through small `production.rs` consumers. Target/client ownership stays in each
existing protocol fixture; no second product collector or shell driver is added.

`tests/production_capture_linux_tdd/systemd.rs` drives unchanged installed units
through the real host manager. Its `systemd_reference.rs` helper only keeps
manager exit metadata alive during assertions; libsystemd is a requirement of
this opt-in test, not a new product collector dependency or a second supervisor.

`src/validation_harness/` exercises the product runtime; production matching,
capture and replay must not import its fixtures as their semantic implementation.
The UDP field adapter calls `packet_capture::CapturedUdpDatagram::into_fact`;
socket-cookie/tuple/time-window reconciliation remains an independent fixture
check. `tests/ipv6_capture_ingest_tdd.rs` feeds fresh captured facts into a
separate product CLI process, with the default registry, over Unix, IPv4 TCP and
IPv6 TCP. That input-path proof must not be relabeled an automatically attached
or continuous production eBPF collector. `LinuxProbeLoader` probes hook
availability; it does not start such a collector.
The separate `live_capture::Collector` now supplies explicit product CLI and
`--serve` capture. `tests/production_capture_linux_tdd.rs` starts the real binary
and independently generates/checks traffic. See [the operator boundary](live-udp-capture.md);
this bounded UDP path does not imply general TCP capture or persistent sessions.
The Linux protocol breadth path has these owners:

| Module under `linux_tcp_capture/linux/` | Owns |
| --- | --- |
| `data_breadth.rs` | campaign ordering, capture attachment, selected sockets, final report |
| `data_breadth/catalog.rs` | immutable protocol identity, pinned images, named package expectations, campaign labels |
| `data_breadth/target.rs` | private target startup/readiness, uncaptured setup, publisher/container/network teardown |
| `data_breadth/evidence.rs` | independent raw-byte reconciliation, source-packet candidate checks, package replay assertions |
| `data_breadth/wire.rs` and protocol modules | bounded independent clients and response decoding |
| `docker_target.rs` | label-checked resource ownership and private IPv4 / IPv6-only ULA endpoint admission shared by Linux fixtures |
| `udp_capture.rs` and `../udp_evidence.rs` | shared bounded UDP socket/tuple capture, explicit address family and full address width, loss checks and packet-only replay; remote port is explicit |
| `udp_dns.rs` / `udp_snmp.rs` / `udp_ntp.rs` / `udp_stun.rs` and their submodules | protocol-specific targets, native client oracles and reconciliation; never product decoders; NTP/STUN target isolation lives in their `target.rs` modules |
| `packet_capture.rs`, `bounded_stream.rs` | capture scope and bounded response reconstruction |

`catalog.rs` is fixture policy, not the product protocol registry. Its package
source and path come from one literal; borrowed static cases avoid rebuilding
the table at every run. Frozen field proofs check campaign labels, case order,
selected-connection expectations, command counts and image identities.

The target owns publisher, container and network in that drop order. Explicit
success cleanup still verifies removal; error paths retain ownership-checked
RAII cleanup. Evidence verification cannot create Docker targets, and target
startup cannot decide whether a product candidate is correct. Protocol-specific
candidate checks deliberately remain independent from `src/ir/` matching so a
product bug cannot validate itself. Existing CLI modes and report schema stay
unchanged; historical proofs retain the source hashes from their original runs.
The [Linux refactor proof](fixtures/gewyvern_breadth_architecture_linux_x86_64_20261113.json)
records six fresh campaigns across all nine protocols and 37 package cases;
this is compatibility evidence, not a new protocol maturity or performance claim.

### Product Layering

The intended boundary rules are:

1. `main.rs` wires subsystems together; it should not own report formatting,
   diagnosis policy, or long-lived socket loops.
2. `serve_runtime.rs` may call into report and API layers, but it should not
   define diagnosis semantics itself.
3. `report_runtime.rs` may consume diagnosis summaries, but it should stay
   focused on rendering and output shaping rather than deriving runtime
   evidence.
4. `diagnosis_runtime.rs` owns interpretation policy such as
   `failure_mode/detail/confidence/basis`, ambiguity handling, competing
   hypotheses, and process-profile synthesis.
5. `external_analysis.rs` may append machine-facing augmentations, but it
   should not redefine the built-in diagnosis contract or become a second
   reporting layer.
6. `data_api.rs` exposes already-produced results; it should not become a
   second execution engine.
7. `render_utils.rs` should remain small, pure, and reusable. It is not a home
   for orchestration or policy logic.
8. `dsl.rs`, `ir.rs`, `runtime.rs`, `fragment.rs`, and `export.rs` remain the
   core compiler/runtime substrate and should stay usable outside the CLI entry
   path.

## Dependency Direction

At a high level, the intended dependency flow is:

```text
main
  -> external_analysis
  -> serve_runtime
  -> report_runtime
  -> diagnosis_runtime
  -> data_api

serve_runtime
  -> report_runtime
  -> data_api

report_runtime
  -> diagnosis_runtime
  -> render_utils

data_api
  -> render_utils

diagnosis_runtime
  -> external_analysis
  -> flow / export / runtime-facing data types
```

The core substrate remains below that layer:

```text
dsl -> template / program / reason / ir
runtime -> fragment / loader / program / reason / flow / ledger
program / reason -> ir -> flow / ledger
runtime <-> export (snapshot construction / deterministic replay)
```

This is intentionally approximate, not a formal module system. The important
part is the direction: entry and operator-facing layers should depend on core
runtime semantics, not the other way around.
The runtime/export replay relationship is still inside the root crate; these
source modules are not independent acyclic Cargo packages. Protocol matching
belongs below program/reason assembly, not above runtime orchestration.

## When To Touch Which File

As a quick rule of thumb:

- If the change is about CLI flags, mode selection, or top-level wiring, start
  in `src/main.rs`.
- If the change is about socket-service lifecycle or per-session orchestration,
  start in `src/serve_runtime.rs`.
- If the change is about JSON/text/HTML output shape, start in
  `src/report_runtime.rs`.
- If the change is about which diagnosis the user should see, start in
  `src/diagnosis_runtime.rs`.
- If the change is about read-only data export over the API port, start in
  `src/data_api.rs`.
- If the change is about calling a sibling analysis engine or merging external
  augmentations, start in `src/external_analysis.rs`.
- If the change is about shared formatting helpers, start in
  `src/render_utils.rs`.
- If the change is about the language, IR, fragments, facts, or runtime event
  semantics, distinguish the owner first: generic GewyLang syntax/compiler/IR
  changes belong in `crates/gewylang-*`; product semantic adapters belong in
  `src/dsl/` and `src/gewyc/`; runtime facts, matching and reconstruction belong
  in `src/ir.rs`, `src/fragment.rs`, `src/runtime.rs`, or `src/export.rs`.

## Near-Term Intent

The current intent is not another large redesign. It is to keep
this split stable while:

- reducing `main.rs` further when a clearly separable subsystem appears
- tightening subsystem contracts instead of re-merging logic into the entrypoint
- continuing to document operator-facing behavior and internal runtime
  boundaries in parallel

If a future refactor makes one of these files disappear, that is fine. The
important part is preserving the layering principles above.
