# Tutorial: First Real Run

This tutorial is the shortest path from “I have the repository” to “I can run
`gewyvern` on purpose and understand the result”.

It is intentionally narrow:

- one repository
- one CLI
- one report path
- one reading strategy

If you are starting from the native Hub instead, use
[Your first Leserpent Desktop session](tutorial-leserpent-desktop.md). Its
offline Learning Center and this CLI tutorial teach the same conservative
diagnosis boundary from different product surfaces.

## Prerequisites

- the repository root as your working directory
- a Rust toolchain accepted by the workspace lock
- no root privileges, daemon, account, or network service

Confirm the active product line before the first run:

```bash
cargo run --quiet -- --version
```

## What You Will Do

By the end of this tutorial, you will have:

1. listed the built-in protocol families
2. run one focused protocol path
3. run a broader sweep
4. rendered an HTML report
5. learned the minimum set of fields to read first
6. seen when to use serve/API and socket ingest

## Step 1: Discover What Is Built In

Start by asking the runtime what protocol families and entries it already
knows.

```bash
cargo run -- --list-protocols
cargo run -- --list-entries quic
```

This confirms two things up front:

- `gewyvern` is registry-driven, not a pile of hard-coded demos
- built-in protocol paths are concrete entries such as `request`, `session`,
  `query`, or `auth`

**Checkpoint:** `quic` lists `initial` as its default entry and exposes its
other bounded entry names.

## Step 2: Run One Focused Path

Pick one protocol family and one entry. A good first example is PostgreSQL:

```bash
cargo run -- --protocol postgres --entry query --json --summary-only
```

This is the fastest useful operator surface.

Read these fields first:

- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`

That is the current narrow diagnosis spine.

### How To Read It

- `confidence=high`
  usually means a direct protocol signal such as a denied or explicit error
- `confidence=medium`
  usually means a missing transition is being summarized conservatively
- `confidence=low`
  means the runtime is intentionally avoiding overclaiming

**Checkpoint:** the top-level result is `kind=single`, and the primary module is
`database_query`.

## Step 3: Render The Same Target As HTML

Now render the same kind of run as a report for a human reader:

```bash
cargo run -- --protocol http3 --entry request --report-format html --out /tmp/http3-request.html
```

This is useful when:

- you want a visual summary card
- you want to share the result with another engineer
- you do not want to read raw JSON first

The HTML report is not the narrowest machine-facing contract, but it is a good
operator-facing surface.

Confirm that the output is non-empty before sharing it:

```bash
test -s /tmp/http3-request.html
```

## Step 4: Run A Full Sweep

Once the focused path makes sense, run the broader built-in sweep:

```bash
cargo run -- --scan-all --json --summary-only
```

Use this when:

- you do not yet know which protocol path matters
- one process may participate in multiple network modules
- you want the runtime to rank likely problem areas

Read these parts first:

- top-level `kind`
- top-level `target_count`
- `targets[*].primary_module_kind`
- `targets[*].primary_failure_mode`
- `targets[*].primary_failure_confidence`

## Step 5: Scope To One Process When Needed

If you already know the process you care about, narrow the sweep:

```bash
cargo run -- --scan-all --pid 4242 --json --summary-only
```

Replace `4242` with a process ID you are authorized to inspect. A missing or
inaccessible process is a failed prerequisite, not evidence about that process.

This is usually the right move when the question is:

- where is this one process stuck?
- is it blocked in DNS, connect, auth, relay, or request/response?

The most useful block is often:

- `process_network_profiles`

That view compresses several matched protocol flows into one process-oriented
network picture.

## Step 6: Know Which Surface To Read

At this stage, the most important judgment is which output surface you should
consume.

Use:

- `summary.json`
  for one conservative conclusion per target
- `analysis.json`
  for machine-facing automation or sidecar integration
- HTML report
  for human-oriented sharing and inspection

If you are using the serve/API path later, this distinction still holds.

## Step 7: Use Serve/API And Socket Ingest When Needed

Once the focused and sweep paths feel normal, the next useful operator shape is
the local service/API surface.

If facts are arriving over a local socket producer:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --summary-only --report-format html --out /tmp/process-scan.html
```

If another local service needs the latest result over HTTP:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --serve --api-socket 127.0.0.1:9100 --json --summary-only
curl http://127.0.0.1:9100/v1/capabilities
curl http://127.0.0.1:9100/v1/latest/targets
curl http://127.0.0.1:9100/v1/latest/summary.json
curl http://127.0.0.1:9100/v1/latest/analysis.json
```

Read this combination carefully:

- `ingest_mode`
- `ingest_trust_mode`
- `pid_attribution_status`
- `primary_failure_confidence`
- `primary_failure_basis`

That tells you both what the runtime thinks and how directly the input supports
that conclusion.

Important boundary:

- `--pid` is intentionally rejected with socket ingest
- `local-advisory` and `remote-advisory` are operator-facing run modes, not
  proof of trust
- process-scoped conclusions in this mode should be read as advisory

## Completion Checkpoint

The current `v1.20.x` line keeps compatibility promises around its documented
stable surfaces while leaving explicitly evolving extensions room to mature.

That means:

- the CLI should be coherent
- the report surfaces should be coherent
- the diagnosis spine should be readable
- the documentation should be enough to get another engineer moving

You have completed the tutorial when you can choose a focused run or full sweep,
locate the conservative diagnosis spine, and explain why confidence and input
trust must be read together.

For the broader ship/no-ship posture, see the
[release checklist](../release-checklist.md).

## Where To Go Next

- If you want to understand the runtime pipeline:
  [Gewy to runtime](explanation-gewy-to-runtime.md)
- If you want the full runtime validation ladder:
  [Validate the runtime surface](how-to-validate-runtime-surface.md)
- If you want to start authoring `gewylang`:
  [Your first GewyLang package](tutorial-gewylang-package.md)
- If you want the native multi-daemon client:
  [Your first Leserpent Desktop session](tutorial-leserpent-desktop.md)
