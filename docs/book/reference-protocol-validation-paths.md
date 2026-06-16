# Reference: Protocol Validation Paths

Use this page when you already know which protocol family you care about and
want the shortest route to the right validation command.

This page sits between the protocol reference shelf and the script shelf.

It exists so reviewers do not have to translate:

- family hub pages
- high-frequency protocol expectations
- runtime validation scripts

by memory every time.

Read this alongside:

- [docs/book/reference-protocol-reading-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-reading-paths.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/book/reference-protocol-example-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-example-paths.md)
- [docs/book/reference-protocol-command-paths.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-command-paths.md)
- [docs/book/reference-protocol-operator-playbook.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-operator-playbook.md)

## Core Rule

Use these layers in order:

1. one family hub page for exact contract and entry identity
2. one direct CLI/runtime command for the narrow path
3. one script shelf for grouped confidence
4. container or stack smoke when release confidence matters

## Family To Validation Shelf

### HTTP, HTTPS, TLS

Families:

- [docs/book/reference-http-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http-surface.md)
- [docs/book/reference-https-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-https-surface.md)
- [docs/book/reference-tls-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-tls-surface.md)

Fastest grouped validation:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)

Useful direct commands:

```bash
cargo run -- --protocol http --entry request --json --summary-only
cargo run -- --protocol https --entry connect --json --summary-only
cargo run -- --protocol tls --entry client --json --summary-only
```

### DNS

Family:

- [docs/book/reference-dns-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-dns-surface.md)

Fastest grouped validation:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)

Useful direct command:

```bash
cargo run -- --protocol dns --entry udp --json --summary-only
```

### SSH And SOCKS5

Families:

- [docs/book/reference-ssh-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ssh-surface.md)
- [docs/book/reference-socks5-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-socks5-surface.md)

Fastest grouped validation:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)

Useful direct commands:

```bash
cargo run -- --protocol ssh --entry session --json --summary-only
cargo run -- --protocol socks5 --entry session --json --summary-only
```

### PostgreSQL And MySQL

Families:

- [docs/book/reference-postgres-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-postgres-surface.md)
- [docs/book/reference-mysql-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-mysql-surface.md)

Fastest grouped validation:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)

Useful direct commands:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
cargo run -- --protocol mysql --entry session --json --summary-only
```

### QUIC And HTTP/3

Families:

- [docs/book/reference-quic-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-quic-surface.md)
- [docs/book/reference-http3-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-http3-surface.md)

Fastest grouped validation:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)

Useful direct commands:

```bash
cargo run -- --protocol quic --entry initial --json --summary-only
cargo run -- --protocol http3 --entry request --json --summary-only
```

### Broad Protocol Shelf Confidence

Use this when the question is broader than one family:

- [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)
- [scripts/validation/runtime_operator_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/runtime_operator_validation.sh)

Use this when the question is “do the important families still behave?”:

- [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)

### Release And Cross-Project Confidence

Use these when source-tree checks are not enough:

- [scripts/packaging/release_container_check.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh)
- [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)
- [scripts/packaging/container_validation_summary.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh)

This is the right layer when the real question is:

- “would I trust this for a release?”
- “does packaged behavior still match source-tree expectations?”
- “does the `gewyvern + etragon + leserpent` stack still hold together?”

## Shortest Practical Routes

- Family contract -> runtime confidence:
  family hub -> [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
  -> relevant script above
- Family contract -> package drift review:
  family hub -> [scripts/validation/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh)
- High-frequency confidence:
  family hub -> [scripts/validation/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh)
- Release confidence:
  family hub -> [scripts/packaging/release_container_check.sh](/Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh)
  -> [scripts/validation/three_module_stack_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh)
