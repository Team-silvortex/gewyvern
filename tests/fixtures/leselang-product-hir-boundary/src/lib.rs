#![forbid(unsafe_code)]

//! Independent consumer of the real product adapter, not an included-source copy.
//! The native facade dependency stays default-free even while the product
//! adapter uses the same core through its own one-way dependency.

pub use leserpent_hir::*;
