# Reference: CoAP Surface

Read this page after the generic protocol surface when the runtime path is a
CoAP request/response exchange over UDP.

Use it for:

- `coap` family lookup
- default entry selection for `get`
- separating read-style lookup from write-style resource mutation
- package aliases such as `coap-post`, `coap_post`, `coap-put`, `coap_put`, `coap-delete`, and `coap_delete`

Primary subpages:

- [docs/book/reference-coap-get-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-coap-get-surface.md)
- [docs/book/reference-coap-write-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-coap-write-surface.md)

Current canonical entries:

- `get` as the default entry
- `post`
- `put`
- `delete`

Default entry: `get`

Read in this order:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-coap-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-coap-surface.md)
3. one exact CoAP subpage
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
