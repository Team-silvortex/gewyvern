# Leselang VM

One shared compiler/session implementation with an explicit optional Leserpent
reference specialization. This crate remains in the product workspace while
complete language, recovery and compatibility extraction gates are open.

## Build Profiles

| Profile | Cargo Features | Entry Points |
| --- | --- | --- |
| Native in-memory | `default-features = false` | `native::{syntax, hir, core, Vm, LocalVm}` |
| Reference in-memory | `default-features = false`, `leserpent-profile` | Released root `Vm`, product operations, ephemeral journal |
| Reference durable | Defaults, or `sqlite-journal` | Released root API and SQLite journal |

The default profile is unchanged. Consumers of the previous database-free root
API must explicitly enable `leserpent-profile`. `sqlite-journal` implies that
profile; there is no silent fallback when a durable journal cannot open.
Cargo features are additive, not a capability or security boundary.

## Native Boundary

`native::Vm` is exactly HIR's `StackedControlSession`, not a parallel evaluator.
It borrows original IR and retains one fuel meter across synchronous reply-gated
slices. Hosts provide catalogs, live policy, authenticated identities, native
dispatch, bounded captures and cancellation delivery. No timer, thread pool,
global lock, database, GUI toolkit or product DTO is required in the native
normal/build dependency closure.

`native::LocalVm` is the optional `LocalStackedControlSession` from shared HIR.
It takes a `LocalExecutionScope` whose weak lease the host has installed in its
native authority, requests and result domains. The original session still owns
the sole meter and all evaluation/reply state. Completion/failure close the scope
before returning; cancellation and Drop close before retained-session cleanup.
An escaping native unwind closes the scope even when the caller catches it and
keeps the wrapper. Ordinary pending reply rejection preserves both input and
liveness. This wrapper is intentionally !Send/!Sync; `native::Vm` is unchanged.
It does not preempt callback-local cleanup or cancel external native collectors.

`tests/native_embedding.rs` proves two scalar-host source/helper/capture/reply
verticals and negative policy, receipt, cancellation and fuel cases. It is not
complete reference-language/group coverage, a real GUI adapter, durable recovery
or the planned full `host_embedding.rs` proof. The source checkpoint currently
exports syntax/runtime-core/HIR and this native facade, not the complete VM or
the product/reference profile. All-features builds of that checkpoint stay native.
See the [embedding contract](../../docs/leselang-embedding.md#native-vm-build-profile)
and [extraction gates](../../docs/leselang-embedding.md#repository-extraction-readiness).
