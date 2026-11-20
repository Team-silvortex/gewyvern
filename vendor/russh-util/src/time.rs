#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

// Use a monotonic clock on the web and std::time on other wasm targets.
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
