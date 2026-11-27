# Observe PostgreSQL Backend Events

On Linux, start the observer before your client connects. Use the client's
canonical, non-root cgroup v2 directory (not PostgreSQL's) and server port:

```sh
sudo gewyvern capture-postgres --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 5432 --capture-seconds 15 --json
```

Wait for `capture-postgres: ready` on stderr. The final JSON is written to
stdout on the deadline (1..300 seconds), SIGINT or SIGTERM. This is for
**existing plaintext IPv4 loopback test traffic**. Do not disable encryption
on production databases to use it. Requires Linux BPF privileges, libbpf,
clang, BPF headers and the `bpf_loop` helper.

**Reading a socket's events.** A `data_row` is one fully observed DataRow
frame, not a copy or validation of its column values. `ready` records the
actual ReadyForQuery state: idle (`I`), in transaction (`T`), or in failed
transaction (`E`). Only an ErrorResponse emits `error_response`. Its phase
is relative to the **first observed readiness**, not an inference about the
cause. Only its C field is compared to three known SQLSTATEs: `28P01`,
`22012` and `25P02`; other valid codes report `other`. Notices, row values
and human-readable error text are not searched. See PostgreSQL's
[wire message formats](https://www.postgresql.org/docs/16/protocol-message-formats.html)
and [error fields](https://www.postgresql.org/docs/16/protocol-error-fields.html).

**Scope.** Supports new SYN-ACK-anchored direct, unfragmented IPv4 loopback TCP
connections, plaintext v3.0 backend startup and a simple-query subset. Only
types, readiness and error categories are exported; not SQL, authentication
data, backend keys or row contents. This is not a query correlator, service
export, replay, TLS/GSS decoder, COPY or extended-query tracer.
Up to 64 sockets, 32 reported events per socket, 16 MiB per message length
and 512 parser steps per observed payload segment. Large row bodies are skipped
by length. An unsupported message, discontinuity, partial frame or exhausted
budget withholds that socket's events and returns exit 3. Exit 0 describes
capture completeness, not query success. No rows observed is not evidence of
a successful connection.

**Reproduce.** Build the Linux `production_capture_linux_tdd` test binary
with all features. As root, set `GEWY_PG_OUT` to a new absolute directory and
run `--exact postgres::production_cli_postgres --ignored --nocapture`. Set
`GEWY_PG_MAJOR` to `16` (default) or `18`; the fixture expects the matching
tools under `/usr/lib/postgresql/<major>/bin` and `/usr/bin/setpriv`. It
initializes a private temporary cluster as an unprivileged user, checks psql
output independently of capture, and stops and removes its own cluster.
The [Linux field record](fixtures/gewyvern_postgres_cli_linux_x86_64_20261127.json)
includes both server versions, report hashes and the Redis transport regression.
