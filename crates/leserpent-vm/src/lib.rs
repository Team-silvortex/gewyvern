#![forbid(unsafe_code)]

//! Product-owned Leserpent evaluator, authority adapters and durable journals.
//!
//! This crate consumes Leselang language/runtime primitives; it is not part of
//! the independent native language export. Disable default features for the
//! database-free product evaluator. The legacy `leselang_vm` root API re-exports
//! these exact types rather than wrapping or duplicating the implementation.

include!("leserpent_profile.rs");
