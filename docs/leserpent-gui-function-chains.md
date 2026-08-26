# Leserpent GUI Function Chains

This document defines how Leserpent distinguishes renderer capability from a
user-reachable product feature. The authoritative inventory is
[`project/release/leserpent-gui-function-chain.json`](../project/release/leserpent-gui-function-chain.json).
Its Rust validator lives in `src/gui_function_chain.rs`.

## Closure Rule

A GUI feature is closed only when every stage declared by its function chain
has source-anchored production evidence:

1. `entry`: a user can reach the control from the shipped product.
2. `semantic`: the control resolves to a typed, frontend-neutral intent.
3. `transport`: the intent crosses the authenticated bounded transport.
4. `authority`: the intended Rust authority, or the declared 1.x compatibility
   authority, executes it.
5. `persistence`: durable mutations retain state, audit, or a receipt.
6. `projection`: the result returns through a bounded safe projection.
7. `feedback`: the operator sees a terminal or explicitly pending outcome.
8. `automation`: the same operation is available through canonical Leselang
   when equivalence is part of that chain.

Renderer fixtures cannot satisfy `entry`, transport mocks cannot satisfy
`authority`, and a successful request without authoritative projection cannot
satisfy `feedback`.

## Coverage States

| State | Score | Meaning |
| --- | ---: | --- |
| `closed` | 100 | Every required stage has production evidence. |
| `partial` | 50 | A production path exists, but at least one required stage or operation is missing. |
| `conformance-only` | 25 | Protocol, fixture, or renderer proof exists without a product path. |
| `absent` | 0 | No implementation exists on the required surface. |

Target and bridge surfaces are reported separately. A closed 1.x Web bridge is
valuable compatibility evidence, but it does not make the Rust-hosted 2.0 Web
surface complete.

## 2026-08-26 Baseline

| Surface | Lifecycle | Score | Closed | Partial | Conformance only | Absent |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Avalonia desktop | target | 100 | 9 | 0 | 0 | 0 |
| Rust-hosted Web | target | 50 | 0 | 1 | 0 | 0 |
| ASP.NET Web | bridge | 100 | 5 | 0 | 0 | 0 |

The combined target score is 95. The closed Avalonia families are fleet
observation, runtime control, existing-runtime registration, daemon lifecycle,
Gewyvern lifecycle, authority health, Rust-authoritative Orchestra control, and
the suspended Leselang debugger workflow, plus live product Leselang
presentation automation. The native
Orchestra workspace now closes revision-fenced plan discovery, automatic run,
queued-only cancellation, lineage-bound retry, authenticated history/event
drilldown, and idempotent cleanup. Guided plans stay visibly review-only. The
registration editor separately closes already-running runtime intake and
revision-fenced metadata updates: the daemon produces a side-effect-free plan,
field edits invalidate it, and an explicit confirmation applies the same command
identity without sending deployment credentials or service secrets. The
remaining target gap is:

- native Rust compatibility mutations for registration, refresh, deletion,
  persistence, and Orchestra control, after which the ASP.NET bridge can retire.

`leserpentd` now serves the exact packaged TypeScript console from its existing
authenticated HTTPS listener. Public HTML, JavaScript, CSS, branding, and
language-pack requests reject credential-bearing variants; API requests require
the daemon Bearer token and reject disagreeing legacy and Authorization values.
The first-screen capabilities, fleet summary, attention, runtime, session, and
safe cleanup-preview reads project live `ControlRuntime` state without endpoint
credentials. A real TLS test proves the CSP-protected document, the 401 boundary,
and a persisted runtime reaching the browser-compatible camel-case projection.
This is a usable read-only product path, but remains `partial` until its mutation
families stop depending on the 1.x bridge.

The debugger workspace starts a bounded daemon-owned VM only to its first
effect, mounts the Rust-authored `UiDocument`, and routes its session-bound
cancel action through a strict .NET client. Cancellation is reviewed as a
side-effect-free dry-run, sealed to the issuing client and principal, before
explicit confirmation consumes the continuation and writes the VM audit. The
desktop New action stays fenced while that effect is waiting. Active-session
reconstruction after a daemon restart remains a VM-host resilience concern, not
part of this closed in-process observation-and-cancellation chain. Deadline
expiry still converges to a terminal revision and releases the 32-session
registry. Cancellation audit is restart-durable within a bounded 64-journal
retention horizon.

The debugger's **Run live** entry now starts that same Rust VM and receives only
the typed pending `PresentationOperation`; continuation state never crosses the
wire. Avalonia applies each atom to the current product window, reports a typed
applied or rejected outcome, and lets the Rust authority consume the continuation
and persist the re-entry result. The loop is capped at 64 effects, strict .NET
decoding binds effect ID, revision, node ID, and required capabilities, and an
adapter rejection converges to visible terminal failure instead of leaving a
suspended session. Wire, real TLS, strict-client, and native-control probes cover
the closed chain.

## Commands

Validate the status tensor and all GUI source anchors together:

```bash
cargo run --quiet --bin gewyvern_status -- validate
```

Inspect the GUI-only human or machine view:

```bash
cargo run --quiet --bin gewyvern_status -- gui
cargo run --quiet --bin gewyvern_status -- gui --json
```

Any new operator command, query, protocol request, product route, or GUI surface
must be added to the inventory and assigned to exactly one function chain. A
non-closed claim must retain an explicit gap; a closed claim must carry every
required stage anchor.
