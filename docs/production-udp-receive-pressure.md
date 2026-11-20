# Production UDP Receive Pressure

This opt-in check separates three events: the cgroup hook observing a datagram,
the kernel admitting it to a socket receive queue, and the application copying
it. It uses the formal CLI and persistent native IPv4/IPv6 socket pairs over the
[private MTU-1500 network fixture](production-udp-network.md), not fabricated
facts or the validation collector.

## Reproduce

Build without root, then use the printed test executable and a new output path:

```bash
cargo test --locked --test production_capture_linux_tdd --no-run
sudo env GEWY_NETWORK_PRESSURE_OUT=/tmp/gewy-receive-pressure-new \
  target/debug/deps/production_capture_linux_tdd-<build-hash> \
  --ignored --exact network::pressure::production_capture_receive_pressure --nocapture
```

The host needs the [production capture prerequisites](live-udp-capture.md),
iproute2 and bpftool. Only owned namespaces, veth links, cgroups and sockets are
changed. No host interface, route, firewall, sysctl, other service or Docker
target is changed. Workers enter the selected cgroup before creating sockets.

## Native Oracles

Five phases run baseline, application truncation, short-message recovery,
receive-queue overflow and recovery without replacing either peer's sockets.

For truncation, clients send 1024 bytes but servers provide only a 64-byte read
buffer. Native `recvmsg(MSG_TRUNC)` must report the original length, set the
truncation flag and copy exactly the expected first 64 bytes. A 32-byte reply
acknowledges this partial read; it is not proof that the whole request reached
application memory. The collector must retain the original packet length and
its own 128-byte bounded prefix, not substitute the application's shorter copy.

For overflow, the server requests `SO_RCVBUF=4096` on its private sockets and
records the returned capacity (8192 in the dated runs). It performs no reads
until both clients finish 64 distinct 1024-byte sends per family. The controller
then releases draining. At least one packet must be read and at least one dropped
per family; exact native deliveries plus `RcvbufErrors` must equal 64. No fixed
queue capacity in packets is assumed. Per-phase namespace counters must match
native reads, sends and errors, with no checksum, closed-port or send-buffer errors.

The recovery message must be received intact on the same socket and carry an
`SO_RXQ_OVFL` count equal to the earlier namespace receive-buffer error delta.
This second, socket-specific check prevents treating an unexplained timeout or
an unrelated network discard as verified receive-queue overflow. Normal/recovery
phases must add no receive-buffer errors.

The native interfaces are documented in Linux [recv(2)](https://man7.org/linux/man-pages/man2/recvmsg.2.html)
and [socket(7)](https://man7.org/linux/man-pages/man7/socket.7.html). These native
observations are independent fixture oracles, not new product telemetry fields.

## Evidence And Interpretation

The [dated proof](fixtures/gewyvern_product_udp_pressure_linux_x86_64_20261120.json)
contains three final runs on x86_64 kernel 7.0.0-43-generic: 408 requests, 24
replies (including truncation acknowledgements), six application truncations and
864 captured observations. Of 384 overflow-burst datagrams, applications read 18
and socket queues drop 366. Both native counter sources agree. Full capture
accounting remains loss-free; replay and owned-resource cleanup pass. Fresh
native-send-error and actual collector-overflow/lifecycle regressions also pass.

All burst datagrams are visible at the tested cgroup ingress hook, including
those rejected by the receive queue afterward. An ingress fact therefore proves
an observation at that hook, not application delivery. Receiver queue drops are
not BPF ring drops, userspace retention drops or decode failures. The product
does not automatically identify these socket drops or application truncation.

The packet oracle checks exact cookie, family, ports, direction, full IP length,
bounded bytes and namespace/interface identity. For overflow ingress, its basis
is the sender bytes plus independent queue accounting, explicitly not a claimed
application receipt. The compact repository fixture stores a deterministic digest
of all 144 expected observations per peer; ordinary tests reconstruct and check
that digest. Full oracles and exports remain in the recorded Linux evidence path.

No product defect was reproduced. This is a bounded private-link burst, not a
sustained-load benchmark, physical-network result, minimum-kernel guarantee,
product socket-drop classifier, full-payload archive or delivery acknowledgement
protocol. No release artifact, protocol count or status maturity is promoted.
