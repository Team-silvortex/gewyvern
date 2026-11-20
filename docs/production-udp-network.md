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

## Evidence And Limits

The [dated proof](fixtures/gewyvern_product_udp_network_linux_x86_64_20261120.json)
records three final runs: six captures, 90 request/reply exchanges and 360
retained observations without recorded loss. The 144 observations longer than
128 payload bytes prove only the captured prefix and full length. Full native
I/O digests are not full captured-payload digests.

The kernel fragments/reassembles these datagrams; the product sees normalized
data at cgroup hooks. It does not implement reassembly or archive raw fragments.
This does not establish handling of arbitrary IPv6 extension chains, overlapping
or malformed fragments, missing fragments, checksums, GSO/GRO, NAT, physical
links, multi-host paths or sustained load. Namespace IDs remain run-local, not
stable host/container identifiers. No product defect was reproduced in these
bounded runs; no release artifact or status maturity is promoted.
