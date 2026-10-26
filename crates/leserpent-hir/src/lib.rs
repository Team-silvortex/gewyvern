#![forbid(unsafe_code)]

//! Leserpent-specific compilation, effects and capability validation.
//! Native IR and compilation primitives belong to `leselang-hir-core`. This
//! adapter must not depend on the compatibility facade, which re-exports it.

pub(crate) use leselang_hir_core::*;

include!("profile_modules.rs");
include!("leserpent_profile.rs");
