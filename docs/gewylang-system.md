# GewyLang Documentation

The canonical entry for language documentation is now the
[GewyLang module](modules/gewylang.md). This compatibility page preserves the
older stable link without maintaining a second language map.

## Minimal Paths

- Author a package:
  [tutorial](book/tutorial-gewylang-package.md) ->
  [canonical style](gewylang-style.md) ->
  [syntax](dsl-syntax.md)
- Generate with a language model:
  [model guide](gewylang-llm-guide.md) +
  [canonical style](gewylang-style.md)
- Look up exact values:
  [vocabulary reference](dsl-reference.md) +
  [EBNF](gewylang.ebnf)
- Understand compilation:
  [language and IR contract](gewylang-contract.md) ->
  [GewyLang to IR](book/explanation-gewylang-to-ir.md) ->
  [IR lowering](book/reference-ir-lowering.md) ->
  [`gewyc` JSON](gewyc-json.md)
- Convert old source:
  [migration guide](gewylang-migration.md)

## Ownership Rule

Language syntax and generation rules belong in the style, syntax, EBNF, or
reference page. Package composition belongs in the package reference. Compiler
output belongs in `gewyc` contracts. Implementation ownership follows
`gewylang-contract -> gewylang-syntax -> gewylang-compiler`, with
`gewylang-contract -> gewylang-ir` owning stable Binding IR and Analysis IR
values, canonical content fingerprints, fail-closed invariant validation,
strict standalone JSON exchange, and typed compatibility diffing. The root `gewyvern::dsl` and
`gewyvern::gewyc` modules remain the
runtime materialization and registry-analysis adapters. Runtime materialization
is reached only through `gewylang_compiler::BindingMaterializer`; compiler-stage
reports return only through `gewylang_ir::CompilerProjectionHost`. This page
contains no duplicate language contract.
