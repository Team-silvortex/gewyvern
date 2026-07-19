# Reference: HTTP Protocol Surface

Use this page when you want the HTTP portion of the built-in protocol shelf as
stable lookup material instead of a tutorial.

This shelf groups the current HTTP coverage into three narrower operator-facing
surfaces:

- client/server request-response flow
- plain `CONNECT` tunnel flow
- proxy-authenticated and denied `CONNECT` branches

## What This Shelf Covers

The current built-in HTTP family models two related but distinct shapes:

- direct request/response traffic
- proxy tunnel establishment through `CONNECT`

Across the subpages, the lookup contract focuses on:

- canonical entry names
- accepted aliases
- coarse request/response shape
- operator reading order
- validation and lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for HTTP entry
selection:

- `http-connect`
- `http-connect-auth-required`
- `http-connect-auth-tunnel`
- `http-connect-denied`
- `http-request`
- `http-server`
- `doh`
- `dns-over-https`
- `dns_over_https`
- `http_connect`
- `http_connect_auth_required`
- `http_connect_auth_tunnel`
- `http_connect_denied`
- `http_request`
- `http_server`

Default entry: `request`

## HTTP Surface Map

### Request And Response

- [docs/book/reference-http-message-surface.md](docs/book/reference-http-message-surface.md)
  Outbound client request flow and inbound server response flow.

Typical entries:

- `request`
- `response`

### CONNECT Tunnel

- [docs/book/reference-http-connect-surface.md](docs/book/reference-http-connect-surface.md)
  Plain proxy tunnel establishment and explicit tunnel denial.

Typical entries:

- `connect`
- `denied`

### CONNECT Auth Branches

- [docs/book/reference-http-connect-auth-surface.md](docs/book/reference-http-connect-auth-surface.md)
  Proxy-auth-required and authenticated-tunnel branches.

Typical entries:

- `auth-required`
- `auth-tunnel`

## Reading Order

If you are validating current HTTP support, the shortest useful order is:

1. [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
2. [docs/book/reference-http-surface.md](docs/book/reference-http-surface.md)
3. one narrower HTTP subpage for the flow you care about
4. [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Next Useful Checks

- For one concrete end-to-end spine walkthrough:
  [docs/architecture-walkthrough-http-request.md](docs/architecture-walkthrough-http-request.md)
- For runtime-confidence checks:
  [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- For exact diagnosis-field meanings:
  [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)
- For encrypted resolver intent layered on the request branch:
  treat `doh` as the HTTP `request` shelf with DNS query semantics carried in
  the request/response payload path
- For the compact DoH reading spine itself:
  [docs/book/reference-doh-overlay.md](docs/book/reference-doh-overlay.md)

## Stability Note

This page is the lookup hub for the HTTP family in the current `1.5.0` line.
New HTTP command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
