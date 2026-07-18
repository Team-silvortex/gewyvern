# Runtime Layout Reference

This page records the standard filesystem layout for `gewyvern` starting in the
`0.15.x` line.

Use it when the question is:

- where should `gewyvern` look for config, data, state, or cache files?
- where should the runtime config file itself live?
- where should packaged protocol assets live?
- how should a `0.14.x` or older local install be carried forward safely?

Do not use this page as:

- the packaging build guide
- the release checklist
- the protocol-surface contract page

For those, use:

- [docs/packaging.md](docs/packaging.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)
- [docs/book/reference-runtime-certificate-state.md](docs/book/reference-runtime-certificate-state.md)

## Layout Policy

The active `1.4.0` policy is:

- standard roots are explicit
- environment overrides stay supported
- old single-root layouts remain readable as compatibility fallbacks
- upgrades should prefer copy-forward, not destructive in-place rewrites

That means `gewyvern` should now think in four roots:

- config
- data
- state
- cache

and one packaged share root:

- installed read-only built-in assets

## Standard Roots

### Linux And Other XDG-Style Unix Hosts

- config:
  `"$XDG_CONFIG_HOME/gewyvern"` or `~/.config/gewyvern`
- data:
  `"$XDG_DATA_HOME/gewyvern"` or `~/.local/share/gewyvern`
- state:
  `"$XDG_STATE_HOME/gewyvern"` or `~/.local/state/gewyvern`
- cache:
  `"$XDG_CACHE_HOME/gewyvern"` or `~/.cache/gewyvern`

### macOS

- config:
  `~/Library/Application Support/gewyvern/config`
- data:
  `~/Library/Application Support/gewyvern/data`
- state:
  `~/Library/Application Support/gewyvern/state`
- cache:
  `~/Library/Caches/gewyvern`

### Windows

- config:
  `%APPDATA%\\gewyvern\\config`
- data:
  `%APPDATA%\\gewyvern\\data`
- state:
  `%LOCALAPPDATA%\\gewyvern\\state`
- cache:
  `%LOCALAPPDATA%\\gewyvern\\cache`

## Config File Name

The standard config file name for the active `1.4.0` line is:

- `gewyvern.toml`

For the exact search order, fallback names, and supported sections, use:

- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)

## Current Asset Expectations

The current `1.4.0` expectation is:

- protocol registry packages live under `data/protocols/`
- built-in DSL helpers can live under `data/dsl/`
- operator certificate assets live under `config/certificates/`
  - trust anchors under `config/certificates/trust/`
  - local authorities under `config/certificates/authorities/`
  - runtime identities under `config/certificates/identities/`
- certificate runtime state and future issued material can live under
  `state/certificates/`
  - rotation records under `state/certificates/rotation-records.tsv`
  - revocation records under `state/certificates/revocation-records.tsv`

For the operator-facing interpretation of these shelves, use:

- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)

During standard-root preparation, `gewyvern` also creates these certificate
roots up front and copy-forwards missing legacy assets from `~/.gewyvern/`
without overwriting files already present in the standard layout.
- latest API snapshot artifacts live under `state/latest/api/`
- archived API snapshot artifacts live under `state/history/api/`
- packaged read-only assets still live under `/usr/share/gewyvern/` on Linux

The current `1.4.0` history retention rule is intentionally simple:

- keep the most recent 32 archived API snapshot refreshes
- prune older archived refreshes during later successful writes

That default can now be overridden with:

- runtime config: `[runtime].history_retention`
- environment: `GEWY_HISTORY_RETENTION`

The current history index written at `state/history/api/v1/index.json` is now a
structured runtime ledger with:

- `api_version`
- `minor_line`
- `history_retention`
- `latest_updated_unix_ms`
- `oldest_updated_unix_ms`
- `lines[]`
- `entries[]`

Treat it as the machine-facing summary for the current on-disk history shelf.

The packaged Linux tree remains:

- `/usr/share/gewyvern/dsl`
- `/usr/share/gewyvern/protocols`
- `/usr/share/gewyvern/package-compat.toml`
- `/usr/share/gewyvern/examples/gewyvern.toml.example`

That packaged share tree is still considered authoritative for installed
read-only built-ins.

`package-compat.toml` is the installed artifact's read-only layout marker. The
current package contract records:

- the release line, such as `v0.17.x`
- the package version and package release
- the layout and config schema versions
- the packaged share, protocol registry, DSL, and example-config paths
- the legacy compatibility root
- the upgrade policy, currently `copy-forward-without-overwrite`

Runtime code should treat this file as an observation point, not as mutable
state. Operator-owned config still belongs in the standard config root, usually
as a copied and edited version of:

- `/usr/share/gewyvern/examples/gewyvern.toml.example`

## Environment Overrides

The runtime currently honors these explicit overrides:

- `GEWY_CONFIG_HOME`
- `GEWY_DATA_HOME`
- `GEWY_STATE_HOME`
- `GEWY_CACHE_HOME`
- `GEWY_CERTIFICATE_ROOT`
- `GEWY_TRUST_ROOT`
- `GEWY_AUTHORITY_ROOT`
- `GEWY_IDENTITY_ROOT`
- `GEWY_CERTIFICATE_STATE_ROOT`
- `GEWY_SHARE_ROOT`
- `GEWY_PROTOCOL_REGISTRY_ROOT`

Priority rule:

1. explicit path override
2. standard `0.15.x` root
3. legacy compatibility root
4. packaged read-only share root
5. repo-local development root when running from source

## Legacy Compatibility

Earlier local setups often treated one directory as the whole app root:

- `~/.gewyvern/`

The `0.15.x` line should continue to recognize that shape as a fallback for
upgrade safety, especially:

- `~/.gewyvern/protocols`
- `~/.gewyvern/dsl`

Compatibility rule:

- if a new standard root exists, prefer it
- if only the legacy root exists, continue reading it
- startup may copy legacy mutable content forward into the new standard roots
  when the target path is still missing
- startup must not overwrite content that already exists in the new standard
  roots

## Upgrade Guidance From Older Local Installs

Recommended conservative upgrade path:

1. keep the old `~/.gewyvern/` tree intact
2. create the new standard roots for the host OS
3. copy mutable operator-owned content first:
   - custom protocol packages
   - local DSL shelves
   - future config files once they are formalized
4. beginning with `0.15.x`, startup performs a conservative copy-forward for:
   - `~/.gewyvern/config.toml` to the standard `gewyvern.toml`
   - `~/.gewyvern/protocols/` to `data/protocols/`
   - `~/.gewyvern/dsl/` to `data/dsl/`
5. that copy-forward is additive only:
   - missing targets are created
   - existing standard-path files are preserved
6. leave packaged built-ins under the installed share root
7. only remove old content after the new runtime has been validated against the
   copied tree

## Why This Matters

This split is not cosmetic.

It gives the project a safer base for:

- future config files
- latest-snapshot persistence
- later historical snapshot retention
- local cache eviction rules
- packaged upgrades
- operator troubleshooting

Without a standard layout, every later minor line would have to renegotiate
where its mutable files belong.

## Protocol Surface Persistence Note

The latest-snapshot tree now persists protocol-surface artifacts that may carry
structured drill-down hints in addition to the canonical family/entry pair.

Watch these files when validating control-plane consumers:

- `latest/api/v1/latest/targets/<path-segment>/protocol-surface.json`
- `latest/api/v1/latest/protocols/<protocol>/entries/<entry>/surface.json`

In the current `1.4.0` line those JSON artifacts may include:

- `selected_overlay`
- `overlays`
- `reading_companions`

That means a downstream UI does not need to rediscover relationships such as:

- `https connect` -> `tls client`
- `http3 request` -> `quic initial`
- `tls client` -> `https connect`

The persistence layer is expected to preserve those fields exactly as emitted by
the live API so offline review and history snapshots keep the same reading
contract.
