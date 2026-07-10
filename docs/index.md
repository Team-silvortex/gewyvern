# Documentation Map

This page is the single entry point for the `gewyvern` documentation set.

If you are not sure where to start, do not browse `docs/` file by file. Start
here, pick one track, and only drill into specialist pages when you need them.

Use this page when you want:

- the shortest route to the right shelf
- the difference between the global docs map and the book reading order
- the right page for operations, language work, release checks, or architecture

## Short Path

For most readers, the right order is:

1. [README.md](README.md)
2. [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)
3. [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
4. [docs/system.md](docs/system.md)
5. [docs/dsl.md](docs/dsl.md)
6. [docs/development.md](docs/development.md)

If you want the project as a book instead of a shelf map, jump to:

- [docs/book/index.md](docs/book/index.md)

If you want the script/operator map first, jump to:

- [docs/script-entrypoints.md](docs/script-entrypoints.md)

If you want the command shelf first, jump to:

- [docs/cli-recipes.md](docs/cli-recipes.md)

If you want the current monorepo stack layout first, jump to:

- [docs/monorepo-stack.md](docs/monorepo-stack.md)

If you want the project-wide dataflow topology first, jump to:

- [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)

## Goal-Based Routes

- release answer:
  [docs/release-checklist.md](docs/release-checklist.md)
- field confidence and validation posture:
  [docs/field-validation.md](docs/field-validation.md)
- running the right scripts:
  [docs/script-entrypoints.md](docs/script-entrypoints.md)
- runtime CLI, `gewyc`, socket, and API commands:
  [docs/cli-recipes.md](docs/cli-recipes.md)
- current monorepo stack layout and subproject entrypoints:
  [docs/monorepo-stack.md](docs/monorepo-stack.md)
- project-wide dataflow topology:
  [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
- packaging and native artifacts:
  [docs/packaging.md](docs/packaging.md)
- runtime exposure and security preflight:
  [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
- current release posture and history:
  [docs/history/v0.20.x.md](docs/history/v0.20.x.md),
  [docs/history/v0.19.x.md](docs/history/v0.19.x.md),
  [docs/history/v0.18.x.md](docs/history/v0.18.x.md),
  [docs/history/index.md](docs/history/index.md)
- current validation program:
  [docs/field-validation.md](docs/field-validation.md)
- current observed validation evidence:
  [docs/field-findings.md](docs/field-findings.md)
- shortest release gate:
  [docs/release-checklist.md](docs/release-checklist.md)

## Main Shelves

### Read Like A Book

Use these when you want reading order instead of topic lookup:

- [docs/book/index.md](docs/book/index.md)
- [docs/book/tutorials.md](docs/book/tutorials.md)
- [docs/book/how-to.md](docs/book/how-to.md)
- [docs/book/reference.md](docs/book/reference.md)
- [docs/book/explanation.md](docs/book/explanation.md)

### Understand The Runtime

- [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
- [docs/system.md](docs/system.md)
- [docs/architecture.md](docs/architecture.md)
- [docs/architecture-blueprint-modules.md](docs/architecture-blueprint-modules.md)
- [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
- [docs/module-boundaries.md](docs/module-boundaries.md)
- [docs/architecture-evolution.md](docs/architecture-evolution.md)
- [docs/service-behavior.md](docs/service-behavior.md)
- [docs/security-posture.md](docs/security-posture.md)
- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/machine-surface-freeze.md](docs/machine-surface-freeze.md)
- [docs/runtime-config-contract.md](docs/runtime-config-contract.md)
- [docs/runtime-certificate-policy-contract.md](docs/runtime-certificate-policy-contract.md)
- [docs/export-format-contract.md](docs/export-format-contract.md)

### Understand `gewylang` And `gewyc`

- [docs/dsl.md](docs/dsl.md)
- [docs/dsl-syntax.md](docs/dsl-syntax.md)
- [docs/dsl-reference.md](docs/dsl-reference.md)
- [docs/gewylang-system.md](docs/gewylang-system.md)
- [docs/gewylang-evolution.md](docs/gewylang-evolution.md)
- [docs/gewyc-json.md](docs/gewyc-json.md)
- [docs/gewyc-field-contract.md](docs/gewyc-field-contract.md)
- [docs/gewyc-contract-matrix.md](docs/gewyc-contract-matrix.md)
- [docs/gewyc-freeze-checklist.md](docs/gewyc-freeze-checklist.md)
- [docs/gewyc-sample-index.md](docs/gewyc-sample-index.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

### Understand Protocol Packages

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)

### Operate And Validate

- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- [docs/field-validation.md](docs/field-validation.md)
- [docs/field-findings.md](docs/field-findings.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/cli-recipes.md](docs/cli-recipes.md)

### Extend The Broader Stack

- [docs/sidecar-collaboration.md](docs/sidecar-collaboration.md)
- [docs/external-engine-contract.md](docs/external-engine-contract.md)
- [docs/book/explanation-dataflow-topology.md](docs/book/explanation-dataflow-topology.md)
- [docs/architecture-coordination.md](docs/architecture-coordination.md)
- [docs/monorepo-stack.md](docs/monorepo-stack.md)

### Build, Package, And Measure

- [docs/packaging.md](docs/packaging.md)
- [docs/performance-baselines.md](docs/performance-baselines.md)
- [docs/headless-linux.md](docs/headless-linux.md)
- [docs/development.md](docs/development.md)

## Role-Based Starting Points

- operator:
  [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md),
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- DSL author:
  [docs/book/tutorial-gewylang-package.md](docs/book/tutorial-gewylang-package.md),
  [docs/gewylang-system.md](docs/gewylang-system.md),
  [docs/dsl-syntax.md](docs/dsl-syntax.md)
- contributor:
  [docs/development.md](docs/development.md),
  [docs/module-boundaries.md](docs/module-boundaries.md)
- reviewer:
  [docs/history/v0.20.x.md](docs/history/v0.20.x.md),
  [docs/history/v0.19.x.md](docs/history/v0.19.x.md),
  [docs/history/v0.18.x.md](docs/history/v0.18.x.md),
  [docs/field-findings.md](docs/field-findings.md)

## Scope

This page is intentionally a map, not a second table of contents for every
chapter in the book.

Use:

- [docs/index.md](docs/index.md)
  when you want the global shelf map
- [docs/book/index.md](docs/book/index.md)
  when you want reading order
- [docs/documentation-system.md](docs/documentation-system.md)
  when you want the design rules behind the docs themselves
