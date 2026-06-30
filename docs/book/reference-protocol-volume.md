# Reference: Protocol Volume Guide

Use this page as the front door to the protocol reference volume.

This page is not the contract itself. It is the routing shelf for choosing the
right protocol book page first.

Read this page when the question is:

- “which protocol page should I open first?”
- “am I here for contract, examples, commands, validation, or release?”
- “what is the shortest route for the kind of protocol work I am doing?”

## The Six Main Doors

### 1. Contract

Start here when you need canonical family, entry, alias, or default-entry
rules:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)

Then continue with:

- [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)

### 2. Reading Spine

Start here when you already found the right family and need the next page:

- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)

Use this for:

- contract lookup order
- explanation order
- package-debug order
- runtime-confidence order

### 3. Examples

Start here when you want the nearest real `.gewy` file:

- [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)

Use this for:

- family hub -> DSL sample
- family hub -> walkthrough
- package change prep

### 4. Commands

Start here when you want the shortest direct CLI route:

- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)

Use this for:

- `cargo run -- --protocol ...`
- `--scan-all`
- `--serve` plus API checks

### 5. Operator Triage

Start here when you are narrowing a protocol issue in practice:

- [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)

Use this for:

- family-first triage
- grouped validation
- operator-facing runtime trust
- packaged versus source-tree confidence

### 6. Release Judgment

Start here when the question is whether `0.15.x` is protocol-healthy enough
to ship:

- [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)

Use this for:

- minor-line ship read
- packaged protocol confidence
- cross-project protocol confidence

## Shortest Routes By Intent

- “I need exact protocol rules”:
  [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
  -> groups -> family shelves
- “I need the closest sample”:
  family hub -> [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)
- “I need the first runnable command”:
  family hub -> [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
- “I need to validate one family or shelf”:
  family hub -> [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- “I need to debug operator-facing drift”:
  family hub -> [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
- “I need a release decision”:
  family hub -> [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)

## Keep Nearby

- [docs/book/reference-protocol-alias-index.md](docs/book/reference-protocol-alias-index.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- [docs/cli-recipes.md](docs/cli-recipes.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
