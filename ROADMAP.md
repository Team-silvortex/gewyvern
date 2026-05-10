# gewyvern Roadmap

This roadmap treats `v1.0.0` as an operational trust milestone, not as a
“feature complete forever” milestone.

The working intention is:

- current line: `v0.5.6`
- pre-1.0 stabilization line: `v0.6.x` through `v0.10.0`
- decision point: if `v0.10.0` satisfies the `1.0` gates, release `v1.0.0`
  directly after validation

## 1.0 Definition

`gewyvern` should be considered `1.0.0` only when all of the following are
true:

- protocol and process-network analysis is stable enough for day-to-day infra
  use
- DSL, compiler, and IR boundaries are stable enough to support compatibility
  promises
- HTML and JSON reports are reliable enough for operators and automation
- runtime performance and safety are predictable under realistic scan loads
- the system is documented well enough that another engineer can operate and
  extend it without tribal knowledge

`1.0.0` does not mean “all protocols are implemented”. It means the core
runtime, reporting surface, and modeling pipeline are trustworthy.

## Release Path

## v0.6.x

Focus: stabilize the language and package boundary.

- tighten `gewylang` function semantics and package composition rules
- keep `gewyc`, `gewy.pkg`, `gewy.lock`, and registry behavior predictable
- reduce avoidable churn in DSL syntax and compiler-facing outputs
- continue cleaning dead edges in CLI/reporting/runtime integration

Exit criteria:

- no major ambiguity in package entry, include, and function composition
- compiler findings and report outputs are stable enough for downstream use
- core registry/package behavior has broad regression coverage

## v0.7.x

Focus: improve module-level diagnosis quality.

- deepen protocol families that already exist instead of only adding new ones
- improve failure-path modeling, suspect module inference, and stage inference
- keep pushing from “protocol matched” toward “which network module failed”
- continue refining QUIC, HTTP/3, HY2, database, and directory-service flows

Exit criteria:

- process-level reports consistently identify useful `primary_module_kind`
- failure-path coverage is strong for the main supported protocol families
- report conclusions are noticeably more actionable than raw flow listings

## v0.8.x

Focus: harden operations and performance.

- expand benchmark coverage for scan, summary, and report generation
- pressure-test `--scan-all`, PID-scoped scans, and socket ingest paths
- tighten operational error handling and quality-of-result signaling
- continue refining trust-mode behavior and runtime resource boundaries

Exit criteria:

- benchmark baselines exist for the critical runtime/report paths
- scan/report behavior remains predictable under larger registered protocol sets
- no obvious safety or operational footguns remain in the default flow

## v0.9.x

Focus: freeze the surfaces that infra users depend on.

- minimize breaking changes in CLI flags and report structure
- minimize breaking changes in export JSON and report JSON schema
- minimize breaking changes in DSL/compiler surface that operators depend on
- prefer fixes, cleanup, and compatibility work over broad new abstractions

Exit criteria:

- release candidates can be exercised without churn in core interfaces
- docs accurately describe the real operational behavior of the system
- remaining work is mostly polish, reliability, and acceptance validation

## v0.10.0

Focus: final pre-1.0 validation.

- run the project with release-candidate discipline
- validate production-like usage patterns and failure reporting quality
- confirm documentation, examples, and reports are good enough for handoff
- confirm that the project is ready to be treated as infra

Exit criteria:

- all `1.0` gates are met
- no known critical schema, runtime, or diagnosis blockers remain
- operators can reliably use the tool without repository-specific context

## 1.0 Gates

The `v1.0.0` decision should require explicit confirmation that the following
areas are in good shape:

### Analysis Quality

- major supported protocol families have both healthy and failure-path coverage
- process-level summaries identify useful primary module and stage conclusions
- scan reports are useful enough to narrow issues without manual spelunking

### Surface Stability

- CLI flags used in day-to-day operations are stable
- HTML and JSON reports are stable enough for both humans and automation
- DSL/compiler outputs are stable enough for package- and registry-driven use

### Operational Safety

- trust modes and socket ingest defaults are safe by default
- package/include/path handling remains bounded and predictable
- loader/probe execution surfaces stay constrained and documented

### Performance

- benchmark coverage exists for the critical hot paths
- scan-all and report generation costs are understood and acceptable
- no obvious quadratic or unbounded paths remain in core operational flows

### Documentation

- README onboarding is accurate
- system, DSL, and development docs reflect the current architecture
- upgrade expectations and operational usage are clear

## Guiding Principle

The path to `v1.0.0` should bias toward:

- stronger diagnosis
- clearer reports
- safer defaults
- more predictable runtime behavior
- fewer breaking changes

and not toward raw protocol-count vanity.
