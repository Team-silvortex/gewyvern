# Export Format

Use this page when you need the current `ExportBundle` contract shape.

This page is intentionally a narrow reference for:

- export bundle fields
- replay-relevant semantics
- stable top-level export structure

This page is not the best first stop for:

- the runtime diagnosis spine
- machine-facing latest API fields
- a first explanation of how `gewyvern` reasons about failures

For those, use:

- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/system.md](docs/system.md)

The export format is designed for deterministic replay, not for generic
analytics ingestion.

## Goal

The export JSON must contain enough information to recompute L1 reason results
offline and preserve the materialized runtime view seen by the debugger.

At minimum this means exporting:

- all physical facts
- fragment inventory
- attach plan
- attach report
- window parameters
- reason profile id
- reason profile model
- materialized transport/program flow state

## Top-Level Shape

The current top-level object contains:

- `template_id`
- `fragment_inventory`
- `attach_plan`
- `attach_report`
- `binding_diagnostics`
- `attach_failure_summary`
- `debug_summary`
- `window_profile`
- `reason_profile_id`
- `reason_profile`
- `fragment_params`
- `evidence_overrides`
- `facts`
- `rejected_facts`
- `rejected_fact_summary`
- `flows`
- `program_flows`
- `reasons`

## Field Notes

### `template_id`

String id of the template used to start the session.

Example:

```json
"template_id": "handshake_debug"
```

### `fragment_inventory`

Ordered list of fragments included in the session.

Each item contains:

- `id`
- `version`

Example:

```json
{
  "id": "tcp_state_fragment",
  "version": 1
}
```

### `attach_plan`

Read-only runtime IR snapshot for the session.

It contains:

- `fragments`
- `hook_graph`
- `fact_graph`
- `dependency_graph`
- `coverage`

#### `attach_plan.fragments`

Each fragment descriptor currently exports:

- `id`
- `version`
- `hookpoints`
- `emits`
- `requires`
- `maps`
- `capabilities`

#### `attach_plan.coverage`

Coverage is exported as:

- `required`
- `covered`
- `missing`

This is used to prove that fragment requirements were satisfied at plan time.

### `attach_report`

Operational summary produced from the plan.

It contains:

- `fragments_loaded`
- `hookpoints_attached`
- `hookpoints_failed`
- `required_fact_kinds_coverage`
- `ringbuf_stats`

`hookpoints_failed` is part of the stable shape because attach outcomes are now
first-class runtime inputs. Loader results can affect both debug output and
fact-ingest gating.

### `binding_diagnostics`

Planner/debug view for the compiled binding.

It currently exports diagnostics for:

- `program_model`
- `reason_model`

Each model diagnostic contains per-rule entries with:

- `rule_index`
- `tier`
- `required_facts`
- `supporting_fragments`
- `missing_facts`
- `supported`

Current `tier` values:

- `core_requirement`
- `optional_enhancement`
- `unsupported`

This field exists to explain why a binding was accepted, degraded, or rejected
against the current fragment inventory.

### `attach_failure_summary`

Aggregated debug view over `attach_report.hookpoints_failed`.

Each item groups failures by:

- `hookpoint_kind`

Current kind values:

- `tracepoint`
- `kprobe`
- `tc_ingress`
- `tc_egress`
- `unknown`

And exports:

- `count`

### `debug_summary`

Small operational overview intended for CLI and UI surfaces.

Current exported fields:

- `fragments_loaded`
- `hookpoints_failed`
- `accepted_facts`
- `rejected_facts`
- `flows`
- `program_flows`
- `reasons`
- `degraded`

`degraded` is `true` when the session saw loader failures or runtime fact
rejections, and `false` otherwise.

### `window_profile`

Current exported fields:

- `id`
- `duration_ms`
- `lateness_ms`

Example:

```json
{
  "id": "default_5s",
  "duration_ms": 5000,
  "lateness_ms": 200
}
```

### `reason_profile_id`

String id of the active reason profile.

Current built-in value:

- `handshake_l1`
- `udp_datagram_l1`

### `reason_profile`

Stable export of the active reason profile.

### `fragment_params`

Per-fragment parameter bindings compiled from the `.gewy` file or attached
directly to a `TemplateBinding`.

### `evidence_overrides`

Template-local rule-tier overrides keyed by fact kind.

Example:

```json
{
  "sock_lineage": "core_requirement",
  "packet_meta": "optional_enhancement"
}
```

These do not mutate fragment descriptors or eBPF behavior. They preserve how
the originating binding reinterpreted evidence priority for planner
diagnostics, and replay keeps them stable.

Built-in profiles currently serialize as a string id.

Declarative profiles serialize as an object containing:

- `id`
- `kind`
- `rules`

This field exists so export replay can preserve DSL-defined reason semantics,
not only built-in profile ids.

### `facts`

Physical fact envelopes in session order.

When a session has been frozen, only facts inside the active window
(`window end - duration_ms` through `window end`) plus the late-arrival cutoff
(`window end + lateness_ms`) are exported.

Each fact includes:

- `id`
- `ts_ms`
- `cpu`
- `ifindex`
- `session`
- `fragment_id`
- `kind`

The `kind` object contains a `tag` plus fact-specific fields.

Current fact tags:

- `tcp_state`
- `packet_meta`
- `route_decision`
- `sock_lineage`
- `drop_action`
- `attach_scope`

### `rejected_facts`

Audit list for facts that were observed by the runtime input path but rejected
before materialization.

Current exported fields:

- `id`
- `fragment_id`
- `reason`

Current reason values:

- `fragment_not_loaded`
- `filtered_by_fragment_param`
- `before_window_start`
- `after_lateness_cutoff`

Window-policy drops are loss-accounted rather than silent. Facts earlier than
the active window are reported as `before_window_start`; facts later than the
window's configured lateness allowance are reported as
`after_lateness_cutoff`. Neither class participates in reconstructed flows.

### `rejected_fact_summary`

Aggregated debug view over `rejected_facts`.

Each item groups drops by:

- `fragment_id`
- `reason`

And exports:

- `count`

This summary is redundant with the raw audit list, but it makes attach/debug
triage much easier because callers can immediately see which fragment is being
dropped most often without scanning every rejected fact.

### `flows`

Materialized transport flow snapshots generated by runtime reconstruction.

Each flow currently exports:

- `id`
- `lifecycle`
- `path`
- `process`
- `evidence`
- `confidence`
- `fragment_sources`

`fragment_sources` is important for auditability. It records which fragments
contributed evidence to the flow.

### `program_flows`

Higher-level flows intended to model the program's network functionality rather
than only transport-level activity.

Each program flow currently exports:

- `id`
- `process`
- `operation`
- `transport_flows`
- `stages`
- `narrative`

Current built-in `operation` values are:

- `connect_flow`
- `datagram_exchange`
- `unknown`

Templates may also export custom operation ids. Callers should therefore treat
`operation` as an extensible string surface rather than a closed public enum.

This layer is driven by the template's embedded `program_model`, which is the
current IR-like rule surface for reconstructing program behavior from fragment
evidence.

### `reasons`

Materialized reason chains generated from facts and flows.

Each reason currently exports:

- `id`
- `flow`
- `l0_facts`
- `l1`
- `l3`

The replay contract is that `reasons` must be recomputable from exported facts
and metadata.

## Replay Contract

Replay is valid only if:

1. export JSON can be parsed
2. the referenced reason profile exists
3. the referenced fragment ids exist in the local registry
4. replay can rebuild a compatible template/program model pairing
5. replayed facts produce the same materialized result

In the current code, replay works by:

1. rebuilding a template from export metadata
2. starting a fresh `RuntimeSession`
3. re-ingesting every exported fact
4. restoring rejected-fact audit state
5. re-exporting the reconstructed state

The replay implementation must preserve:

- `flows`
- `program_flows`
- `reasons`
- `debug_summary`
- rejected-fact audit semantics

## Stability Notes

This format is still project-internal and versioned by repository evolution, not
by a public schema registry.

For now, when the export format changes:

1. add or update a test first
2. update this document
3. keep deterministic replay intact

## Companion References

- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
  Exact meanings for the runtime diagnosis spine that sits above exported
  facts and flow state.
- [docs/machine-contract.md](docs/machine-contract.md)
  Machine-facing latest API and additive sidecar context contract candidate.
- [docs/surface-stability.md](docs/surface-stability.md)
  Stable versus intentionally evolving surfaces across CLI, API, and export.
- [docs/system.md](docs/system.md)
  Broader compiler/runtime/export layering.
