# Process Profiles

This page is a durable semantics note for `process_network_profiles`.

Use it when the question is not "what protocol matched?" but "what kind of
network module is this process failing in?"

If you want a first-use operator path instead of a semantics page, start with:

- [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)
- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)

The key runtime view is:

- `process_network_profiles`

That view is the best current compression layer for process-oriented debugging.

## Current Boundary

The active `1.15.x` CLI does not yet support direct live-process inspection via
`--pid`.

Today, `--pid` is not a usable operator path because:

- non-socket CLI runs are synthetic demos
- socket-ingested runs are advisory-only and intentionally reject PID filtering

So treat the PID-shaped examples in this page as intended future posture, not a
currently supported live-debug command path.

## Companion Shelves

Read this page alongside:

- [docs/ingest-modes.md](docs/ingest-modes.md)
  when you need to decide how much faith to place in PID-scoped conclusions
- [docs/failure-semantics.md](docs/failure-semantics.md)
  when you need the exact meaning of failure modes, details, confidence, and
  basis
- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
  when you need the structured machine-facing diagnosis contract behind these
  summaries

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
- `ambiguous`
- `competing_hypotheses`
- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `operator_guidance_status`
- `operator_guidance_action`

## How To Read It

Read a process profile in this order:

1. `status`
   Is this process healthy, idle, or attention-worthy?
2. `primary_module_kind`
   What kind of network module does the runtime think this most resembles?
3. `ambiguous`
   Is the runtime explicitly telling you there are multiple plausible stories?
4. `competing_hypotheses`
   What other module or transition-level explanations are still alive?
5. `primary_failure_detail`
   What exact operational problem is being suggested?
6. `primary_failure_confidence`
   How hard should you lean on that conclusion?
7. `primary_failure_basis`
   Did the system see a direct protocol signal, a missing transition, or a
   weaker phase-level inference?
8. `operator_guidance_action`
   Given the current evidence quality, what is the safest built-in next step?

Then use:

- `missing_transitions`
- `suspect_areas`
- `protocol_flows`

to inspect the rawer supporting evidence.

Before leaning too hard on any process-level conclusion from socket ingest,
check the ingest-side guardrails in
[docs/ingest-modes.md](docs/ingest-modes.md).

## Example: `apt`

Typical question:

- "is `apt` stuck in DNS, connect, TLS, or HTTP?"

Intended future command shape:

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

Intended future command shape:

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

Intended future command shape:

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

Intended future command shape:

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

## Example: `apt update`

Typical real-world mixed picture:

- DNS lookup happens
- TLS starts
- HTTP request path also starts
- only the final request/response edge is clearly missing

In that case a good report should usually look like this:

- `primary_module_kind = http_request_response`
- `ambiguous = true`
- `competing_hypotheses` includes `name_resolution` and `tls_handshake`
- `primary_failure_confidence = low`

That is the right kind of conservatism. The runtime is saying:

- the strongest concrete failure is in the HTTP request/response path
- but DNS and TLS were both present enough that they still matter

This is much safer than pretending the process has one clean explanation.

## Example: `curl` Via Proxy

Typical mixed picture:

- proxy authentication / tunnel setup is present
- upstream HTTP request path is also present
- the final upstream response never arrives

Useful output shape:

- `ambiguous = true`
- `competing_hypotheses` includes `proxy_authentication`
- `primary_failure_stage = send_request->receive_response`

This says:

- the best current lead is "upstream request sent, reply missing"
- but the proxy leg is still a competing explanation

That is exactly the sort of result you want in a real proxy-heavy environment.

## Example: Media Client Over QUIC / HTTP3

Typical mixed picture:

- QUIC transport/session setup is present
- HTTP/3 request path is present
- HY2-like auth/tunnel behavior may also be present
- the clearest concrete miss is still the HTTP/3 response side

Good conservative output:

- `primary_module_kind = http3_request_response`
- `ambiguous = true`
- `competing_hypotheses` includes `quic_stream_session`
- `competing_hypotheses` may also include `proxy_authentication`

This is useful because it separates:

- "what failed most concretely"
from
- "what other transport/tunnel stories are still live"

## Example: Database Clients

Typical question:

- "did the client fail in auth or query?"

Intended future command shape:

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

If you want a cluster-by-cluster explanation of the failure language itself,
continue with
[docs/failure-semantics.md](docs/failure-semantics.md).
