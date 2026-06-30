# Reference: gRPC Protocol Surface

Use this page when HTTP/2 traffic should be interpreted as RPC intent rather
than only generic encrypted or proxied request/response traffic.

Default entry: `call`

Protocol aliases: `grpc-bidi`, `grpc-call`, `grpc-status`, `grpc-stream`,
`grpc-trailer`, `grpc-unary`, `grpc_bidi`, `grpc_call`, `grpc_status`,
`grpc_stream`, `grpc_trailer`, `grpc_unary`, `http2-rpc`, `http2_rpc`

## What This Shelf Covers

The current gRPC family models three stable debugger-facing paths:

- unary RPC calls carrying `application/grpc` HTTP/2 headers and DATA frames
- response trailers carrying `grpc-status` and optional `grpc-message`
- streaming RPCs with repeated DATA-frame continuation

This is intentionally not a Protobuf decoder. The useful 0.18.x behavior is to
identify RPC shape, direction, continuation, and terminal status without making
payload decoding a dependency.

## gRPC Surface Map

### Unary Call

- [docs/book/reference-grpc-call-surface.md](docs/book/reference-grpc-call-surface.md)
  Client call setup and request/response message exchange.

Typical entries:

- `call`

### Status Trailer

- [docs/book/reference-grpc-status-surface.md](docs/book/reference-grpc-status-surface.md)
  Response trailer status and message metadata.

Typical entries:

- `status`

### Streaming RPC

- [docs/book/reference-grpc-stream-surface.md](docs/book/reference-grpc-stream-surface.md)
  Client, server, or bidirectional repeated DATA-frame flow.

Typical entries:

- `stream`

## Reading Order

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
3. one narrower gRPC subpage
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
