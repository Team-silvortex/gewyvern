# Reference: Protocol Reading Paths

Use this page when you already found the right protocol family, but do not yet
know what to read next.

This page exists to bridge three different needs:

- exact contract lookup
- system-level understanding
- practical validation and debugging

It turns the protocol reference shelf into a usable reading spine instead of a
flat pile of hub pages.

Read this alongside:

- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)
- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
- [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
- [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)

## When To Use This Page

Use this page when the question is:

- “I found the right family hub, what should I open next?”
- “Do I need exact contract, explanation, or validation?”
- “What is the shortest path from one protocol family to runtime confidence?”

## Path 1: Exact Contract Lookup

Use this path when you need the strictest answer to:

- what is the canonical family and entry?
- what is the default entry?
- what aliases are accepted?
- what does this lower into?

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-protocol-groups.md](docs/book/reference-protocol-groups.md)
3. [docs/book/reference-protocol-family-shelves.md](docs/book/reference-protocol-family-shelves.md)
4. one exact family hub page
5. one exact family subpage if the family has narrower shelves
6. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

This is the right path for:

- release notes and contract review
- tooling that depends on canonical names
- protocol registry or alias drift review

## Path 2: Understand The Whole Spine

Use this path when you want to understand how one protocol package becomes a
runtime-facing diagnostic story.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. one exact family hub page
3. [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
4. [docs/book/explanation-gewylang-to-ir.md](docs/book/explanation-gewylang-to-ir.md)
5. [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)
6. [docs/architecture-walkthrough-http-request.md](docs/architecture-walkthrough-http-request.md)
7. [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)

This is the right path for:

- protocol model reviewers
- IR evolution work
- understanding how registry, DSL, lowering, and runtime stay aligned

## Path 3: Add Or Debug A Protocol Package

Use this path when the question is not just “what is this family?” but “how do
I safely change or extend it?”

Read in this order:

1. one exact family hub page
2. one nearby family subpage or sibling entry
3. [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)
4. [docs/book/reference-gewylang-package.md](docs/book/reference-gewylang-package.md)
5. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
6. [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)

This is the right path for:

- adding one more entry under `protocols/<family>/`
- debugging package drift
- reviewing whether a new package really belongs in the current shelf

## Path 4: Validate Runtime Confidence

Use this path when you care more about health and confidence than about raw
registry shape.

Read in this order:

1. one exact family hub page
2. [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
3. [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
4. [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
5. [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
6. [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
7. [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)

This is the right path for:

- deciding whether a checkout is healthy
- checking whether one protocol family drifted
- preparing a release-confidence judgment

## Minimal Reading Recipes

If you only need the shortest useful route, use one of these:

- “What is the exact contract?”:
  [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
  -> family hub -> subpage -> [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- “How does one package become runtime behavior?”:
  family hub -> [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
  -> [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)
- “How do I prove this path still works?”:
  family hub -> [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
  -> [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Alias-Led Reading Recipes

Some operator-facing names intentionally land on an existing canonical family
instead of introducing a brand new protocol shelf.

- `dot`:
  start at [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md),
  then read the TCP subpage, then cross-check the TLS client setup path in
  [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md),
  then use [docs/book/reference-dot-overlay.md](docs/book/reference-dot-overlay.md)
  as the shortest combined operator path
- `doh`:
  start at [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md),
  then follow the request/response shelf, while reading it with DNS resolver
  intent in mind rather than generic web traffic intent, then use
  [docs/book/reference-doh-overlay.md](docs/book/reference-doh-overlay.md)
  as the shortest combined operator path

## Companion-Led Reading Recipes

Some canonical surfaces are intentionally small, but they now expose
`reading_companions` so a UI or operator can jump into the next shelf without
guessing.

- `https connect`:
  read [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md),
  then immediately jump to `tls client` using the companion hint recorded in
  [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)
- `http3 request`:
  read [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md),
  then jump to `quic initial` before treating request semantics as trustworthy
- `tls client`:
  stay on [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
  only long enough to decide whether the next shelf is `https connect` or
  `dns tcp`

## Why This Matters

The protocol shelf is now big enough that “just click around until it feels
right” is no longer a good workflow.

The reference volume needs to do three things well:

- tell you what the stable contract is
- show you how that contract becomes runtime behavior
- point you at the right validation path when trust matters

That is the purpose of this page in the active `0.19.x` line.
