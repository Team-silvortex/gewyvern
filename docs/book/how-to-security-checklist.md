# How To Run The Security Checklist

This page is the short preflight for operators who want to run `gewyvern`
without guessing which exposure and resource-boundary questions matter first.

Use it before:

- turning on `--serve`
- exposing `--api-socket` to anything beyond localhost
- wiring an external engine
- pointing the packaged registry at custom roots
- treating a `0.19.x` deployment as stable enough for repeated automation

This is not a penetration-test guide.

It is a practical operator checklist for the active `1.4.6` runtime shape.

For the broader posture statement, see
[docs/security-posture.md](docs/security-posture.md).

For long-lived service expectations, see
[docs/service-behavior.md](docs/service-behavior.md).

## 1. Confirm The Deployment Shape

Before anything else, confirm that you are treating `gewyvern` as:

- a standalone runtime debugger
- a latest-snapshot analysis service
- a read-only nearby automation surface

Do not treat it as:

- a public multi-tenant API
- an authenticated control plane
- a durable history store
- a policy enforcement system

If your intended shape sounds more like the second list, stop here and put a
stronger system in front of `gewyvern` instead of assuming it already is one.

## 2. Check Ingest Intent

Ask these questions:

- are you using `demo`, `local-advisory`, or `remote-advisory` on purpose?
- are you keeping remote socket ingest explicitly opt-in?
- are you avoiding PID-trust claims from socket-fed sessions?

Current `1.4.6` expectation:

- socket-fed input is advisory-first
- remote TCP ingest should be a conscious decision, not a default
- `--pid` and socket ingest are intentionally not a trust-preserving mix

If you are relying on strong lineage from a socket producer, your deployment
assumption is stronger than the runtime contract.

## 3. Check API Exposure

Before enabling the API, confirm:

- `--api-socket` is only used together with `--serve`
- remote API bind is only used with explicit `--allow-remote-api`
- remote API bind is only used when a runtime admin token is configured
- localhost is the default unless you truly need broader reach
- callers understand the API is read-only and latest-snapshot only

Current `1.4.6` safety behavior:

- remote bind is rejected unless explicitly allowed
- remote bind is rejected unless a runtime admin token is also configured
- remote callers must present `X-Gewyvern-Admin-Token`
- restart clears the live in-memory snapshot
- the most recent served snapshot may still remain mirrored under the standard
  state root for operator inspection
- archived serve refreshes may also remain under the standard history root
- older archived refreshes may be pruned once the bounded retention window is exceeded
- overload may degrade into a short `503 service_busy` response
- oversized successful bodies may degrade into `503 response_too_large`

That means a client should be prepared for:

- `404` when no latest snapshot exists yet
- `503` when the API is overloaded
- `503` when a large report/export body exceeds the response budget

## 4. Check External Engine Wiring

If you use `--external-engine-bin`, verify:

- the engine binary path is intentional and operator-controlled
- the engine is treated as additive, not authoritative
- failure of the external engine does not break your core workflow
- the engine can tolerate timeouts and bounded output expectations

Current `1.4.6` behavior:

- built-in analysis runs first
- external augmentations are appended
- capability probing is bounded just like full analysis execution
- timeout and output caps are part of the expected contract

You should not depend on an external engine to redefine:

- ingest trust
- PID attribution strength
- core failure truth
- the built-in diagnosis spine

## 5. Check Registry And Package Roots

If you use custom protocol/package roots, verify:

- `GEWY_PROTOCOL_REGISTRY_ROOT` points at a directory you trust
- `GEWY_SHARE_ROOT` points at a directory you trust
- your package tree is intentionally small and reviewable
- you are not accidentally scanning a very large shared filesystem subtree

Current `1.4.6` behavior:

- symlink recursion is skipped
- repeated-directory loops are avoided
- directory count is budgeted
- manifest count is budgeted
- single-manifest size is budgeted

If your package source is huge, generated, or user-controlled, treat that as a
deployment smell even with the new scan budgets.

## 6. Check Resource Expectations

The current runtime already has several concrete guardrails.

Confirm that your operators know about them:

- socket ingest has per-line and total fact-count budgets
- socket reads are bounded by timeout
- Unix sockets are created with restricted permissions
- API reads and writes are bounded by timeout
- API client concurrency is bounded
- API large-body success responses are budgeted
- external engine execution is bounded by timeout and output caps

If your surrounding automation assumes “infinite stream, infinite body, infinite
wait,” that automation is not aligned with the current runtime contract.

## 7. Check Failure Semantics

Before rollout, verify that downstream readers understand:

- `degraded` does not mean “useless”
- advisory output does not mean “wrong”
- external failure does not erase built-in diagnosis
- missing latest snapshot data may be an honest bounded-state answer

This matters because the current runtime is conservative by design.

Many “surprising” results are actually the tool refusing to overclaim.

## 8. Check Automation Discipline

If a script or nearby service consumes `gewyvern`, confirm:

- it handles `404` and `503` without panicking
- it does not assume history is durable across restart
- it tolerates additive external-engine fields appearing in analysis JSON
- it does not treat the API as an auth boundary
- it does not treat one success response shape as permanently unbounded

Good automation posture is:

- read the latest snapshot
- accept bounded failure
- retry carefully
- preserve the distinction between core and additive signals

## 9. Run The Short Preflight

Use this exact short list before calling a deployment “safe enough” for the
current line:

1. verify ingest mode matches trust intent
2. verify API exposure is local by default or explicitly opted in
3. verify any remote API exposure also has an intentional runtime admin token
4. verify external engine paths are intentional and bounded
5. verify custom registry roots are trusted and scoped
6. verify automation handles `404`, `503`, and restart-cleared state
7. verify operators know the API is read-only and latest-snapshot only
8. verify dependency vulnerability checks and debugger cross-validation stay
   green before release-style automation

If all eight are true, you are aligned with the current `1.4.6` security shape.

## 10. Pair The Checklist With Validation

The checklist is about operator intent and deployment shape.

It should be paired with one real runtime validation pass before you trust a
new local or packaged setup.

The current first native check to run is:

- `cargo run --quiet --bin gewyvern_validate -- runtime-operator`

Pair it with the debugger cross-check before release-style automation:

- `cargo run --quiet --bin gewyvern_validate -- debugger-cross`

The legacy shell wrapper remains available for older automation:

- [scripts/validation/runtime_operator_validation.sh](scripts/validation/runtime_operator_validation.sh)

The runtime-operator check exercises the practical serve/API shell by checking
that:

- `--serve` keeps running across repeated sessions
- the latest snapshot is refreshed through the read-only API
- malformed ingest does not kill the service loop
- both TCP and UDP-oriented latest-snapshot paths still look sane
- the script prints a short coverage summary showing which checklist items were
  directly exercised and which still need operator confirmation

Use the checklist first when deciding whether a deployment shape is reasonable.

Use the validation script next when confirming that the runtime still behaves
that way in practice.

## 11. Know When To Escalate Beyond This Checklist

This checklist is no longer enough when you need:

- authenticated API consumers
- durable historical retention
- cross-instance coordination and audit
- policy enforcement
- public-facing service posture

At that point, `gewyvern` should become one bounded component inside a larger
system, not the whole system by itself.
