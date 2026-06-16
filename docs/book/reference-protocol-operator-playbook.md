# Reference: Protocol Operator Playbook

Use this page when you care less about protocol taxonomy and more about what
to run next as an operator or release reviewer.

This page turns the protocol reference shelf into a small operational playbook
for the current `0.15.x` line.

Read this alongside:

- [docs/book/reference-protocol-command-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-command-paths.md)
- [docs/book/reference-protocol-validation-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-validation-paths.md)
- [docs/book/reference-protocol-release-handbook.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-release-handbook.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/release-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/release-checklist.md)

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

- [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
- [docs/book/reference-https-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-https-surface.md)
- [docs/book/reference-tls-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-surface.md)
- [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)
- [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)

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

- [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)
- [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)
- [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)

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

### Stateful Data Paths

Families:

- [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)
- [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)

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
bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh
```

Key grouped checks:

- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)
- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)

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

## Release Confidence

Use this when you are deciding whether to ship:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh
bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh
```

Key scripts:

- [scripts/packaging/release_gate.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh)
- [scripts/packaging/release_container_check.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh)
- [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)

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
  one direct command -> `release_gate.sh` -> `three_module_stack_smoke.sh`
