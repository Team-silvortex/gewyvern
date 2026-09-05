#![forbid(unsafe_code)]

//! Stable, product-independent GewyLang IR values, validation, and wire exchange.

mod analysis;
mod binding;
mod diagnostics;
mod diff;
mod fingerprint;
mod projection;
mod validation;
mod wire;

pub use analysis::*;
pub use binding::*;
pub use diagnostics::*;
pub use diff::*;
pub use fingerprint::*;
pub use projection::*;
pub use validation::*;
pub use wire::*;
