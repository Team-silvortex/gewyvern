# Leselang Documentation Module

Owns the Leserpent automation language, its compiler boundary, effect model,
capability checks, continuation format, and deterministic re-entry semantics.

Leselang is scoped as a protocolized GUI/control automation runtime, not a
general-purpose VM. The intended core is a hostable Rust crate for Rust-native
GUI frameworks plus a narrow protocol/FFI surface for other frontend languages.
No GUI framework is automatically compatible; each one needs a developer-owned
adapter or generated framework binding against the Leselang UI protocol. The
`UiAdapterManifest` is the shared proof for that binding.

## Start

1. [Current language contract](../leselang-language.md)
2. [Renderer-neutral UI IR contract](../leselang-ui.md)
3. [Leserpent 2.0 architecture](../leserpent-2-architecture.md)
4. [Gate-based delivery roadmap](../leserpent-2-roadmap.md)

## Contracts

- [Domain and protocol compatibility](../../crates/leserpent-protocol/COMPATIBILITY.md)
- [Project status tensor](../project-status-system.md)
- [GewyLang module](gewylang.md)

Leselang and GewyLang are separate languages. GewyLang defines Gewyvern
protocol behavior; Leselang drives Leserpent orchestration and UI functions.
