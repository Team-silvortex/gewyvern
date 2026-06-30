# Reference: gRPC Streaming RPC Surface

Use this page when a gRPC exchange is long-lived or multi-message rather than a
single unary call.

## Entry

### `stream`

The `stream` entry tracks:

- RPC stream opening through gRPC HTTP/2 headers
- repeated outbound DATA continuation
- repeated inbound DATA continuation

Useful aliases:

- `grpc-stream`
- `grpc-bidi`
- `streaming`
- `bidi`
- `server-stream`
- `client-stream`

## Operator Notes

Streaming RPCs often look healthy at connect time and fail later through
backpressure, cancellation, or missing terminal status. Use this shelf when
message continuity matters more than the first request.

## Reading Order

1. [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
2. [docs/book/reference-grpc-status-surface.md](docs/book/reference-grpc-status-surface.md)
3. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `bidi`
- `client-stream`
- `grpc-bidi`
- `grpc-stream`
- `grpc_bidi`
- `grpc_stream`
- `server-stream`
- `streaming`

<!-- gewyvern:entry-aliases:end -->
