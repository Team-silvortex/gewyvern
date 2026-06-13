# Security Posture

This note captures the practical security boundary of `gewyvern` as it exists
today.

It is not a promise that no future issues will appear. It is a concise answer
to a narrower question:

- what `gewyvern` is safe to treat as
- what `gewyvern` is intentionally not trying to be
- which boundaries matter in the current `0.14.x` line

For long-lived runtime behavior, see
[docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md).

For ingest trust semantics, see
[docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md).

For external-engine collaboration shape, see
[docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md).

## Intended Deployment Shape

`gewyvern` should currently be treated as:

- a standalone debugger/runtime
- a latest-snapshot analysis service
- a conservative evidence and diagnosis authority

It should not currently be treated as:

- a multi-tenant control plane
- an authenticated orchestration service
- a durable event warehouse
- a public-facing observability endpoint

That means the safest default mental model is:

- local or intentionally nearby usage
- bounded sessions
- read-mostly consumers
- explicit operator intent for broader exposure

## Ingest Trust Boundary

Socket-fed ingest is intentionally advisory-first.

Current ingest modes:

- `demo`
- `local-advisory`
- `remote-advisory`

Important consequence:

- local and remote socket producers are not treated as authenticated lineage
  authorities
- PID-scoped conclusions must therefore remain guarded
- ambiguous or advisory output is often the runtime refusing to overclaim, not a
  failure of the tool

`--pid` is intentionally rejected with socket ingest for this reason.

## API Exposure Boundary

The API is a read-only latest-snapshot surface, not a trust boundary.

Current posture:

- `--api-socket` requires `--serve`
- remote API bind requires explicit operator opt-in
- latest snapshot state is in-memory only
- restart clears the latest API snapshot

This API is suitable for:

- local operator tooling
- nearby sidecars
- lightweight automation

It is not equivalent to an authenticated fleet service.

## External Engine Boundary

External engines are additive and bounded.

Current posture:

- `gewyvern` computes built-in analysis first
- the external engine receives an analysis snapshot through a process boundary
- returned enrichments are appended
- the built-in diagnosis spine remains authoritative

External engines should not be allowed to redefine:

- fact truth
- core failure semantics
- ingest trust
- strong PID attribution

If an external engine fails, built-in analysis should still complete and remain
usable.

## Resource And Safety Boundaries

The current tree already enforces several practical bounds that matter for the
next preparation line:

- socket ingest applies line and fact-count limits during read
- Unix sockets are created with restricted permissions
- slow API clients are handled independently and bounded by timeout behavior
- external-engine execution is bounded by timeout, output caps, augmentation
  caps, and cache limits
- protocol/profile discovery avoids symlink recursion and repeated-directory
  loops

These are not a full security model, but they are the current concrete guards
that keep standalone use from drifting into obviously unsafe territory.

## What Operators Should Assume

Before exposing or automating `gewyvern`, assume:

1. socket ingest is advisory unless proven otherwise
2. the API is a convenience surface, not an auth system
3. external engines can enrich results, but should not become truth authority
4. exported bundles and JSON reports are useful artifacts, not access-control
   boundaries

If a deployment needs:

- durable history
- authenticated access
- multi-instance coordination
- policy and audit

that should live above `gewyvern`, not be inferred from its current local
service shape.

## Practical `0.14.x` Goal

For the current `0.14.x` line, the security goal is not “become a control
plane”.

The goal is narrower:

- keep defaults conservative
- keep exposure explicit
- keep runtime/resource bounds documented and real
- keep nearby extensibility additive rather than authority-stealing

That is the posture worth preserving while the project continues converging
toward a later stable release.
