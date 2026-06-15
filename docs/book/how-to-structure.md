# How-To Structure

Use this page when you need the task discipline for the `how-to` volume.

This page explains how practical operator and contributor tasks should be
organized inside the book.

Read this alongside:

- [docs/book/how-to.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to.md)
- [docs/book/structure.md](/Users/Shared/chroot/dev/gewyvern/docs/book/structure.md)
- [docs/book/conventions.md](/Users/Shared/chroot/dev/gewyvern/docs/book/conventions.md)

## Why This Page Exists

The `how-to` volume is easy to neglect because task pages often begin life as
"just one useful note".

Without structure, that quickly turns into:

- one huge grab-bag page
- duplicated command ladders
- pages that mix tutorial and task guidance
- missing task entrypoints for the most common maintenance work

This page keeps the `how-to` volume practical without letting it become messy.

## What A How-To Is

A `how-to` page should answer one practical question well.

Examples:

- how do I validate the current runtime surface?
- how do I add or debug one protocol package?
- how do I wire an external engine safely?
- how do I validate packaged runtime behavior?

The reader should already have basic context.
The page should focus on completing the task with confidence.

## The Current Task Groups

The current `how-to` volume should be understood as four task bands.

### 1. Validate

Use this band when the question is:

- is the current checkout trustworthy?
- where did drift enter?
- what evidence supports a release judgment?

Typical pages:

- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

### 2. Extend

Use this band when the question is:

- how do I add one more packaged capability?
- how do I debug a drifting package?
- what exact checks prove the new package belongs in the shelf?

Typical pages:

- [docs/book/how-to-add-or-debug-protocol-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-add-or-debug-protocol-package.md)

### 3. Operate

Use this band when the question is:

- how do I run and observe the runtime correctly?
- how do I use serve/API mode?
- how do I interpret degraded or advisory behavior during operation?

Right now this band is still partly covered by top-level durable docs such as:

- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
- [docs/ingest-modes.md](/Users/Shared/chroot/dev/gewyvern/docs/ingest-modes.md)

Future `how-to` pages should likely land here.

### 4. Collaborate And Package

Use this band when the question is:

- how do I validate sidecar collaboration?
- how do I package or validate Linux/container surfaces?
- how do I exercise the multi-module stack intentionally?

Right now this band is still distributed across:

- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
- [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md)
- [docs/external-engine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/external-engine-contract.md)

Future `how-to` pages should make this band more explicit.

Current concrete page:

- [docs/book/how-to-wire-etragon-sidecar.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-wire-etragon-sidecar.md)

## Good How-To Shape

A strong `how-to` page should usually have:

1. when to use this guide
2. the shortest practical task ladder
3. how to interpret success
4. how to triage failure
5. where to go next

This is enough structure to stay repeatable without becoming rigid.

## What Does Not Belong In A How-To

Avoid using a `how-to` page for:

- broad architecture rationale
- exact schema reference
- first-ever onboarding
- release-line philosophy

Those belong in:

- explanation
- reference
- tutorial
- top-level posture/history docs

## Practical Rule

Before adding a new `how-to` page, ask:

1. is the reader trying to complete one task?
2. does the page have a clear success condition?
3. would a tutorial or reference page be a better fit?
4. which task band does it belong to: validate, extend, operate, or collaborate?

If the task band is unclear, the page usually needs reframing.

## Current Thesis

For the current line, the `how-to` volume should feel like:

- a practical operator/contributor handbook
- short enough to act from directly
- structured enough to scale as more tasks are documented

That is the standard future `how-to` pages should match.
