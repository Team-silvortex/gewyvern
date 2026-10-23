# Stack Applications

<p align="center">
  <img src="../docs/assets/branding/gewyvern-logo-v1.svg" alt="gewyvern logo" width="140">
</p>

This directory contains replaceable clients, a compatibility bridge, and the
optional advisory application. The control-plane authority is Rust
[`leserpentd`](../crates/leserpentd), not an application under this directory.
Use the [repository map](../docs/monorepo-stack.md) for complete source ownership.

## Application Shelves

| Application | Role |
| --- | --- |
| [Avalonia desktop](leserpent-avalonia/README.md) | Native multi-daemon hub and renderer |
| [Mobile](leserpent-mobile/README.md) | Mobile renderer using the same authority protocol |
| [Web compatibility bridge](leserpent/README.md) | ASP.NET compatibility and TypeScript browser rendering; no separate control authority |
| [Etragon](etragon/README.md) | Optional advisory sidecar; never the owner of evidence or control truth |

One client may manage many independent daemons; each daemon manages many
Gewyvern services. Shared commands, authorization, persistence, and effects live
in the Rust crates. UI work belongs here; language work belongs in the language
crates, not inside a renderer. Companion applications follow the root workspace
release line, while protocol versions describe their own compatibility.

## Local Entrypoints

From the repository root:

```bash
cargo dev doctor
cargo dev check --scope desktop
cargo dev build --scope control
cargo dev package desktop
```

For Web-specific frontend checks, use the
[Web bridge guide](leserpent/README.md). A direct build of its `.csproj` builds
only the compatibility host, not the whole Rust authority stack.

Use the [development guide](../docs/development.md) for focused tests and cache
retention, and [packaging](../docs/packaging.md) for installation workflows.
