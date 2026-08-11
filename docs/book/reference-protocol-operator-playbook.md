# Reference: Protocol Operator Playbook

Use this page when you care less about protocol taxonomy and more about what
to run next as an operator or release reviewer.

This page turns the protocol reference shelf into a small operational playbook
for the active `1.14.x` line.

Read this alongside:

- [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)
- [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)
- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/release-checklist.md](docs/release-checklist.md)

## When To Use This Page

Use this page when the question is:

- “which protocol check should I run first?”
- “is this a family-local issue or a whole-runtime issue?”
- “what is the shortest route from protocol suspicion to release confidence?”

## Fastest Decision Ladder

Run these in order:

1. one direct family command
2. one broad registry check
3. one high-frequency grouped validation
4. one live `--serve` check if operator trust matters
5. one container or stack check if release trust matters

That order is deliberate:

- family command answers “is one path alive?”
- registry answers “did the built-in shelf drift?”
- grouped validation answers “did important traffic families drift?”
- `--serve` answers “does this behave like a runtime shell?”
- container or stack checks answer “would I trust this outside the source tree?”

## Family-First Triage

### Web And Encrypted Edge

Families:

- [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
- [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
- [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
- [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
- [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)

First commands:

```bash
cargo run -- --protocol http --entry request --json --summary-only
cargo run -- --protocol https --entry connect --json --summary-only
cargo run -- --protocol tls --entry client --json --summary-only
cargo run -- --protocol http3 --entry request --json --summary-only
cargo run -- --protocol quic --entry initial --json --summary-only
```

Use this branch when the symptom looks like:

- request/response drift
- handshake or encrypted edge drift
- HTTP/3 and QUIC path disagreement

### Name Resolution And Access Edge

Families:

- [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
- [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
- [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)

First commands:

```bash
cargo run -- --protocol dns --entry udp --json --summary-only
cargo run -- --protocol ssh --entry session --json --summary-only
cargo run -- --protocol socks5 --entry session --json --summary-only
```

Use this branch when the symptom looks like:

- routing, name-resolution, or tunnel drift
- session/open/connect posture drift
- “the runtime is up, but access paths look wrong”

### Management UDP Control Families

Families:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)
- [docs/book/reference-management-udp-role-matrix.md](docs/book/reference-management-udp-role-matrix.md)
- [docs/book/reference-management-udp-diagnosis-matrix.md](docs/book/reference-management-udp-diagnosis-matrix.md)
- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)
- [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md)
- [docs/book/reference-dhcp-surface.md](docs/book/reference-dhcp-surface.md)
- [docs/book/reference-stun-surface.md](docs/book/reference-stun-surface.md)

First commands:

```bash
cargo run -- --protocol snmp --entry get --json --summary-only
cargo run -- --protocol ntp --entry query --json --summary-only
cargo run -- --protocol dhcp --entry request --json --summary-only
cargo run -- --protocol stun --entry binding --json --summary-only
```

Use this branch when the symptom looks like:

- probe/request datagram seen but reply missing
- a control-plane result packet should be kept distinct from timeout
- an explicit denial or report packet may matter more than generic packet loss

### Stateful Data Paths

Families:

- [docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
- [docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md)

First commands:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
cargo run -- --protocol mysql --entry session --json --summary-only
cargo run -- --protocol redis --entry ping --json --summary-only
```

Use this branch when the symptom looks like:

- query/session drift
- stateful request semantics drift
- data-plane package behavior moving independently of the rest of the tree

## Broad Shelf Confidence

Once one family command gives you a signal, step outward:

```bash
cargo run -- --list-protocols
cargo run -- --scan-all --json --summary-only
bash scripts/validation/registry_validation.sh
bash scripts/validation/high_frequency_validation.sh
```

Key grouped checks:

- [scripts/validation/registry_validation.sh](scripts/validation/registry_validation.sh)
- [scripts/validation/high_frequency_validation.sh](scripts/validation/high_frequency_validation.sh)

Use this layer to answer:

- is the break local to one family?
- is the registry still coherent?
- did only the important shelf drift?

## Live Runtime Confidence

Use this when the source-tree contract is not enough:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/health
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
```

Use this layer when the real question is:

- “is it serving like a runtime?”
- “did operator-facing JSON stay reachable?”
- “does the latest snapshot still look coherent?”

When the selected protocol surface exposes `reading_companions`, prefer those
structured jumps before inventing your own detour:

- `https connect` should usually send you into `tls client`
- `http3 request` should usually send you into `quic initial`
- `tls client` should usually send you into `https connect` or `dns tcp`

For the exact machine-facing contract, keep nearby:

- [docs/book/reference-protocol-reading-companions.md](docs/book/reference-protocol-reading-companions.md)

## Release Confidence

Use this when you are deciding whether to ship:

```bash
cargo run --quiet --bin gewyvern_validate -- release-gate
cargo run --quiet --bin gewyvern_validate -- release-container-check
bash scripts/validation/three_module_stack_smoke.sh
```

Key scripts:

- [scripts/packaging/release_gate.sh](scripts/packaging/release_gate.sh)
- [scripts/packaging/release_container_check.sh](scripts/packaging/release_container_check.sh)
- [scripts/validation/three_module_stack_smoke.sh](scripts/validation/three_module_stack_smoke.sh)

Use this layer when the real question is:

- “would I trust the packaged runtime?”
- “does the three-module topology still hold?”
- “is the protocol shelf good enough for a release judgment?”

## Shortest Practical Routes

- One family looks wrong:
  family hub -> one direct command -> `registry_validation.sh`
- Important traffic families look wrong:
  one direct command -> `high_frequency_validation.sh`
- Operator surface looks wrong:
  one direct command -> `--serve` -> `/health` and `summary.json`
- Release confidence is in doubt:
  one direct command -> `gewyvern_validate release-gate` -> `three_module_stack_smoke.sh`
