# Reference: Runtime Diagnosis Spine

Use this page when you need exact meanings for the core runtime diagnosis
fields that show up in:

- `--json --summary-only`
- `summary.json`
- `analysis.json`
- target-level latest API outputs

This page is intentionally narrow. It focuses on the diagnosis spine, not the
full rendered report.

For broader contract notes, see
[docs/surface-stability.md](/Users/Shared/chroot/dev/gewyvern/docs/surface-stability.md).
For process-oriented reading, see
[docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md).

## What Counts As The Diagnosis Spine

The current narrow diagnosis spine is centered on:

- `primary_module_kind`
- `primary_module_family`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `evidence_posture`
- `automation_outcome`
- `operator_guidance_status`
- `operator_guidance_action`
- `operator_guidance_reason`
- `operator_guidance_summary`
- `ambiguous`
- `competing_hypotheses`
- `operations`
- `phases`
- `missing_transitions`
- `suspect_areas`

These are the fields other operators, tools, and sidecar consumers should read
first.

## Identity And Shape

### `kind`

Current top-level object kind.

Typical values:

- `single`
- `scan`

Meaning:

- `single`: one target-level rendered result
- `scan`: multi-target sweep result

### `name`

Canonical name for the rendered object when one exists.

For scan outputs, per-target names appear inside target arrays.

## Primary Module And Failure

### `primary_module_kind`

The best current answer to:

- what network-module family does this target/process most resemble?

Typical examples:

- `name_resolution`
- `connection_establishment`
- `tls_handshake`
- `http_request_response`
- `database_query`
- `proxy_authentication`
- `proxy_tunnel_establishment`

This is often the first field a human operator should read.

### `primary_module_family`

Stable family-level band for the current primary module.

Typical examples:

- `request-response`
- `database`
- `auth`
- `messaging`

Meaning:

- the broad protocol/behavior family the current diagnosis best fits

### `primary_failure_stage`

The most suspicious transition-level stage boundary.

Typical shape:

- `resolve_upstream->connect`
- `send_request->receive_response`
- `send_query->receive_ok`

Meaning:

- the runtime thinks the problem is best summarized at this stage boundary

### `primary_failure_mode`

The broad failure family.

Typical values include:

- `setup_incomplete`
- `request_sent_no_reply`
- `denied`
- `semantic_error`

Meaning:

- this is the coarse operator-facing class of what went wrong

### `primary_failure_detail`

The more specific operational explanation within the current failure family.

Typical examples:

- `dns_unresolved`
- `route_or_connect_blocked`
- `auth_denied`
- `request_sent_no_reply`
- `handshake_incomplete`

Meaning:

- this is the concrete detail the runtime is willing to surface right now

## Confidence And Basis

### `primary_failure_confidence`

How hard the runtime is willing to lean on the current primary explanation.

Current values:

- `high`
- `medium`
- `low`

Practical reading:

- `high`
  usually means a direct protocol signal or explicit denial/error
- `medium`
  usually means a plausible missing transition or bounded inference
- `low`
  usually means the runtime is intentionally refusing to over-collapse

### `primary_failure_basis`

What kind of evidence gave rise to the current failure summary.

Typical values:

- `direct_protocol_signal`
- `missing_transition`
- `phase_level_inference`

Practical reading:

- `direct_protocol_signal`
  strongest basis; the protocol itself said enough
- `missing_transition`
  observed request/setup side, but not the next expected stage
- `phase_level_inference`
  weaker compression from broader runtime evidence

### `evidence_posture`

Machine-facing bucket for how the current diagnosis should be consumed.

Typical values:

- `direct_protocol_signal`
- `missing_transition`
- `ambiguous_multi_hypothesis`
- `unverified_ingest`
- `heuristic_summary`

Meaning:

- the shortest stable answer to how strong and how direct the current diagnosis
  really is

### `automation_outcome`

Machine-facing bucket for the safest next automation stance.

Typical values:

- `advisory_only`
- `collect_more_evidence`
- `multi_hypothesis`
- `manual_review`
- `targeted_escalation`

Meaning:

- the shortest stable answer to what an automation layer should do with this
  diagnosis next

## Operator Guidance

### `operator_guidance_status`

The runtime's current stance on whether it has enough evidence to guide action.

Treat it as the top-level state for built-in next-step advice.

### `operator_guidance_action`

The safest built-in next action.

Common values include:

- `observe_more`
- `manual_review`
- `collect_more_runtime_evidence`
- `keep_multiple_hypotheses`
- `avoid_pid_strong_actions`

Meaning:

- what the runtime thinks the operator or automation should do next

### `operator_guidance_reason`

Short machine-facing explanation for why the current action was chosen.

Use this when you need more than the action label but less than a long report.

### `operator_guidance_summary`

Human-oriented one-line explanation of the current guidance.

This is the safest summary field for operator surfaces that do not want to
reconstruct guidance from lower-level details.

## Ambiguity

### `ambiguous`

Boolean signal that the runtime is explicitly keeping multiple plausible
stories alive.

Meaning:

- do not read the primary module/failure as the only story

When `ambiguous=true`, conservative handling is usually the correct behavior.

### `competing_hypotheses`

List of other still-live module/failure possibilities.

## Rolled-Up Protocol Context

### `operations`

Top-level rolled-up operation names for the current diagnosis spine.

Typical examples:

- `http_request_path`
- `postgres_query_path`
- `redis_get_path`

Meaning:

- what higher-level protocol path names were active in the current diagnosis

### `phases`

Top-level rolled-up observed phases for the current diagnosis spine.

Typical examples:

- `connect`
- `send_request`
- `receive_response`

Meaning:

- how far the best current process/path progressed before it became healthy,
  attention-worthy, or inconclusive

Use this to answer:

- what else is still plausible?
- why did the runtime choose `low` confidence?

This field matters most in:

- mixed-flow results
- proxy-heavy paths
- DNS/TLS/HTTP chains
- partial setup / partial response scenarios

### `missing_transitions`

Top-level rolled-up transition gaps from the current primary process profile or,
when no primary process rollup exists, from the observed protocol-flow set.

Use this to answer:

- which expected edges are currently missing?
- where is the strongest transition-level gap without traversing nested views?

### `suspect_areas`

Top-level rolled-up operational pressure areas behind the current diagnosis.

Typical values include:

- `transport_io`
- `authentication`
- `application_protocol`

Use this to answer:

- what class of runtime pressure is most implicated right now?

## Supporting Runtime Views

These are not the narrow spine itself, but they are the most important
supporting surfaces.

### `process_network_profiles`

Process-oriented compression layer over matched protocol flows.

Use when the question is:

- where is this process stuck?
- what network module does this process look like?

### `protocol_flows`

Rawer flow-level support for the diagnosis spine.

Use when the question is:

- what concrete protocol-shaped paths matched?
- which transitions were healthy, missing, or denied?

### `augmentations`

Append-only enrichment slot for built-in advisories and external sidecars.

Do not treat it as the built-in diagnosis spine itself.
Use it as a supporting context surface.

## Reading Order

When scanning one result quickly, read in this order:

1. `primary_module_kind`
2. `primary_failure_mode`
3. `primary_failure_detail`
4. `primary_failure_confidence`
5. `primary_failure_basis`
6. `ambiguous`
7. `competing_hypotheses`
8. `operator_guidance_action`

That sequence usually gives the fastest correct mental model.

## Stability Notes

Treat the fields listed in “What Counts As The Diagnosis Spine” as the current
reference surface for runtime diagnosis.

Do not treat these as equally stable:

- exact HTML wording
- incidental report layout
- every nested report-only field in `report.json`
- arbitrary augmentation payload internals

Prefer the diagnosis spine first, then the supporting runtime views above.
