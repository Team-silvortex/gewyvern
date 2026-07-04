# Reference: Syslog Surface

Read this page after the generic protocol surface when the runtime looks like
classic syslog forwarding, relay, or secure syslog transport setup.

Use it for:

- `syslog` family lookup
- default entry selection for `udp`
- UDP syslog datagrams sent to port 514
- TCP syslog streams sent to port 514
- TLS-protected syslog transport bootstrap on port 6514
- protocol aliases such as `syslog-udp`, `syslog_udp`, `syslog-tcp`,
  `syslog_tcp`, `syslog-tls`, `syslog_tls`, and `syslog-secure`
- entry aliases such as `datagram`, `message`, `rfc3164`, `rfc5424`, `stream`, `octet-counted`, `secure`, and `rfc5425`

Current canonical entries:

- `udp` as the default entry
- `tcp`
- `tls`

Default entry: `udp`

Operator notes:

- The current stable subset identifies syslog payload shape by the leading
  `<PRI>` marker and transport port.
- It deliberately does not infer severity yet; PRI parsing should become a
  structured IR field before severity-specific failure paths are added.
- For TLS syslog, the runtime can identify the protected transport bootstrap,
  but payload content remains encrypted.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-syslog-surface.md](docs/book/reference-syslog-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
