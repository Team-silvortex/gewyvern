# Ingest Modes

This guide explains the operator-facing ingest model for `gewyvern`.

It answers a simple question:

- when facts arrive from different sources, how much should you trust the
  resulting process-level conclusions?

The short version is:

- `ingest_mode` tells you what run mode the operator chose
- `ingest_trust_mode` tells you how the runtime classifies that evidence source
- `pid_attribution_status` tells you how seriously to take PID-scoped conclusions

These are related, but they are not the same thing.

## Why This Exists

`gewyvern` can work with:

- synthetic demo facts
- Unix socket ingest
- TCP socket ingest

Those modes do not all have the same attribution strength.

For example:

- a synthetic demo is useful for validating flows and reports
- a local socket producer may still send unverified lineage
- a remote TCP producer is even more clearly an advisory source

So the runtime should not present every result with the same tone.

That is why reports now expose:

- `ingest_mode`
- `ingest_mode_note`
- `ingest_trust_mode`
- `pid_attribution_status`
- `pid_attribution_note`

## The Main Modes

The current operator-facing modes are:

- `demo`
- `local-advisory`
- `remote-advisory`

### `demo`

Use this when:

- you are exercising a built-in path
- you are validating report shape
- you want deterministic sample output

What it means:

- facts are synthetic
- process lineage is synthetic
- conclusions are useful for development and examples, not live host diagnosis

Typical report shape:

- `ingest_mode = demo`
- `ingest_trust_mode = synthetic-demo`
- `pid_attribution_status = synthetic`

### `local-advisory`

Use this when:

- facts arrive over a local Unix or TCP socket
- you want to inspect a real local feed
- you accept that lineage is still unverified

What it means:

- the source is local
- the source is not treated as authenticated
- process-level conclusions should be read as advisory

Typical report shape:

- `ingest_mode = local-advisory`
- `ingest_trust_mode = unverified-local`
- `pid_attribution_status = unverified`

### `remote-advisory`

Use this only when:

- you intentionally want to receive facts from a remote producer
- you understand that remote ingest is explicitly opt-in
- you are comfortable treating the result as unverified advisory evidence

What it means:

- remote TCP listener mode was explicitly enabled
- facts are still treated as unverified
- process-level conclusions should be read even more conservatively

Typical report shape:

- `ingest_mode = remote-advisory`
- `ingest_trust_mode = unverified-remote`
- `pid_attribution_status = unverified`

## Recommended Commands

Local advisory socket ingest:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --summary-only --json
```

Remote advisory socket ingest:

```bash
cargo run -- --scan-all --tcp-socket 0.0.0.0:9000 --ingest-mode remote-advisory --summary-only --json
```

Visual HTML report from local advisory ingest:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --summary-only --report-format html --out /tmp/gewyvern-scan.html
```

## Why `--pid` Is Rejected With Socket Ingest

`--pid` is intentionally rejected when facts come from socket ingest.

Reason:

- the incoming lineage is not currently authenticated
- a strong PID-scoped conclusion would look more certain than the evidence really is

The safer workflow is:

1. run a broader advisory scan
2. inspect `process_network_profiles`
3. use the module and failure summary as a lead
4. only narrow to hard PID attribution when the source is verified

## How To Read Reports Safely

When reading JSON or HTML output, use this order:

1. `ingest_mode`
2. `ingest_mode_note`
3. `ingest_trust_mode`
4. `pid_attribution_status`
5. `pid_attribution_note`
6. `primary_failure_confidence`
7. `primary_failure_basis`

That sequence tells you:

- what the operator asked for
- how the runtime classified the source
- whether PID attribution is strong or advisory
- how direct the failure evidence is

## Legacy Flags

These still work as compatibility aliases:

- `--socket-trust trusted-local|unsafe-remote`
- `--allow-remote-socket`

They are expected to remain as compatibility entrypoints, even as the preferred
public interface stays:

- `--ingest-mode local-advisory`
- `--ingest-mode remote-advisory`

## Mental Model

The safest way to think about socket ingest today is:

- mode tells you how you are operating
- trust tells you how verified the source is
- pid attribution tells you how far to trust per-process claims

If those fields say `advisory`, `unverified`, or `ambiguous`, that is not a
failure of the tool. It is the runtime deliberately refusing to overclaim.

For companion reading:

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md)
- [failure-semantics.md](/Users/Shared/chroot/dev/gewyvern/docs/failure-semantics.md)
