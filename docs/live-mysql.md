# Observe MySQL Classic Replies

On Linux, start the observer **before connecting**, using the client's
canonical non-root cgroup v2 directory and the server's TCP port:

```sh
sudo gewyvern capture-mysql --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 3306 --capture-seconds 15 --json
```

Wait for `capture-mysql: ready` on stderr. At the deadline (1..300 seconds)
or on SIGINT/SIGTERM, one JSON report goes to stdout. This is for an
**already plaintext, uncompressed IPv4 loopback** session, not a reason to
weaken an existing deployment's authentication or TLS. Requires cgroup/BPF
privileges, clang, BPF headers, libbpf and the `bpf_loop` helper.

**Read the events.** A v10 greeting followed directly by a sequence-2
protocol-4.1 OK starts the command phase. A complete ERR captures its phase,
numeric errno and SQLSTATE bucket (`28000`, `42000` or `other`). Thus even
`1045/28000` **after** a successful login is not classified as a login
denial. SQLSTATE is read from the protocol field, not searched in message
text. A single-column text row carries only `value_bytes`: JSON null is SQL
NULL and zero is an empty value. A legacy end-of-result EOF produces
`result_end`; a supported zero-count command OK produces `command_ok`.
This is server-side reply evidence, not a trace of the originating SQL.
No SQL, credentials, authentication material, column names, row values or
error messages are exported.

**Bounds.** The observer anchors new sockets at SYN-ACK and requires
contiguous TCP payload on direct, unfragmented IPv4 loopback. It follows a
restricted stream: direct login without extra authentication packets,
single-column text result sets with both classic EOF packets, and basic
seven-byte OK packets. It checks selected framing fields, not all greeting
or column metadata. No general modern-client compatibility, deprecated-EOF
negotiation, multiple results, authentication switching, prepared/binary
rows, compression, TLS, installed service or replay. Since client requests
and negotiated flags are not read, only use with a workload known to fit
this dialect. The fixture's native-password configuration is lab-only.

Up to 64 sockets, 32 reported events per socket, 2 MiB per MySQL packet
payload and 512 parser steps per observed segment; bulk row data is skipped
by its declared length. On unsupported framing, a gap, an unfinished login
or result set, or a limit, the affected socket's events are withheld and the
CLI exits 3. Unanchored payload increments a counter without inventing a row.
Exit 0 describes captured framing, not successful authentication or SQL.
See the MySQL [ERR packet](https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_basic_err_packet.html)
and [text result set](https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_com_query_response_text_resultset.html)
references.

**Reproduce.** Build the Linux `production_capture_linux_tdd` test binary with
all features. As root, set `GEWY_MY_OUT` to a fresh absolute directory and
run `--exact mysql::production_cli_mysql --ignored --nocapture`. Requires
Docker, nsenter and the repository-pinned MySQL 8.0 image already cached.
The fixture creates its own read-only, non-root container with a tmpfs data
directory and no external network or published ports. Only the test peer is
removed. The `control-*` cases use a separate controlled TCP peer.
See the [three Linux trials](fixtures/gewyvern_mysql_cli_linux_x86_64_20261127.json)
for ordered events, image identity, source hashes and raw report hashes.
