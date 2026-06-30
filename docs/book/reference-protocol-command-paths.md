# Reference: Protocol Command Paths

Use this page when you already know the protocol family and want the shortest
command path to a real runtime check.

This page sits between the protocol reference shelf and the operator-facing CLI
recipes.

It exists so protocol reviewers can move from:

- family contract
- example DSL
- runtime command

without rebuilding the route from memory every time.

Read this alongside:

- [docs/book/reference-protocol-reading-paths.md](docs/book/reference-protocol-reading-paths.md)
- [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)
- [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
- [docs/book/reference-protocol-release-handbook.md](docs/book/reference-protocol-release-handbook.md)
- [docs/cli-recipes.md](docs/cli-recipes.md)

## How To Use This Page

Use this page when the question is:

- “what is the first command I should run for this family?”
- “which direct CLI path matches the family hub I am reading?”
- “how do I jump from shelf-level reference into `cargo run -- ...` quickly?”

The normal route is:

1. family hub page
2. one direct command below
3. one validation shelf if broader confidence is needed

## High-Frequency Families

### HTTP

- Hub:
  [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
- First command:

```bash
cargo run -- --protocol http --entry request --json --summary-only
```

### HTTPS

- Hub:
  [docs/book/reference-https-surface.md](docs/book/reference-https-surface.md)
- First command:

```bash
cargo run -- --protocol https --entry connect --json --summary-only
```

### TLS

- Hub:
  [docs/book/reference-tls-surface.md](docs/book/reference-tls-surface.md)
- First command:

```bash
cargo run -- --protocol tls --entry client --json --summary-only
```

### DNS

- Hub:
  [docs/book/reference-dns-surface.md](docs/book/reference-dns-surface.md)
- First command:

```bash
cargo run -- --protocol dns --entry udp --json --summary-only
```

### SSH

- Hub:
  [docs/book/reference-ssh-surface.md](docs/book/reference-ssh-surface.md)
- First command:

```bash
cargo run -- --protocol ssh --entry session --json --summary-only
```

### SOCKS5

- Hub:
  [docs/book/reference-socks5-surface.md](docs/book/reference-socks5-surface.md)
- First command:

```bash
cargo run -- --protocol socks5 --entry session --json --summary-only
```

### PostgreSQL

- Hub:
  [docs/book/reference-postgres-surface.md](docs/book/reference-postgres-surface.md)
- First command:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
```

### MySQL

- Hub:
  [docs/book/reference-mysql-surface.md](docs/book/reference-mysql-surface.md)
- First command:

```bash
cargo run -- --protocol mysql --entry session --json --summary-only
```

### QUIC

- Hub:
  [docs/book/reference-quic-surface.md](docs/book/reference-quic-surface.md)
- First command:

```bash
cargo run -- --protocol quic --entry initial --json --summary-only
```

### HTTP/3

- Hub:
  [docs/book/reference-http3-surface.md](docs/book/reference-http3-surface.md)
- First command:

```bash
cargo run -- --protocol http3 --entry request --json --summary-only
```

## Broad Runtime Commands

Use these when the question is larger than one family:

```bash
cargo run -- --list-protocols
cargo run -- --scan-all --json --summary-only
```

Use this when you want one broad scan plus an operator-facing API:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only
```

## Serve And API Checks

Once `--serve` is active, these checks are the shortest practical probes:

```bash
curl http://127.0.0.1:9100/health
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
```

This is the right layer when the real question is:

- “does the process answer like a runtime, not just like a parser?”
- “did the latest scan surface become reachable through the API?”
- “can I connect this family-level review to live runtime evidence?”

## Shortest Practical Routes

- Contract -> direct command:
  family hub -> one command above
- Contract -> example -> direct command:
  family hub -> [docs/book/reference-protocol-example-paths.md](docs/book/reference-protocol-example-paths.md)
  -> one command above
- Contract -> validation shelf:
  family hub -> [docs/book/reference-protocol-validation-paths.md](docs/book/reference-protocol-validation-paths.md)
- Contract -> served runtime:
  family hub -> one direct command above -> `--serve` API checks
