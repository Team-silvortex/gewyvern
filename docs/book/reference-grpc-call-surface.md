# Reference: gRPC Unary Call Surface

Use this page when a single RPC call should be separated from generic HTTP/2
traffic.

## Entry

### `call`

The `call` entry tracks:

- process and route binding
- outbound HTTP/2 headers with `content-type: application/grpc`
- outbound request DATA frame
- inbound response DATA frame

Useful aliases:

- `grpc-call`
- `grpc-unary`
- `http2-rpc`
- `unary`
- `request`
- `rpc`

## Operator Notes

Treat this as an RPC-shape detector, not a Protobuf parser. If this path is
present but no status trailer appears, move next to the `status` shelf before
assuming the service succeeded.

## Reading Order

1. [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
2. [docs/book/reference-grpc-status-surface.md](docs/book/reference-grpc-status-surface.md)
3. [docs/book/reference-protocol-command-paths.md](docs/book/reference-protocol-command-paths.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `grpc-call`
- `grpc-unary`
- `grpc_call`
- `grpc_unary`
- `http2-rpc`
- `http2_rpc`
- `request`
- `rpc`
- `unary`

<!-- gewyvern:entry-aliases:end -->
