# Temporary chrono compatibility patch

Source: crates.io `russh-sftp` 2.3.0, Apache-2.0 (see `LICENSE`).
Archive SHA-256:
`9ed8949eca4163c18a8f59ff96d32cf61e9c13b9735e21ef32b3907f4aafa1a9`.
Library sources and the upstream README/license are retained with CRLF line
endings normalized to LF. Examples, benchmarks,
their dev-dependencies and the upstream lockfile are not product build inputs.

RUSTSEC-2026-0543 marks `chrono` unmaintained. Upstream removed it after 3.0.1
(`93954669047e8ac05bc8d60f5c3a077fb67f9f1b`), but that revision requires
`serde >= 1.0.229`, incompatible with the independently pinned Leselang core's
`serde = 1.0.228`. This backport avoids upgrading the protocol API or language core.

Apart from line endings, only the following library files differ from the release archive:

- `src/utils.rs`: use `SystemTime::duration_since`, preserving fractional negative
  timestamp flooring and wrapping conversion to SFTP v3's unsigned seconds.
- `src/protocol/file.rs`: use the workspace's existing `time` dependency to retain
  the exact UTC, English `%b %d %Y %H:%M` longname format for all u32 timestamps.

The manifest drops `chrono`, adds `time` and omits example/benchmark-only deps.
No packet, authentication, filesystem, timeout or transport logic is changed.
CI runs the backport's boundary tests with
`cargo test --locked -p leserpent-adapters -p russh-sftp --features leserpent-adapters/native-ssh --lib`.
No advisory is ignored. Remove this directory and its root `[patch.crates-io]`
entry and `version.rs`'s independent-version entry once a maintained upstream
release is compatible with the pinned core;
rerun audit, adapter tests and the all-feature workspace checks before doing so.

cargo-audit skips path/Git packages, so CI also audits this package's original
registry identity in `vendor/advisory.Cargo.lock`. The main audit checks the actual
transitive dependencies after replacing `chrono`. Remove the supplemental identity
only when replacing this path patch with a released registry package.
