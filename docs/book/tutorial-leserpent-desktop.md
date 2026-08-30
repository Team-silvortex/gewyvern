# Tutorial: Your First Leserpent Desktop Session

This tutorial takes you from a source checkout to a safe, useful Leserpent Hub
session. It does not require a Team Silvortex account, a subscription, or a
remote machine.

## What You Will Do

By the end, you will have:

1. launched the native desktop client
2. identified the client, daemon-authority, and runtime layers
3. inspected Local Orchestra without entering a credential
4. refreshed topology and interpreted live, stale, and unavailable states
5. opened a runtime workspace when one is available
6. found the diagnostic and canonical Leselang surfaces

## Prerequisites

- macOS or desktop Linux
- the repository root as your working directory
- Rust and .NET 10 available to the native developer workflow

Check the host before building:

```bash
cargo dev doctor
cargo dev version check
```

Stop if `doctor` reports a missing required toolchain. Do not work around a
failed locked restore with an unlocked package update during this tutorial.

## Step 1: Build And Launch

On macOS, install the current development app atomically into Applications and
launch it:

```bash
cargo dev deploy desktop --launch
```

For a source-tree session on macOS or Linux:

```bash
cargo dev build --scope desktop
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  --no-build
```

The normal entry has no arguments. A window that immediately asks only for a
remote endpoint is not the current product entry; the expected first screen is
the Hub.

**Checkpoint:** the Hub opens and remains usable without sign-in.

## Step 2: Read The Topology Before Acting

Open `Quick tour`, press F1, or choose `Learning Center...` from the macOS
application menu. All three routes open the same offline tutorial. The first
model to retain is:

```text
Leserpent client -> leserpentd authority -> Gewyvern runtime
```

- The Hub root is this client session.
- Each daemon card is an independent authority with its own service and Web
  surface.
- Each runtime child stays under the daemon that owns it.
- Closing one daemon window does not close the Hub or another daemon session.

Sign-in is optional and reserved for future hosted services. All current local,
self-hosted, and remote core workflows remain available without it.

**Checkpoint:** you can point to the client root and explain why a runtime never
silently moves between daemon cards.

## Step 3: Inspect Local Orchestra

On a non-mobile desktop, Leserpent starts an app-owned loopback `leserpentd`
session called Local Orchestra. It is independent of Gewyvern and may be healthy
even when it owns no runtimes.

Read the card before opening it:

- `LIVE` means the health proof and topology are authoritative.
- `CACHED` or retained data is intentionally stale and cannot authorize a
  mutation.
- `UNAVAILABLE` means the client could not establish current authority.
- queue pressure is an authority signal, not a cosmetic warning.

Choose `Open` only after the card reaches a terminal visible state. If Local
Orchestra did not start, use the visible recovery message rather than manually
starting a second daemon against its app-owned database.

**Checkpoint:** Local Orchestra is visible, and you can distinguish daemon
health from runtime availability.

## Step 4: Refresh And Filter The Fleet

Choose `Refresh all` or press F5. Repeated refresh requests join the current
single-flight operation instead of starting duplicate network work.

Use the topology search field to filter by daemon ID, runtime name, runtime ID,
tag, or status. `Cmd+F` on macOS and `Ctrl+F` elsewhere focuses search. Escape
clears an active filter before it can close the Hub.

Filtering is local presentation only. It does not query a daemon, change the
saved snapshot, or make stale data authoritative.

**Checkpoint:** the refresh summary is terminal, and clearing the filter restores
the same authority/runtime ownership tree.

## Step 5: Add An Existing Authority Only When Needed

Skip this step when Local Orchestra is enough. `+ Add daemon` is for an existing
authenticated HTTPS service and requires:

- a root HTTPS origin
- the reviewed CA for that exact endpoint
- an endpoint-scoped bearer credential stored through the platform vault

Use the connection test before saving. It is read-only and must succeed against
the same origin and trust anchor that will be persisted. Never place a bearer
token in the profile name, endpoint URL, CA file, screenshot, or repository.

`Manage` can inspect trust identity, replace a managed CA, forget an endpoint-
scoped credential, or remove a saved authority. An environment-provided token
is not silently deleted by the application.

**Checkpoint:** either Local Orchestra or one saved daemon card has a current
health result. No remote authority is required to finish this tutorial.

## Step 6: Open A Runtime Workspace

If an authority owns a runtime, refresh it to `LIVE`, then choose the runtime
child. Leserpent waits for an authoritative snapshot at least as new as the Hub
topology before opening the native workspace. A heartbeat, cache entry, removed
runtime, or another daemon's state cannot release that fence.

Read these areas first:

1. identity and owning daemon
2. current revision and snapshot state
3. observed capabilities
4. bounded command history and logs
5. severity and change summary

Use local log search and level filters before exporting diagnostics. Diagnostic
export is explicit, bounded, and excludes the endpoint and bearer credential.

If there are no runtimes, the tutorial still succeeds at the previous
checkpoint. Continue with the [remote deployment lab](tutorial-remote-deployment-lab.md)
only when you intentionally want to create one.

## Step 7: Find The Equivalent Automation

In a runtime workspace, `Workspace Leselang` shows the canonical representation
of the same typed query or mutation. Form previews update only while their
values are valid. `Copy Leselang` copies source; it does not execute it.

Mutation controls remain disabled until shared policy proves current authority,
capability, revision, and confirmation. A disabled button is evidence to read,
not a frontend obstacle to bypass.

For a complete local walkthrough, continue with
[First Leselang GUI automation](tutorial-leselang-gui-automation.md).

## Completion Checkpoint

You have completed the tutorial when you can explain all four statements:

- Desktop is a client, not the control authority.
- Every runtime operation remains bound to one daemon and one revision.
- Cached topology can be read but cannot authorize a mutation.
- Canonical Leselang is an equivalent typed path, not a hidden privileged API.

## Verify The Tutorial Surface

These offline product probes exercise the real Hub, Learning Center, remote
shell, localization, and accessibility controls without opening a network
client:

```bash
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-desktop-tutorial
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-hub-topology
dotnet run --project \
  apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -- --verify-remote-shell-controls
```

## Where To Go Next

- [First Leselang GUI automation](tutorial-leselang-gui-automation.md)
- [Remote deployment lab](tutorial-remote-deployment-lab.md)
- [Leserpent GUI function chains](../leserpent-gui-function-chains.md)
- [Desktop implementation reference](../../apps/leserpent-avalonia/README.md)
