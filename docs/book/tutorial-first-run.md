# Tutorial: First Real Run

This tutorial is the shortest path from “I have the repository” to “I can run
`gewyvern` on purpose and understand the result”.

It is intentionally narrow:

- one repository
- one CLI
- one report path
- one reading strategy

If you want broader command examples, see
[docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md).
If you want the lower-level compiler/runtime story, see
[docs/walkthrough.md](/Users/Shared/chroot/dev/gewyvern/docs/walkthrough.md).

## What You Will Do

By the end of this tutorial, you will have:

1. listed the built-in protocol families
2. run one focused protocol path
3. run a broader sweep
4. rendered an HTML report
5. learned the minimum set of fields to read first

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

## Step 7: What “Good Enough To Start Using” Means

The current `v0.13.0` line is not claiming final forever-frozen `1.0`
stability, but it is already meant to be usable on purpose.

That means:

- the CLI should be coherent
- the report surfaces should be coherent
- the diagnosis spine should be readable
- the documentation should be enough to get another engineer moving

For the broader release posture, see
[docs/v0.13-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.13-posture.md).

## Where To Go Next

- If you want more operator examples:
  [docs/examples.md](/Users/Shared/chroot/dev/gewyvern/docs/examples.md)
- If you want to understand the runtime pipeline:
  [docs/walkthrough.md](/Users/Shared/chroot/dev/gewyvern/docs/walkthrough.md)
- If you want to start authoring `gewylang`:
  [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
