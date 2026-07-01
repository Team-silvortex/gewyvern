# Stack Applications

This repository now carries the nearby application stack in one place so
`gewyvern`, `leserpent`, and `etragon` can evolve together without losing their
separate roles.

Current app shelves:

- `apps/etragon`
  Diagnosis-partner sidecar for one nearby `gewyvern` runtime.
- `apps/leserpent`
  Cross-platform control plane for fleets of nearby runtimes and paired
  sidecars.

Versioning rule:

- companion applications follow the root workspace release line
- do not assign independent `etragon` or `leserpent` product versions unless
  the repository deliberately re-splits their release cadence later

Working rule:

- keep runtime/core protocol work in the repository root `gewyvern` crate
- keep companion applications under `apps/`
- only promote shared code upward when the boundary is stable enough to deserve
  it

Common local entrypoints from the repository root:

- `cargo run -- --scan-all --json --summary-only`
  Run the main `gewyvern` runtime surface.
- `cargo run -p etragon -- --help`
  Run the nearby diagnosis-partner sidecar CLI.
- `cd apps/leserpent && npm run check:frontend`
  Type-check the `leserpent` dashboard frontend.
- `dotnet build apps/leserpent/src/Leserpent/Leserpent.csproj`
  Build the `leserpent` control plane backend.
