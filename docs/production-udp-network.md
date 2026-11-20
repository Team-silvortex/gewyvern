# Production UDP Across Network Namespaces

The opt-in Linux test drives the actual CLI across a private MTU-1500 veth
pair, with IPv4 and IPv6 native peers in two disposable network namespaces.
Both peers create sockets inside one selected cgroup. Each successful exchange
therefore produces four hook observations, not two independent wire exchanges.

## Reproduce

Build without root, then use Cargo's printed test executable and a fresh path:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_NETWORK_OUT=/tmp/gewy-network-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::production_capture_network_namespaces --nocapture
```

The host needs the [production capture prerequisites](live-udp-capture.md),
`iproute2`, `bpftool`, and permission to create network namespaces and veth links.
Only owned namespaces receive interfaces, addresses and connected routes. No
host interface, host route, firewall rule, clock, Docker target or public
listener is changed. Workers use native `setns` on the I/O thread without
remounting sysfs; counters come from `/proc/thread-self/net`.

## Acceptance

- One phase sends 0, 1, 128, 129, 1400, 1452, 1, 0 and 32 payload bytes, all
  within the link MTU for both families. Kernel fragmentation counters stay zero.
- A second phase sends 1473, 4096 and 60000 bytes, then 1, 0 and 32 bytes.
  PMTU discovery is disabled only on these disposable sockets. Native I/O is
  byte-checked, and both peers must report actual IPv4/IPv6 fragmentation and
  successful reassembly without failures.
- Captured multisets must match each peer's cookie, direction, ports, family,
  full IP length and bounded payload prefix. Interface indices must match the
  actual veth; identical interface numbers in distinct namespaces must not
  merge their run-local namespace identities. No PID attribution is inferred.
- All counters reconcile after graceful stop. Facts, capture metadata, program
  flows and attachment reports survive export/replay. Hooks, owned cgroups,
  network namespaces and their links must be removed.

## Network Loss And Recovery

Use the same build prerequisites, including `tc`, and a fresh evidence directory:

```bash
sudo env GEWY_NETWORK_LOSS_OUT=/tmp/gewy-network-loss-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::faults::production_capture_network_loss --nocapture
```

Persistent dual-stack peers run baseline, request loss, recovery, response loss,
and recovery without recreating their sockets. A controller installs selected
UDP-destination-port `flower`/`gact drop` rules on the disposable sender's veth
egress. It waits for both workers before verifying exact per-family TC packet
and drop counters, removing the rules, and releasing the next phase.

The [network-loss proof](fixtures/gewyvern_product_udp_network_loss_linux_x86_64_20261120.json)
retains three final runs: 30 requests, 18 successful exchanges, 12 client
timeouts, 12 actual TC drops and 96 retained observations. Each run has 8, 2, 8,
6 and 8 expected facts across its five phases. Request loss leaves only client
egress; response loss leaves client egress plus server ingress/egress, but no
invented client ingress. Full 32-byte payloads, socket/namespace identities,
ports, interface indices and replay are checked by the same packet oracle.

Capture counters remain complete and loss-free despite application timeouts.
This is correct: the scope observed all packets that reached its hooks. The
independent TC oracle establishes where this fixture discarded packets; the
product does not automatically classify their root cause. A fresh lifecycle
regression separately verifies that actual collector overflow remains incomplete.

The fixture uses a longer normal receive deadline for initial ARP/NDP resolution
and short deadlines only for deliberate timeouts. It is deterministic, selected
port loss, not random loss, reordering, congestion, ICMP/PMTU or a latency benchmark.
No qdisc or filter is added to host interfaces; owned namespace teardown also
removes their private links and TC state on a failed assertion.

## Delay, Duplication And Reordering

The same isolated fixture can run selected-port netem checks:

```bash
sudo env GEWY_NETEM_OUT=/tmp/gewy-netem-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::netem::production_capture_network_netem --nocapture
```

Persistent sockets send four distinct 32-byte messages per family and phase:
baseline, response delay, request duplication, response duplication, request
reordering, and recovery. Flower classifiers direct only the selected UDP port
to a bounded child netem queue; unrelated private-namespace traffic stays in
the default class. No host-interface qdisc is changed.

Delay is configured at 120 ms without jitter; the first client reply must arrive
at least 100 ms after the burst starts. Duplication is 100%; all expected copies
must reach the corresponding peer. Reordering uses a 120-ms delay with gap 2
and must produce an actual order inversion, not merely a configured rule.
The controller checks live classifier/queue settings, transmitted counts, zero
drops and an empty queue before removing each injection. A receive guard rejects
unexpected extra packets. The same sockets complete the recovery phase.

The [dated netem proof](fixtures/gewyvern_product_udp_netem_linux_x86_64_20261120.json)
retains three runs: 144 application requests, 192 client response deliveries and
672 captured observations without recorded collector loss. Duplicated requests
cause extra server replies; duplicated replies add client deliveries without
inventing extra server sends. Packet multisets retain every copy. Separately,
each socket/direction's captured sequence must match native send/receive order;
the reordered phase observed `1, 3, 0, 2` rather than `0, 1, 2, 3`.

These are small bounded bursts on one host, not a global cross-CPU wire-order
guarantee, transaction de-duplication, latency percentile benchmark, random
jitter distribution, congestion or high-load result. Initial ARP/NDP time is
not counted as injected delay. The product does not classify the netem cause;
the independent fixture establishes it. The original loss and MTU checks also
pass fresh regressions after the shared-worker extraction.

## Send Errors And Socket Recovery

Use the same native build and a fresh output directory:

```bash
sudo env GEWY_NETWORK_ERRORS_OUT=/tmp/gewy-network-errors-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::errors::production_capture_network_errors --nocapture
```

Connected IPv4/IPv6 client sockets enable error queues and force PMTU discovery
only on these disposable sockets. Five phases exercise baseline, local MTU
rejection, short-message recovery, a closed remote port and service recovery.
The controller observes server close/rebind readiness before releasing clients.
Clients keep their original sockets; the server's replacement sockets must have
new, separately observed cookies within the same run-local namespace identity.

Each 4096-byte send over MTU 1500 fails locally with `EMSGSIZE`. The native
error queue reports local origin and MTU 1500, while the server times out.
No UDP observation is invented for this rejected send. With the remote port
closed, a 32-byte send succeeds but receive returns `ECONNREFUSED`. Native
`MSG_ERRQUEUE` reports IPv4 ICMP type/code `3/3` or IPv6 `1/4`, and quotes the
exact request payload and destination. Only client egress is captured: there
is no server socket to receive it. Error consumption and subsequent successful
small exchanges prove that the original clients remain usable.

The [dated error/recovery proof](fixtures/gewyvern_product_udp_errors_linux_x86_64_20261120.json)
retains three final runs: 30 send attempts, six local MTU rejections, six remote
port refusals, 18 successful exchanges and 78 captured observations. Each run
has 8, 0, 8, 2 and 8 expected facts across its five phases. Exact packet
multisets, all socket generations, interface/namespace identity, final accounting,
replay and owned-resource cleanup pass. Fresh fragmentation, TC loss and actual
collector-overflow/lifecycle regressions also pass. No product defect was found.

Application failure is not collector loss. The error queue is an independent
native test oracle, not a product ICMP decoder or automatic cause classifier.
This proves local MTU rejection and remote port refusal on one private link,
not routed PMTU discovery, ICMP blackholes, multi-hop failures or physical
network behavior. No release artifact or status maturity is promoted. See
the Linux [receive error-queue contract](https://man7.org/linux/man-pages/man2/recvmsg.2.html)
for the ancillary-message interface used by the oracle.

## Evidence And Limits

For application read truncation and socket queue exhaustion, see the separate
[receive-pressure check](production-udp-receive-pressure.md). It distinguishes
hook observations, kernel queue admission and actual application reads using
native receive flags and two independent drop counters.
For a real GSO/GRO false-completeness repair and explicit unsupported behavior,
see the [offload boundary check](production-udp-offload.md). It verifies native
segmentation/merging without claiming per-datagram normalization by the product.

The [dated proof](fixtures/gewyvern_product_udp_network_linux_x86_64_20261120.json)
records three final runs: six captures, 90 request/reply exchanges and 360
retained observations without recorded loss. The 144 observations longer than
128 payload bytes prove only the captured prefix and full length. Full native
I/O digests are not full captured-payload digests.

The kernel fragments/reassembles these datagrams; the product sees normalized
data at cgroup hooks. It does not implement reassembly or archive raw fragments.
This does not establish handling of arbitrary IPv6 extension chains, overlapping
or malformed fragments, missing fragments, checksums, GSO/GRO normalization, NAT, physical
links, multi-host paths or sustained load. Namespace IDs remain run-local, not
stable host/container identifiers. No product defect was reproduced in these
bounded runs; no release artifact or status maturity is promoted.
