# Explanation: gewylang To Frontend And IR

This chapter explains the middle of the `gewyvern` language pipeline:

```text
.gewy source (Syntax v1)
  -> Expanded AST v1
  -> Binding IR v1 (TemplateBinding)
  -> Analysis IR v1 (IrReport)
  -> archival Analysis IR snapshot
```

It sits between:

- the normative stage contract in
  [docs/gewylang-contract.md](../gewylang-contract.md)
- the source-language guide in
  [docs/dsl.md](../dsl.md)
- the source-shape companion in
  [docs/dsl-syntax.md](../dsl-syntax.md)
- the exact lowering contract in
  [docs/book/reference-ir-lowering.md](reference-ir-lowering.md)
- the runtime walkthrough in
  [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)

Use this page when you want to understand how one authored package becomes a
reviewable compiler story before the runtime ever starts.

## Book Path

This chapter lives in Part II: The Language And Compiler Spine.

Read it after:

- [docs/dsl.md](../dsl.md)
- [docs/dsl-syntax.md](../dsl-syntax.md)
- [docs/gewylang-evolution.md](../gewylang-evolution.md)

Then continue with:

- [docs/book/reference-ir-lowering.md](reference-ir-lowering.md)
- [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)

## Best Used For

Use this page when you want to answer questions like:

- what does `gewyc frontend` prove?
- what changes when the compiler lowers into IR?
- why do `ir_lowering_delta` and `history_snapshot` both exist?
- how should a reviewer inspect a new package or language feature?

Do not use this page as exact schema reference.

For that, use:

- [docs/gewyc-json.md](../gewyc-json.md)
- [docs/book/reference-ir-lowering.md](reference-ir-lowering.md)

## Step 1: Start With Author Intent

At the source level, a `gewylang` package is trying to say a few simple
things:

- which runtime capability should be selected
- which reusable helpers should be included
- which fragments and operations matter
- which parameters and narratives make the package specific

At this stage, the important property is not “full language power”.

It is legibility.

The package should still be reviewable as authored intent.

That is why the current language posture stays narrow:

- pipeline-driven
- package-oriented
- function-unit reuse
- lightweight safety boundaries

## Step 2: Expand Into Expanded AST v1

The first public compiler projection is Expanded AST v1, rendered as the
expanded frontend graph.

This is what `gewyc frontend` is for.

It answers:

- what files were included?
- what functions were declared?
- which `use(...)` edges exist?
- what does the merged entry-level structure look like?

Conceptually:

```text
main.gewy
  + include(...) sources
  + fn units
  + use(...) edges
  = expanded frontend module graph
```

This is still close to authorship.

It is the last compiler surface where package/module structure is shown almost
directly rather than as rule-bearing models.

## Why The Frontend Graph Matters

Without this stage, the compiler would jump too quickly from source text to
lowered behavior.

That would make several things harder to review:

- include provenance
- helper-function expansion
- accidental package complexity
- whether a feature is actually local or spread across modules

The frontend graph acts as the “source truth you can still inspect”.

## Step 3: Cross Into Binding IR v1

Once the frontend package is expanded, the compiler crosses into Binding IR
v1, represented by `TemplateBinding`.

This is the first deliberate narrowing step.

At this point the compiler keeps the essentials:

- template id
- fragment set
- window profile
- reason profile
- operation/program shape
- fragment parameter bindings
- evidence-tier overrides

It stops carrying purely editorial source detail as first-class truth.

That is intentional.

The system needs to preserve author intent, but it also needs to become stable
enough for runtime planning and diagnostics.

## Step 4: Project Analysis IR v1

After the binding boundary, the compiler projects explicit Analysis IR model
surfaces and enriches them with supportability diagnostics.

Today the important pair is:

- `program_model`
- `reason_model`

This is where the compiler answers:

- what rule-bearing behavior was materialized?
- what explanatory surface was selected?
- which modules and phases exist after lowering?

This is a more semantic view than the frontend graph.

The frontend graph is about composition structure.
The lowered IR is about behavior structure.

## Frontend Graph Versus Lowered IR

A good shorthand is:

- frontend graph says “how the package was assembled”
- lowered IR says “what executable model shape that assembly became”

Those are related, but they are not the same job.

The compiler needs both because:

- authors need structural provenance
- reviewers need behavioral shape
- the runtime needs rule-bearing models

## Step 5: Read `ir_lowering_delta`

`ir_lowering_delta` exists to connect those two worlds.

It is the compact compare surface between:

- the frontend/module-side view
- the lowered/model-side view

It helps answer questions like:

1. did the package lower into the shape I expected?
2. did rule counts jump unexpectedly?
3. did module and phase names survive the transition clearly?
4. do `program_model` and `reason_model` still look aligned?

This is why `ir_lowering_delta` is so important during language evolution.

It is the “did we preserve explainability while lowering?” checkpoint.

## Why `lowered_models` Exists

Inside `ir_lowering_delta`, the `lowered_models` list is the compact per-model
summary.

It exists so reviewers do not have to scan every rule just to answer:

- how many lowered models exist?
- what kinds are they?
- how many rules became supported or unsupported?
- which modules and phases are present?

This is the best middle layer for change review.

It is narrower than full IR detail, but much richer than a single summary
line.

## Step 6: Read `history_snapshot`

`history_snapshot` is not just another copy of the IR report.

It serves a different purpose.

`ir_lowering_delta` is for current inspection and comparison during active
review.

`history_snapshot` is for durable archival shape.

Its job is to preserve a smaller answer to:

- what was the lowered shape of this package or minor line?
- what did the compiler consider the program and reason surfaces to be?
- what model counts and support posture existed at that time?

This is why it is ideal for:

- minor-line history pages
- release-line snapshots
- long-term contract diffs

## A Practical Inspection Sequence

When reviewing a package or compiler change, a strong inspection order is:

1. `gewyc frontend`
   Check includes, function units, and graph edges.
2. `gewyc ir`
   Check the direct `program_model`, `reason_model`, and supportability shape.
3. `gewyc_ir_snapshot`
   Check the durable archival shape you would be comfortable recording in
   history.

In command form:

```bash
cargo run -p gewyc -- frontend dsl/http_request_path.gewy --focus graph
cargo run -p gewyc -- ir dsl/http_request_path.gewy --json
cargo run --bin gewyc_ir_snapshot -- dsl/http_request_path.gewy --json
```

Use `gewyc explain --focus ir` instead of step 2 when the review also needs
`ir_lowering_delta` and surrounding troubleshooting notes.

This sequence moves from:

- source assembly
- to lowered behavior
- to durable historical record

## What This Means For Language Features

When a new `gewylang` feature is proposed, it should be reviewable at all
three middle surfaces:

1. Expanded AST
2. Binding IR
3. Analysis IR and its history projection

That means a feature is not really integrated yet if:

- it cannot be shown cleanly in the frontend graph
- it lowers in a way that is hard to explain
- it disappears from the archival view entirely

## Why This Middle Layer Is So Important

This is where the project either stays honest or starts to drift.

If source features grow faster than these middle surfaces can explain them,
then:

- reviewers lose trust
- runtime surprises increase
- documentation gets vague
- minor-line history stops meaning much

If these middle surfaces stay strong, then the project can keep evolving
without becoming mysterious.

## How This Connects To Runtime

The runtime does not consume the raw frontend graph.

It consumes the consequences of lowering and supportability planning.

That is why this middle layer is the handoff point between:

- author intent
- compiler explanation
- runtime truth

The runtime walkthrough begins where this page ends:

- [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)

## Review Order

If you want the full language pipeline in a stable reading order, use:

1. [docs/book/tutorial-gewylang-package.md](tutorial-gewylang-package.md)
2. [docs/dsl.md](../dsl.md)
3. [docs/book/explanation-gewylang-to-ir.md](explanation-gewylang-to-ir.md)
4. [docs/book/reference-ir-lowering.md](reference-ir-lowering.md)
5. [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)

## Current Thesis

For the current line, the middle compiler layer should remain:

- inspectable
- provenance-aware
- compact enough to review
- structured enough to archive

That is what lets `gewylang` stay small while still growing into a serious,
trustworthy subsystem.

## Continue With

If you want the exact lowering contract next, go to:

- [docs/book/reference-ir-lowering.md](reference-ir-lowering.md)

If you want to follow the same path into runtime materialization, go to:

- [docs/book/explanation-gewy-to-runtime.md](explanation-gewy-to-runtime.md)
