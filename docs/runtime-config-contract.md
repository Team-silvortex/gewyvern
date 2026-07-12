# Runtime Config Contract

Use this page when you want the machine-facing contract candidate for the
runtime config surface, not the full explanatory reference.

This page answers:

- which inputs should tools depend on first
- which compatibility behaviors still exist
- what must stay true before the next tightening line

Use these nearby pages with it:

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/machine-surface-freeze.md](docs/machine-surface-freeze.md)
- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)

## Preferred Contract

New tools should depend on these first:

- config search order
- `schema_version`
- section names
- supported keys inside each section
- explicit override precedence
- explicit legacy copy-forward behavior

## Current Stable Reads

Treat these as the current contract candidate:

| Area | Preferred read | Current status |
| --- | --- | --- |
| file discovery | `GEWY_CONFIG_FILE` then standard root then legacy fallback | `blessed` |
| top-level gate | `schema_version = 1` | `blessed` |
| compatibility posture | missing schema means `legacy_unversioned` | `compat` |
| section surface | `[runtime]`, `[external_engine]`, `[paths]`, `[certificates]`, `[logging]`, `[resilience]` | `blessed` |
| rejection posture | unknown sections and unknown top-level keys reject | `blessed` |
| migration posture | copy-forward from legacy config roots without overwrite | `blessed` |

## Current Compatibility Carry-Over

These behaviors still exist for compatibility and should not disappear
casually:

- unversioned config files still load
- legacy `~/.gewyvern/config.toml` and `~/.gewyvern/gewyvern.toml` still
  participate in fallback discovery
- legacy certificate shelves may still be copied into standard roots when the
  destination is missing

New documentation should not present those as the preferred first path.

## Freeze Gate

Treat the runtime config surface as frozen enough for the next minor tightening
step only when:

1. the search order stays explicit
2. the supported section/key list stays enumerated
3. compatibility fallback stays documented where it still exists
4. migration behavior remains conservative and non-destructive

## Earliest Tightening Reading

For the current planning posture:

- `blessed` reads should remain dependable through the `0.18.x` line
- `compat` behaviors should not tighten without an explicit release-line note
- legacy unversioned acceptance should remain deliberate in `1.0.0`, not be
  dropped by surprise
