# GewyLang Documentation Module

Owns the language, `gewyc`, package composition, lowering, and migration.

## Start

1. [Package tutorial](../book/tutorial-gewylang-package.md)
2. [Canonical style](../gewylang-style.md)
3. [Language and model guide](../gewylang-llm-guide.md)

For a minimal LLM context, provide only the canonical style, model guide, and
the nearest real `.gewy` example.

## Authoring

- [Language overview](../dsl.md)
- [Syntax](../dsl-syntax.md)
- [Vocabulary reference](../dsl-reference.md)
- [Canonical EBNF](../gewylang.ebnf)
- [Package and module reference](../book/reference-gewylang-package.md)
- [Legacy migration](../gewylang-migration.md)

## Compiler

- [Language and IR contract](../gewylang-contract.md)
- [`gewylang-syntax` frontend](../../crates/gewylang-syntax)
- [`gewylang-compiler` semantic-host and materializer boundaries](../../crates/gewylang-compiler)
- [`gewylang-ir` stage values and projection-host contract](../../crates/gewylang-ir)
- [GewyLang to IR](../book/explanation-gewylang-to-ir.md)
- [Lightweight types](../book/explanation-gewylang-lightweight-types.md)
- [IR lowering](../book/reference-ir-lowering.md)
- [`gewyc` JSON](../gewyc-json.md)
- [`gewyc` field contract](../gewyc-field-contract.md)
- [`gewyc` samples](../gewyc-sample-index.md)

Protocol-specific implementation belongs to the
[protocols module](protocols.md).
