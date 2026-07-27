# Reference: Protocol Release Handbook For `1.10.x`

Use this page when the question is not just “what command do I run?” but “what
counts as enough protocol confidence to ship the current minor line?”

This page is the protocol-facing release handbook for the active `1.10.x`
line.

Read this alongside:

- [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)
- [docs/field-validation.md](docs/field-validation.md)

## What This Page Is For

Use this page when the question is:

- “what protocol evidence should still exist before we call `1.10.x` green?”
- “which checks are family-local, grouped, operator-facing, or release-facing?”
- “how do I walk from one suspicious protocol family to a minor-line ship read?”

## The `1.10.x` Protocol Gate

Treat the current line as protocol-ready only when all of these stay true:

1. direct high-frequency family commands still resolve and explain cleanly
2. the broad registry shelf still scans cleanly
3. the high-frequency grouped validation shelf still passes
4. live `--serve` still exposes coherent runtime JSON
5. packaged protocol and operator-path validation still pass
6. three-module stack smoke still confirms protocol behavior survives integration

## Minimum High-Frequency Family Checks

These are the shortest family-level checks worth keeping alive in `1.10.x`:

```bash
cargo run -- --protocol http --entry request --json --summary-only
cargo run -- --protocol dns --entry udp --json --summary-only
cargo run -- --protocol ssh --entry session --json --summary-only
cargo run -- --protocol postgres --entry query --json --summary-only
cargo run -- --protocol quic --entry initial --json --summary-only
```

These stand in for the current high-value operator shelf:

- `HTTP / HTTPS / TLS`
- `DNS`
- `SSH / SOCKS5`
- `PostgreSQL / MySQL`
- `QUIC / HTTP/3`

## Grouped Protocol Confidence

Once one direct family path is healthy, confirm the grouped shelves:

```bash
cargo run -- --list-protocols
cargo run -- --scan-all --json --summary-only
bash scripts/validation/registry_validation.sh
bash scripts/validation/high_frequency_validation.sh
```

Key grouped routes:

- [scripts/validation/registry_validation.sh](scripts/validation/registry_validation.sh)
- [scripts/validation/high_frequency_validation.sh](scripts/validation/high_frequency_validation.sh)

Use these to answer:

- is the registry coherent?
- did one family drift, or did the shelf drift?
- do the important operator-facing families still behave?

## Runtime And Operator Confidence

Protocol confidence is not just parser confidence. Keep one served runtime path
in the gate:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/health
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
```

This is the bridge between protocol family confidence and operator trust.

## Packaged And Cross-Project Confidence

The current minor line should not be called healthy from source-tree checks
alone.

Keep these in the release path:

```bash
cargo run --quiet --bin gewyvern_validate -- release-container-check
cargo run --quiet --bin gewyvern_validate -- three-module-stack-smoke
cargo run --quiet --bin gewyvern_validate -- pathological-container-validation
```

Key release routes:

- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [scripts/packaging/release_container_check.sh](scripts/packaging/release_container_check.sh)

Use this layer when the real question is:

- “did packaged protocol behavior survive?”
- “does protocol evidence still survive `gewyvern + etragon + leserpent`?”
- “is this good enough for a `1.10.x` ship read?”

## Shortest Practical Release Routes

- One protocol family looks wrong:
  family hub -> one direct command -> `registry_validation.sh`
- One important traffic class looks shaky:
  one direct command -> `high_frequency_validation.sh`
- Runtime trust looks shaky:
  one direct command -> `--serve` -> `summary.json` -> `analysis.json`
- Release confidence looks shaky:
  `gewyvern_validate release-container-check` -> `three_module_stack_smoke.sh` ->
  `pathological_container_validation.sh`
