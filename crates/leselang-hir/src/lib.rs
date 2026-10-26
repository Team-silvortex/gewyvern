#![forbid(unsafe_code)]
#![cfg_attr(
    not(feature = "leserpent-profile"),
    doc = r#"
The native profile does not export product operations or legacy entry points:

```compile_fail
use leselang_hir::{Effect, lower};
```

```compile_fail
use leselang_hir::host_call::HostOperation;
```

```compile_fail
use leselang_hir::computation::Computation;
```

```compile_fail
use leselang_hir::result_field::ResultField;
```
"#
)]

//! Host-defined compilation, typed control IR and bounded in-memory execution.
//!
//! Default features preserve the Leserpent reference profile and its legacy APIs.
//! Independent hosts use `default-features = false` to omit that profile and its
//! product operation contract. Native adapter work and authority remain explicit.

pub use leselang_hir_core::*;

#[cfg(feature = "leserpent-profile")]
pub use leserpent_hir::*;
