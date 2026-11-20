# Production UDP Packet And Socket Boundaries

This opt-in Linux test drives the formal CLI with real native UDP peers.
It complements the real [DNS](production-dns-field.md) and
[SNMP/NTP/STUN](production-management-udp-field.md) targets; it is not another
protocol-server conformance test or a substitute validation collector.

## Reproduce

Build without root, then run only the named privileged test with a fresh path:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_BOUNDARY_OUT=/tmp/gewy-boundaries-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact boundaries::production_capture_packet_boundaries --nocapture
```

Use Cargo's printed executable path. Requirements are Linux cgroup v2,
clang/libbpf/bpftool and a loopback MTU of at least 60,048 bytes. No Docker target,
public listener, external service or host configuration change is needed.
The actual product executable uses its default UDP debug template.

## Checks

Both IPv4 and IPv6 exercise payload lengths 0, 1, 2, 3, 4, 127, 128, 129,
512, 1,400, 4,096 and 60,000, followed immediately by 1-byte and empty packets,
then a 32-byte recovery exchange.
Four independent CLI sessions cover:

- Connected client sockets.
- Unconnected clients using send-to/receive-from.
- Unconnected clients created in a child cgroup below the selected scope.
- Bound unconnected server sockets serving two client ports per address family.

An independent worker validates full native I/O and produces an expected packet
multiset. Product facts must preserve family, socket cookie, local/remote ports,
direction, full IP length and exactly the observed bounded payload prefix.
Cached byte and prefix fields must agree, including absence on short/empty
packets; no old bytes may leak from earlier large packets. Server replies test
source-port selection, not only the destination-port client path.

For the three client modes, a same-target socket created before entering the
cgroup and a same-cgroup socket using a different UDP port both complete real
exchanges but must be absent from captured facts. These are socket-creation and
port-scope checks, not proof of a product per-socket allowlist. Child coverage
does not establish exclusive ownership across ancestor/descendant collectors.

Graceful stop must detach hooks, reconcile all counters, retain every expected
observation and preserve facts/capture/stages/attachment report through replay.
The test owns and removes its cgroups and children. Each run has 120 selected
request/reply exchanges and 240 hook observations, not a throughput benchmark.

## Evidence And Limits

The [dated Linux proof](fixtures/gewyvern_product_udp_boundaries_linux_x86_64_20261120.json)
retains three final runs, source/binary hashes, a lifecycle/loss regression and
cleanup checks. The oracle's full-I/O digests describe native client/server data;
they are not full-payload digests asserted by the product for truncated captures.
For messages over 128 bytes, only the actual prefix and full packet length are
proven. An accounted loss-free run still does not mean full packet archival.

This is direct loopback UDP on one host/kernel, not fragmentation/reassembly,
GSO/GRO, physical interfaces, checksum verification, protocol parsing, process
attribution, sustained load or minimum-kernel support. It does not promote a
release artifact, status maturity or protocol count.
