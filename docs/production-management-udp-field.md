# Production SNMP, NTP And STUN Field Checks

These native campaigns run the formal CLI with its own product UDP collector
against real private Net-SNMP, chrony and coturn targets. They do not substitute
the validation BPF producer, socket-fed facts or client bytes for capture.

## Run On Linux

Build the two executables and preload the existing target images from
`ebpf/validation/{snmp,ntp,stun}` as described in
[native validation commands](script-entrypoints.md). Use a new directory per run:

```bash
cargo build --locked --bin gewyvern --bin gewyvern_validate
sudo target/debug/gewyvern_validate --json linux-udp-capture \
  --production-snmp --out-dir /tmp/gewy-product-snmp-new
sudo target/debug/gewyvern_validate --json linux-udp-capture \
  --production-ntp --out-dir /tmp/gewy-product-ntp-new
sudo target/debug/gewyvern_validate --json linux-udp-capture \
  --production-stun --out-dir /tmp/gewy-product-stun-new
```

The modes are mutually exclusive with all other UDP campaigns. The runner uses
`target/debug/gewyvern`, records its digest and starts a fresh process for each
package. Targets are resolved to local image IDs before creation; their actual
configuration must match the compiled fixture. No pull, public port, host mount
or external upstream is permitted by the target fixture. NTP uses `chronyd -x`
and no SYS_TIME capability; it cannot discipline the host clock.

## Coverage

| Target | Formal CLI entries | Independent client checks |
| --- | --- | --- |
| Net-SNMP | `get`, `get-next`, `bulk` | GET/GETNEXT/GETBULK, variable-length IDs, noSuchObject, wrong-community timeout followed by recovery |
| chrony | `client`, `query`, `sync`, each against two configurations | NTPv3/v4, echoed origin, unsynchronised versus local-reference replies, short-request timeout and recovery |
| coturn | `binding`, `binding-error` | XOR-MAPPED-ADDRESS, 420 with unknown required attribute, optional padded attribute, short-request timeout and recovery |

Each package uses two selected sockets and an actual same-target neighbor socket
created before entering the capture cgroup. Product capture is cgroup/port scoped,
not a per-socket allowlist. Independent client I/O must match the full captured
socket/direction/payload multiset. Messages longer than the 128-byte captured
prefix fail the check instead of being filled from client data. Packet family,
length, ports, socket identity, final accounting, clean exit and hook removal
must agree; replay must preserve facts, program stages, capture and attach data.

Every emitted request/response phase must refer to an allowed packet from the
client oracle, and each expected socket must appear. This checks negative
outcomes too: short NTP/STUN requests must not create complete-request phases,
an unsynchronised NTP response must not satisfy the sync time-reply phase, and
STUN success/error replies must not substitute for one another. Program phases
can deduplicate repeated observations; the complete raw packet multiset cannot.

SNMP response phases remain generic response candidates, not correlated request
success. A wrong-community request can be structurally valid despite no reply;
noSuchObject is a decoded exception, not a successful MIB value. NTP origin and
STUN transaction/address validation belong to the independent client oracle;
they do not establish product transaction correlation.

## Evidence And Limits

The [dated Linux proof](fixtures/gewyvern_product_management_udp_linux_x86_64_20261113.json)
retains accepted summaries, selected source and binary hashes, negative outcomes
and cleanup checks. Raw product export/status/logs and client oracle files stay
in the isolated server evidence directory. Existing validation-only modes and
the formal DNS path remain separate regression gates.

These are bounded IPv4 UDP sessions on one host/kernel, not IPv6 coverage,
fragment reassembly, full protocol parsing, vulnerability detection, sustained
load, PID attribution or cross-window transactions. SNMPv3, writes/traps,
accurate time/NTS, TURN relays, real NAT traversal, ICE and authentication remain
unproved. No protocol-count, status-maturity or release-artifact promotion is
implied.
