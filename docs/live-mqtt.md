# Observe MQTT 3.1.1 Broker Frames

On Linux, start the observer **before** the client connects. Select the
client's canonical, non-root cgroup v2 directory and the broker's TCP port:

```sh
sudo gewyvern capture-mqtt --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 1883 --capture-seconds 15 --json
```

Wait for `capture-mqtt: ready` on stderr. At the deadline (1..300 seconds)
or on SIGINT/SIGTERM the command prints one JSON report on stdout. It needs
cgroup/BPF privileges, clang, BPF headers, libbpf and the kernel's `bpf_loop`
helper. Use only with a **known plaintext MQTT 3.1.1** workload on direct,
unfragmented IPv4 loopback. Do not disable TLS on a production broker.

### Reading A Report

Events are ordered **within each client socket**, in the broker-to-client
direction. CONNACK gives the return code and Session Present bit. PUBLISH
gives QoS, DUP, RETAIN, payload byte length, and a packet identifier for QoS
1 or 2 (null for QoS 0). SUBACK includes the per-filter return codes; PUBACK,
PUBREC, PUBREL, PUBCOMP and UNSUBACK carry their packet identifiers.
PINGRESP has no body. Topics, client IDs, credentials and message contents
are not exported.

For example, on a reconnect, `session_present: true` followed by a QoS 2
PUBLISH with `dup: true` is evidence of a broker retransmission. Packet IDs
are scoped to the exchange, not global message identities. The CLI neither
reads client acknowledgements nor correlates separate connections; it cannot
certify application delivery, persistence across broker restarts or protocol
compliance. See the [MQTT 3.1.1 specification](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html).

### Limits And Exit Status

The first observed broker frame on a newly anchored socket must be CONNACK.
The observer requires contiguous TCP payload, canonical Remaining Length
up to 1 MiB, at most eight return codes per SUBACK, 32 events per socket,
64 sockets and 512 parser steps per segment. It skips topics and message
data by length, not by scanning for control bytes. No MQTT 5, WebSockets,
TLS, general TCP reassembly or installed service/export integration.

Exit 0 means the observed framing was complete, not that the broker accepted
the connection. Exit 3 means incomplete: inspect the counters and socket
outcomes. A partial, malformed, unsupported or over-budget socket publishes
no events, not even a plausible prefix. Payload on a pre-existing connection
increments the unanchored counter without inventing a session.

### Reproduce

Build the Linux `production_capture_linux_tdd` test binary with all features.
As root, set `GEWY_MQ_OUT` to a fresh absolute directory and run the binary
with `--exact mqtt::production_cli_mqtt --ignored --nocapture`. Requires
Docker, nsenter and the pinned Eclipse Mosquitto image already cached. The
fixture starts a private, non-root broker with no external network or
published ports. A separate client in the same namespace but outside the
selected cgroup is a negative control. The `control-*` cases use a scripted
TCP peer, not Mosquitto. The [three Linux trials](fixtures/gewyvern_mqtt_cli_linux_x86_64_20261127.json)
record the individual report hashes and the client-verified event sequences.
