#![forbid(unsafe_code)]

//! Stable, product-independent GewyLang Binding IR and Analysis IR values.

mod analysis;
mod binding;
mod diagnostics;
mod projection;

pub use analysis::*;
pub use binding::*;
pub use diagnostics::*;
pub use projection::*;
