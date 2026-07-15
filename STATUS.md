# Project Status

The machine-readable project status is a sparse
`architecture x module x feature` tensor.

- Protocol and maintenance rules:
  [docs/project-status-system.md](docs/project-status-system.md)
- JSON Schema: [project/status/schema.json](project/status/schema.json)
- Current catalog: [project/status/catalog.json](project/status/catalog.json)

Inspect it through the native Rust command:

```bash
cargo run --bin gewyvern_status -- summary
cargo run --bin gewyvern_status -- weakest
cargo run --bin gewyvern_status -- standalone
cargo run --bin gewyvern_status -- developing
cargo run --bin gewyvern_status -- validate
```

Use `--json` for automation and model context. Do not copy the computed status
into this file; the catalog remains the only status source of truth.
