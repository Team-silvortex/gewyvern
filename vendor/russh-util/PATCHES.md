# Temporary chrono compatibility patch

Source: crates.io `russh-util` 0.52.0, Apache-2.0.
Archive SHA-256:
`668424a5dde0bcb45b55ba7de8476b93831b4aa2fa6947e145f3b053e22c60b6`.
The archive omits a license file; `LICENSE.txt` is retained from the upstream
repository at revision `1c0ddf4ac390d4cb60a431ab94b42bfd0a2486c3`.

RUSTSEC-2026-0543 marks `chrono` unmaintained. This applies the time adapter change
from upstream PR #825 (`1c0ddf4ac390d4cb60a431ab94b42bfd0a2486c3`), which has not
yet been merged/released: `web-time::Instant` replaces the WebAssembly wall clock.
Native `std::time::Instant`, the runtime module and public module exports remain
unchanged. The manifest only replaces the target-specific dependency and disables
publication. No SSH authentication, cryptography or transport implementation is
forked here.

A path patch keeps this small backport reproducible without an unreleased Git
dependency. cargo-audit skips both path and Git packages. CI therefore also audits
`vendor/advisory.Cargo.lock`, which retains both patched packages' upstream
registry identities. Regression tests bind that inventory to the real lock and
manifests. Transitive dependencies remain covered by the main lockfile audit.
Do not add advisory exceptions or remove the supplemental audit.

Remove this directory, its root `[patch.crates-io]`/workspace exclusion and the
independent version entry in `crates/gewyvern-dev/src/version.rs` when the upstream
release includes the fix. Remove its supplemental audit entry at the same time.
Rerun both audits and native SSH/all-feature workspace checks.
