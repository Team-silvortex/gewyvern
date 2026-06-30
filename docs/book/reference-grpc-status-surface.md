# Reference: gRPC Status Trailer Surface

Use this page when the interesting event is the terminal gRPC status rather
than the request body.

## Entry

### `status`

The `status` entry tracks response trailers that carry:

- `grpc-status`
- optional `grpc-message`

Useful aliases:

- `grpc-status`
- `grpc-trailer`
- `trailer`
- `trailers`
- `result`

## Operator Notes

The trailer is the decisive gRPC outcome signal. A TCP-level success or HTTP/2
response frame is not enough to call the RPC healthy if the trailer reports an
application-level failure.

## Reading Order

1. [docs/book/reference-grpc-surface.md](docs/book/reference-grpc-surface.md)
2. [docs/book/reference-grpc-call-surface.md](docs/book/reference-grpc-call-surface.md)
3. [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `grpc-status`
- `grpc-trailer`
- `grpc_status`
- `grpc_trailer`
- `result`
- `trailer`
- `trailers`

<!-- gewyvern:entry-aliases:end -->
