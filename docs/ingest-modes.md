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

## Companion Shelves

Read this page alongside:

- [docs/process-profiles.md](docs/process-profiles.md)
  when you want to see how ingest trust changes the meaning of process-level
  summaries
- [docs/failure-semantics.md](docs/failure-semantics.md)
  when you need the exact failure-language vocabulary that shows up in those
  summaries
- [docs/security-posture.md](docs/security-posture.md)
  when the question shifts from operator interpretation to exposure/trust
  boundaries

## Why This Exists

`gewyvern` can work with:

- synthetic demo facts
- Unix socket ingest
- TCP socket ingest
- explicit Linux cgroup UDP capture

Those modes do not all have the same attribution strength.

Socket input is the live **external-fact** boundary. Explicit
`--capture-cgroup` / `--capture-udp-port` starts the product-owned UDP producer,
including continuous `--serve` consumption. See [live UDP capture](live-udp-capture.md)
for prerequisites, commands, bounded windows, loss accounting and recovery.
`LinuxProbeLoader` still only checks hook availability, and the bounded Linux
field collectors are separate validation tools.
Do not interpret a successful attach plan, an IPv6 listener or a field campaign
as proof that a default production kernel collector is running.

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
- or the explicit local UDP collector supplies packet-only evidence
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

IPv6 loopback advisory input (the listener transport is distinct from the
address family of the packets represented by its facts):

```bash
cargo run -- --protocol dns --entry udp --tcp-socket '[::1]:9000' --ingest-mode local-advisory --json
```

Native Rust producers can use `packet_capture::CapturedUdpDatagram::into_fact`
and `export::fact_to_json` to emit one fact per line. The adapter preserves
IPv4/IPv6 EtherType, full IP length and only actually captured bytes, with a
128-byte prefix bound. It rejects mixed families, unsupported IP header sizes,
inconsistent lengths and overflowing CPU IDs. It neither verifies raw packet
checksums nor authenticates socket/process identity. Packet facts currently
retain cookie/netns/ports and family, not full endpoint IP addresses.

On a Linux host with the field prerequisites, validate the real kernel-to-CLI
boundary without substituting the production fragment registry:

```bash
cargo build --bin gewyvern --bin gewyvern_validate
sudo target/debug/gewyvern_validate linux-udp-capture --ipv6-dns --out-dir /tmp/gewy-ipv6-capture
GEWY_IPV6_DNS_CAPTURE_DIR=/tmp/gewy-ipv6-capture \
GEWY_CAPTURE_INGEST_OUT=/tmp/gewy-ipv6-ingest \
cargo test --test ipv6_capture_ingest_tdd -- --ignored --nocapture
```

Use new output paths and make the capture files readable by the test account;
do not run Cargo as root. The test exercises both DNS packages over Unix,
IPv4-loopback TCP and IPv6-loopback TCP, malformed-input recovery, default
fragment plans, unchanged captured facts, advisory trust and export/replay.
The bounded capture remains direct IPv6 UDP carrying DNS A questions, not
IPv6 extension-header, fragmentation, AAAA, public routing or throughput proof.

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

- [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)
- [process-profiles.md](docs/process-profiles.md)
- [failure-semantics.md](docs/failure-semantics.md)
