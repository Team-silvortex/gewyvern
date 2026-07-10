# Headless Linux For eBPF

This project can keep most runtime and export work on macOS, but real eBPF
attach and ringbuf work should be validated in Linux.

The lightest path for this repository is a headless Linux container running
inside Docker Desktop's Linux VM.

## What This Gives Us

- a Linux kernel context instead of Darwin
- a reproducible shell for clang/libbpf/Rust work
- a place to prototype loader and attach behavior without adding a GUI VM

## What This Does Not Give Us

- direct access to the macOS host kernel
- production parity with a real bare-metal Linux host
- guaranteed support for every eBPF feature on every Docker Desktop build

Treat this as the first Linux landing zone, not the final validation target.

## Role In The Shelf

Treat this page as the Linux eBPF bring-up shelf.

Use it when the question is:

- how do I get into a Linux context for real eBPF attach work?
- what should I run first inside the headless Linux container?
- what are the limits of this Docker-based Linux path?

Do not use this page as:

- the generic contributor workflow guide
- the packaging validation shelf
- the short command index

For those, use:

- [docs/development.md](docs/development.md)
- [docs/packaging.md](docs/packaging.md)
- [docs/cli-recipes.md](docs/cli-recipes.md)

## Companion Shelves

- [docs/development.md](docs/development.md)
  for the broader contributor workflow
- [docs/packaging.md](docs/packaging.md)
  when the question shifts from Linux bring-up to native artifacts
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
  for the shortest goal-based routing into Linux or packaging scripts

## Prerequisite

Docker Desktop needs to be running.

On this machine, `docker` exists, but the daemon was not running when this doc
was written, so the container flow could not yet be exercised end-to-end.

## Build The Image

From the repository root:

```bash
docker compose -f docker-compose.headless-linux.yml build
```

## Start The Headless Linux Shell

```bash
docker compose -f docker-compose.headless-linux.yml up -d
docker compose -f docker-compose.headless-linux.yml exec ebpf-dev bash
```

The repository is mounted at `/workspace`.

## First Commands Inside Linux

Verify the kernel and toolchain:

```bash
uname -a
clang --version
cargo --version
bpftool version
```

Run the current test suites:

```bash
cargo tdd-rules
cargo tdd
cargo test --workspace
```

Run the native Linux smoke commands when the container has enough privilege:

```bash
sudo cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke
sudo cargo run --quiet --bin gewyvern_validate -- linux-kprobe-smoke
sudo cargo run --quiet --bin gewyvern_validate -- linux-tc-smoke --dev eth0
```

Each smoke writes `run.log`, `target.txt`, `environment.txt`, and
`evidence-index.json` under `target/validation/...`; the tc path also writes
`netdev.txt` so interface state is captured next to the attach transcript.

## Recommended Next eBPF Milestones

1. Add a loader-facing TDD spec for attach outcomes and ringbuf wiring.
2. Introduce a minimal Linux-only smoke path that attempts one tracepoint
   attach.
3. Export real attach failures from the Linux path instead of injected failures.
4. Add a replay fixture captured from Linux so macOS-side development can stay
   deterministic.

## Notes About Privilege

The compose file runs the container with elevated Linux capabilities because
eBPF development usually needs privileged access to kernel facilities such as:

- `/sys/fs/bpf`
- `/sys/kernel/debug`
- host PID namespace visibility

If Docker Desktop tightens those mounts on a future build, the next fallback is
to use a dedicated headless Linux VM.
