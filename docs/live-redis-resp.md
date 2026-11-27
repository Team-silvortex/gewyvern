# Observe Redis Pipeline Replies

Start this on Linux **before the client connects**, using the canonical
cgroup v2 directory of the client (not Redis) and Redis's TCP port:

```sh
sudo gewyvern capture-redis-resp \
  --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 6379 --capture-seconds 15 --json
```

Wait for `capture-redis-resp: ready` on stderr. The deadline (1..300 seconds),
SIGINT or SIGTERM ends the observation and writes one JSON report to stdout.
The Linux host needs cgroup/BPF privileges, clang, BPF headers, libbpf.so.1
and the `bpf_loop` helper. Links and maps are not pinned.

**Read the result.** Each socket's `replies` lists **top-level** RESP2 frames
in wire order, not commands or transactions. A `GET` returning
`-ERR not an error` is a `bulk_string` of 17 bytes; an actual Redis error
frame is `error`. Null bulk and zero-byte bulk are different. A flat array
reports its element count, not the types or success of individual elements.
Neither the report nor the BPF maps store keys, commands, credentials, reply
text or bulk bodies. An exit code of 0 means the observed framing was complete,
not that the commands succeeded or that every request received an answer.

**Boundaries.** This is plaintext, unfragmented IPv4 loopback client ingress
only. A new socket must have a captured SYN-ACK. The observer accepts a
contiguous stream, not general TCP reassembly. It supports simple strings,
errors, integers, bulk strings and one level of arrays. Limits per run: 64
sockets, 32 top-level replies per socket, 16 MiB per bulk, 128 elements per
array and 512 parser steps per observed payload segment (bulk bodies are
skipped by length). Unknown types, a gap/overlap, partial frames or a limit
return exit 3; an uncertain socket's reply list is withheld, not guessed.
Unanchored payload increments a counter without inventing a socket row.
An empty report is not evidence of a successful exchange. No TLS, RESP3,
request correlation, service integration or export/replay is claimed.

**Reproduce.** Build the Linux `production_capture_linux_tdd` test binary with
all features. As root, run its ignored `redis_resp::production_cli_redis_resp`
test with `GEWY_REDIS_RESP_OUT` set to a fresh absolute directory. It starts a
password-protected, unprivileged local `/usr/bin/redis-server` via `setpriv`
with persistence disabled. Client bytes are checked independently of the CLI;
each scenario retains its own `report.json`. See the
[Linux field record](fixtures/gewyvern_redis_resp_cli_linux_x86_64_20261127.json).
