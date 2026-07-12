# Announcing gewyvern 1.0.0

Today marks the formal `1.0.0` release of `gewyvern`.

This is the point where the project stops reading like a long convergence
experiment and starts reading like a stable Linux-first debugging system with a
clear operating story.

`gewyvern` is built for protocol-aware local network debugging driven by eBPF
fragments, `gewylang` packages, and deterministic runtime surfaces.
It is not trying to be a general observability platform.
Its strength is a narrower promise: when you need to inspect, reconstruct, and
reason about process-level network behavior on Linux, the core system now has a
stable enough shape to trust, operate, and build around.

## What Reached Stability

With `v1.0.0`, the project now presents one coherent core across:

- the runtime and CLI
- `gewylang` and `gewyc`
- JSON and HTML reporting surfaces
- release validation entrypoints
- lifecycle behavior, persistence, and cleanup
- Linux-host validation and eBPF proof paths

That does not mean the project is "finished."
It means future growth no longer needs to happen inside an unfinished identity.

## What Helped This Release Cross The Line

Several things made `1.0.0` possible:

- the final `v0.20.x` sealing pass across docs, runtime behavior, and release
  posture
- native `gewyvern_validate` entrypoints replacing shell-heavy release habits
- remote Linux host validation becoming a normal part of release confidence
- practical target-lab checks proving the core can capture suspicious and
  rejected traffic patterns, not just happy-path demos
- tighter release evidence across `gewyvern`, `etragon`, and `leserpent`

## What This Release Says About The Project

`gewyvern` is now making a serious claim:

it can serve as real Linux debugging infrastructure for local network and
protocol behavior, with stable operational surfaces and repeatable validation.

That claim is intentionally narrower than Wireshark, mitmproxy, or Burp Suite.
It is also intentionally different.
The center of gravity here is not generic traffic viewing or proxy control; it
is eBPF-grounded, process-aware, protocol-aware local diagnosis with a stable
runtime and machine-readable outputs.

## What Comes After 1.0.0

The next phase is straightforward:

- make `v1.0.x` sturdier
- keep reliability and validation quality high
- reduce operator friction
- improve performance where it matters in real Linux debugging loops
- widen carefully, only when the extension is justified and validated

## Read More

- [README.md](../README.md)
- [docs/index.md](index.md)
- [docs/history/v1.0.0.md](history/v1.0.0.md)
- [docs/history/v1.0.0-release-notes.md](history/v1.0.0-release-notes.md)
