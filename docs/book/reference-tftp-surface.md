# Reference: TFTP Surface

Read this page after the generic protocol surface when the runtime looks like
Trivial File Transfer Protocol traffic over UDP.

Use it for:

- `tftp` family lookup
- default entry selection for `read`
- RRQ download paths that receive DATA
- WRQ upload paths that receive ACK
- explicit transfer failure paths that receive ERROR opcode 5
- protocol alias spellings such as `tftp-read`, `tftp_read`, `tftp-rrq`, `tftp_rrq`, `tftp-write`, `tftp_write`, `tftp-wrq`, `tftp_wrq`, `tftp-error`, and `tftp_error`
- entry alias spellings such as `rrq`, `wrq`, `download`, `upload`, `get`, `put`, `transfer-error`, `failed-transfer`, and `error-packet`
- lightweight boot, provisioning, firmware, or config-transfer debugging

Current canonical entries:

- `read` as the default entry
- `write`
- `error`

Default entry: `read`

Operator notes:

- TFTP starts on UDP port 69, but a real transfer may continue on a
  server-selected transfer identifier port.
- Treat `read` and `write` as initial control-path evidence until a later
  transfer-port correlation layer is enabled.
- Treat `error` as the first-class failure path when the server returns opcode
  5 instead of DATA or ACK.

Read in this order:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-tftp-surface.md](docs/book/reference-tftp-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
