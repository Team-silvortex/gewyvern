# Production DNS Field Check

This acceptance slice connects the formal CLI's own Linux UDP producer to the
existing real CoreDNS target. It does not substitute the validation collector,
demo facts or socket-fed observations for the product capture path.

## Run On Linux

Build both native executables, preload the pinned CoreDNS image described in
[native validation commands](script-entrypoints.md), then choose a fresh output
directory for each run:

```bash
cargo build --locked --bin gewyvern --bin gewyvern_validate
sudo target/debug/gewyvern_validate --json linux-udp-capture \
  --production-dns --out-dir /tmp/gewy-product-dns-v4-new
sudo target/debug/gewyvern_validate --json linux-udp-capture \
  --production-ipv6-dns --out-dir /tmp/gewy-product-dns-v6-new
```

Each option is mutually exclusive with the other UDP campaign modes. The runner
uses the checkout's `target/debug/gewyvern` and records its digest. The target is
digest-pinned, private, resource-limited and unpublished; no image is pulled, no
host resolver is changed and no public DNS query is forwarded. IPv6 uses a new
IPv6-only ULA bridge. Existing image/config/client/cleanup code is shared with
the earlier DNS campaign.

The product CLI runs separately for `--protocol dns --entry udp` and `error`,
each with a fresh capture identity and private state. Each is a bounded one-shot
capture, stopped gracefully after the requests, not a rolling-window service or
an archive. The existing product collector supplies the only BPF observations.

## Acceptance

Two selected sockets share a DNS transaction ID across ten queries: normal A
answers, FORMERR/SERVFAIL/NXDOMAIN/REFUSED, TC and non-TC replies from the erratic
endpoint, a dropped reply, and successful
reuse after the timeout. A same-target NXDOMAIN neighbor socket is created
**before** the client enters the selected cgroup. It must remain excluded even
when used later. The product selects a cgroup/port, not the validation collector's
per-socket allowlist; every admitted socket in that scope remains in scope.

Independent native client I/O validates complete bounded DNS replies. Product
facts must match the complete socket/direction/payload multiset, including every
extra server response. Prefixes exceeding 128 bytes fail this check rather than
being filled from the client oracle. Family, full IP length, local/remote ports,
socket identity, final loss accounting and absence of invented PID facts are
checked separately.

Both existing protocol packages must produce the expected phase/socket sets.
Every emitted error candidate must point to a real received packet with the
corresponding error bits. Export/replay preserves the actual facts, capture
accounting and program stages. TC is not successful resolution; timeout is not a
DNS RCODE. Reports retain the advisory `unverified-local` trust level.

## Evidence And Limits

The pinned CoreDNS 1.15.0 target alternates TC and non-TC replies for `truncate 1`.
This differs from the plugin documentation's one-in-N description: the pinned
[implementation](https://github.com/coredns/coredns/blob/v1.15.0/plugin/erratic/erratic.go)
uses a bitwise counter test for truncation. The campaign observed this on the
second package, then added consecutive TC/non-TC requests for each package.
Both carry the erratic endpoint's `192.0.2.53` answer; ordinary success uses
`192.0.2.7`. No target restart hides this behavior, and it is not reported as a
Gewyvern capture failure or general CoreDNS conformance.

`summary.json` records target identities, product digest, package source hashes,
packet counts, client reply checks and cleanup. Each package directory retains
the original product export/status/stdout/stderr and independent `oracle.json`.
The dated [Linux proof](fixtures/gewyvern_product_dns_linux_x86_64_20261113.json)
binds those results to selected source hashes.

This is direct IPv4/IPv6 UDP with small DNS A messages on one host. It is not
general DNS parsing, request/response correlation across windows, full IP address
retention in product facts, AAAA, DNSSEC/EDNS, DoH/DoT, TCP fallback, recursive
caching, vulnerability classification or a throughput benchmark. No service
deployment, boot setting or existing workload is modified by this campaign.
