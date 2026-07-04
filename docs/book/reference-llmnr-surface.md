# Reference: LLMNR Surface

Read this page when a host falls back to local-link name resolution instead of
ordinary DNS.

Use it for:

- `llmnr` family lookup
- default entry selection for `query`
- UDP LLMNR datagrams on port 5355
- query frames with DNS-style QR bit cleared
- response frames with DNS-style QR bit set
- protocol aliases such as `llmnr-query`, `llmnr_query`,
  `llmnr-response`, `llmnr_response`, `llmnr-error`, and `llmnr_error`
- entry aliases such as `lookup`, `local-name-query`, `answer`,
  `local-name-answer`, `nxdomain`, `servfail`, `formerr`,
  `resolution-failed`, and `local-name-failed`

Current canonical entries:

- `query` as the default entry
- `response`
- `error`

Default entry: `query`

Operator notes:

- The stable subset uses DNS-like header bits only: QR `0` for query, QR `1`
  for response, and QR `1` plus non-zero rcode for error.
- Treat this as local-link discovery, not general resolver health. If both
  DNS and LLMNR are present, read DNS first and LLMNR as fallback behavior.
- Payload name parsing is intentionally deferred until the IR can represent
  queried names without mixing them into generic packet predicates.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-llmnr-surface.md](docs/book/reference-llmnr-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

<!-- gewyvern:entry-aliases:end -->
