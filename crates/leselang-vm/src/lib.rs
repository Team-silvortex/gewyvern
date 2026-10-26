#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "leserpent-profile"),
    doc = r#"
The native profile does not expose the product-specialized evaluator or journals:

```compile_fail
use leselang_vm::{Vm, EffectOperation, PresentationOperation};
```
"#
)]

//! Host-neutral embedded entry points with an optional Leserpent specialization.
//!
//! Defaults retain the released product API and SQLite journal. Use
//! `default-features = false` for the native in-memory profile, or add
//! `features = ["leserpent-profile"]` for the database-free reference evaluator.
//! Native execution uses the existing HIR sessions, not a duplicate evaluator.

pub mod native;

// Compatibility only: the product implementation lives outside the language crate.
#[cfg(feature = "leserpent-profile")]
pub use leserpent_vm::*;
