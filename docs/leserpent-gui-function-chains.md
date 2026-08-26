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
| Avalonia desktop | target | 86 | 7 | 1 | 1 | 0 |
| Rust-hosted Web | target | 0 | 0 | 0 | 0 | 1 |
| ASP.NET Web | bridge | 100 | 5 | 0 | 0 | 0 |

The combined target score is 78. The closed Avalonia families are fleet
observation, runtime control, existing-runtime registration, daemon lifecycle,
Gewyvern lifecycle, authority health, and Rust-authoritative Orchestra control. The native
Orchestra workspace now closes revision-fenced plan discovery, automatic run,
queued-only cancellation, lineage-bound retry, authenticated history/event
drilldown, and idempotent cleanup. Guided plans stay visibly review-only. The
registration editor separately closes already-running runtime intake and
revision-fenced metadata updates: the daemon produces a side-effect-free plan,
field edits invalidate it, and an explicit confirmation applies the same command
identity without sending deployment credentials or service secrets. The remaining
target gaps are:

- a product VM-session projection and debugger cancellation bridge;
- a product Leselang VM host for live GUI presentation automation;
- a Rust-owned per-daemon TypeScript Web console and compatibility API.

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
