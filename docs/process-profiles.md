# Process Profiles

This guide explains how to read `gewyvern` when the goal is not "what protocol
matched?" but "what kind of network module is this process failing in?"

The key runtime view is:

- `process_network_profiles`

That view is the best current compression layer for process-oriented debugging.

## What A Process Profile Tries To Answer

A process profile is trying to summarize:

- what protocol-shaped flows matched this process
- what network-module family those flows imply
- which stage looks most suspicious
- whether the problem is more likely:
  - blocked before sending
  - sent but not answered
  - explicitly denied
  - protocol-semantic failure

The most important fields are:

- `status`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`

## How To Read It

Read a process profile in this order:

1. `status`
   Is this process healthy, idle, or attention-worthy?
2. `primary_module_kind`
   What kind of network module does the runtime think this most resembles?
3. `primary_failure_detail`
   What exact operational problem is being suggested?
4. `primary_failure_confidence`
   How hard should you lean on that conclusion?
5. `primary_failure_basis`
   Did the system see a direct protocol signal, a missing transition, or a
   weaker phase-level inference?

Then use:

- `missing_transitions`
- `suspect_areas`
- `protocol_flows`

to inspect the rawer supporting evidence.

## Example: `apt`

Typical question:

- "is `apt` stuck in DNS, connect, TLS, or HTTP?"

Recommended command:

```bash
cargo run -- --scan-all --pid <apt-pid> --json --summary-only
```

Typical useful outcomes:

- `primary_module_kind = name_resolution`
  likely points to upstream resolution trouble
- `primary_module_kind = connection_establishment`
  likely points to route, socket, or connect trouble
- `primary_module_kind = tls_handshake`
  suggests HTTPS path reached transport but not completed handshake
- `primary_module_kind = http_request_response`
  suggests the request path started and got stuck waiting for reply

What to trust most:

- `failure_detail = request_sent_no_reply`
  often means the request side clearly happened and the reply side did not
- `failure_detail = route_or_connect_blocked`
  is more of a setup-side inference and should be read more cautiously

## Example: `curl`

Typical question:

- "did `curl` fail before the request, during handshake, or after sending?"

Recommended commands:

```bash
cargo run -- --protocol http --entry request --pid <curl-pid> --json --summary-only
cargo run -- --protocol tls --entry client --pid <curl-pid> --json --summary-only
```

Good interpretation pattern:

- if TLS says `handshake_incomplete`
  and HTTP does not match at all,
  the failure is probably before application request exchange
- if HTTP says `request_sent_no_reply`
  and TLS is healthy,
  the problem is more likely upstream response behavior than transport setup

## Example: `ffmpeg` Or Media Clients

Typical question:

- "is the media client stuck in RTSP setup or after setup?"

Recommended commands:

```bash
cargo run -- --protocol rtsp --entry setup --pid <ffmpeg-pid> --json --summary-only
cargo run -- --protocol http3 --entry request --pid <ffmpeg-pid> --json --summary-only
```

Useful interpretation:

- `signaling_session + request_sent_no_reply`
  often means control-plane negotiation started but did not receive the next
  expected control response
- if multiple signaling-style paths compete, confidence may be intentionally
  reduced; that is a feature, not a weakness

## Example: Proxy Processes

Typical question:

- "is the proxy failing in auth, connect negotiation, or relay?"

Recommended commands:

```bash
cargo run -- --scan-all --pid <proxy-pid> --json --summary-only
```

What to expect:

- `proxy_authentication`
  if the failure centers on auth-required or auth-denied signals
- `proxy_negotiation`
  if SOCKS5-style method/connect negotiation is the main issue
- `proxy_tunnel_establishment`
  if HTTP CONNECT is the main issue
- `proxy_udp_relay` or `proxy_tcp_relay`
  if setup succeeded and the problem moved into relay traffic

This is where `primary_failure_basis` is especially helpful:

- `direct_protocol_signal`
  means the proxy itself explicitly said no
- `missing_transition`
  means the runtime observed the request side but not the next expected stage

## Example: Database Clients

Typical question:

- "did the client fail in auth or query?"

Recommended commands:

```bash
cargo run -- --protocol postgres --entry auth --pid <pid> --json --summary-only
cargo run -- --protocol postgres --entry query --pid <pid> --json --summary-only
cargo run -- --protocol mysql --entry session --pid <pid> --json --summary-only
```

Interpretation pattern:

- `database_authentication`
  points to login/setup
- `database_query`
  points to request/reply path after setup
- `database_error_handling + protocol_error`
  is stronger than a plain timeout-style inference because the server reported a
  semantic failure directly

## Confidence Rules Of Thumb

Use these defaults:

- `high`
  good candidate for immediate operator action
- `medium`
  good lead, but still based on path completion logic rather than a direct
  protocol denial/error
- `low`
  hypothesis-level signal; compare with neighboring profiles and protocol flows

If you see:

- multiple module kinds
- multiple missing transitions
- multiple competing protocol paths

then the runtime will intentionally lower confidence rather than pretend there
is one obvious winner.

## When To Trust The Process Profile Most

Trust it most when:

- one process clearly dominates one matched flow family
- `primary_failure_basis = direct_protocol_signal`
- `primary_failure_confidence = high`

Trust it less and inspect rawer detail when:

- multiple protocol paths matched the same process
- `primary_failure_basis = heuristic_summary`
- confidence is low

## Recommended Drill-Down Order

When a process profile says `attention`:

1. read `primary_module_kind`
2. read `primary_failure_detail`
3. check `primary_failure_confidence`
4. check `primary_failure_basis`
5. inspect `missing_transitions`
6. compare against `protocol_flows`

That order usually gives the best balance between speed and accuracy.
