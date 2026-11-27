# Observe Memcached Binary Replies

Use this for existing plaintext **classic binary** traffic on Linux IPv4
loopback. Set the client's non-root cgroup v2 directory and the server port:

```sh
sudo gewyvern capture-memcached --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 11211 --capture-seconds 15 --json
```

Wait for `capture-memcached: ready` on stderr **before connecting**. The
observer prints its JSON to stdout at the deadline or on SIGINT/SIGTERM.
It requires cgroup/BPF privileges, clang, BPF headers, libbpf and `bpf_loop`.
If testing inside a network namespace, the actual traffic must be between
loopback peers there; a Docker published port is not equivalent.

**Interpretation.** Each event is one complete *response*, not an inferred
request. GET success requires the four-byte flags prefix: an empty hit has
`result: "hit", value_bytes: 0`, while an explicit key-not-found GET response
is a miss with no value length. A quiet GET that sends no response contributes
**no event**. A NOOP is just a reply, not a count of intervening silent
requests. Numeric server status is preserved even for unrecognized errors.
No keys, values, flag values, opaque tokens, CAS tokens or error text are
exported. Memcached's [binary protocol reference](https://docs.memcached.org/protocols/binary/)
defines the header and quiet commands; this observer is not for text or meta protocol.

**Limits.** GET, GETQ, SET, ADD, REPLACE, DELETE, APPEND, PREPEND and NOOP
responses only. Requires a new SYN-ACK-anchored socket and contiguous TCP
data on direct unfragmented IPv4 loopback. Per run: 64 sockets, 32 replies
per socket; 16 MiB per response body and 512 parser steps per observed
payload segment. Bodies are skipped by declared length. Unsupported opcodes,
invalid shapes, partial frames, discontinuities and overflows withhold the
affected socket's events and return exit 3. Pre-attachment data increments
the unanchored counter. Exit 0 indicates complete observed framing, not that
every cache operation succeeded. No TLS, UDP, general TCP reassembly,
request correlation, installed-service integration or export/replay.

**Reproduce.** Build the Linux `production_capture_linux_tdd` test binary
with all features. As root, set `GEWY_MC_OUT` to a fresh absolute directory
and run `--exact memcached::production_cli_memcached --ignored --nocapture`.
The fixture requires Docker, nsenter and the pinned Memcached image already
present locally. It creates an unprivileged, read-only container with no
external network, no published ports and UDP disabled. Only the network
namespace is shared with the test coordinator. The `control-*` cases use a
separate local peer, not the Memcached daemon.
See the [three Linux trials](fixtures/gewyvern_memcached_cli_linux_x86_64_20261127.json)
for server/image identity, expected events, raw report hashes and segmentation.
