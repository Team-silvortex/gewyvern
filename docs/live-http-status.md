# Observe The First HTTP/1.1 Response

Use this on Linux to check what status prefix a **new connection from your own
client workload** actually received, including when the prefix spans TCP
segments. Start the observer before making the request. Specify the client's
existing non-root cgroup v2 directory and the HTTP server's TCP port:

```sh
sudo gewyvern capture-http-status \
  --capture-cgroup /sys/fs/cgroup/my-client.service \
  --capture-tcp-port 8080 --capture-seconds 15 --json
```

Use the actual canonical cgroup path, not the server's cgroup. Wait for
`capture-http-status: ready` on stderr. The final JSON goes to stdout at the
deadline (1..300 seconds), or on SIGINT/SIGTERM. Root/BPF privileges, clang,
Linux/BPF headers and libbpf.so.1 are required. The ingress link and maps are
process-owned, not pinned. An empty connection list does not mean HTTP succeeded.

**Read the result.** Rows identify the client socket by its cookie (decimal
string) and local port. `verified_prefix` means the first 13 stream bytes
match `HTTP/1.1 NNN ` for a code from 100 to 599; it is not verification of a
complete HTTP exchange. `not_http11` has no status. Missing or conflicting
prefix evidence also returns no status. Exit 3 means the report is incomplete:
check the per-connection outcomes and the counters for unanchored payload,
missing socket cookies, invalid observations or exhausted capacity. Exit 0
is not a claim that every HTTP request was successful.

**Scope.** Client ingress only, IPv4 loopback interface (ifindex 1) and both
addresses in 127/8, explicit server port, direct unfragmented TCP. A captured
SYN-ACK anchors the offsets. Kernel maps retain only matching literal/digit
masks, not raw bytes, URLs, headers, or bodies. They hold at most 64 socket
identities for the entire run, without eviction or attaching to old connections.
Only the first prefix is classified, not every response on keep-alive, the final
response after a 1xx, or a request/response transaction. No TLS, IPv6, general
TCP reassembly, DSL export/replay, installed service or rolling API windows.

**Linux evidence.** The [field record](fixtures/gewyvern_http_status_cli_linux_x86_64_20261127.json)
distinguishes controlled segmentation/decoy traffic from a real unprivileged
Caddy server. To reproduce, build `production_capture_linux_tdd` with all
features and run the ignored `http_status::production_cli_http_status` and
`http_status::caddy::production_cli_caddy` cases as root, setting
`GEWY_HTTP_STATUS_OUT` and `GEWY_HTTP_CADDY_OUT` to separate fresh absolute
directories. The second case uses the installed Caddy and setpriv binaries,
not an external site or the validation BPF producer.
