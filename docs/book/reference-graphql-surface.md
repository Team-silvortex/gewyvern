# Reference: GraphQL Surface

GraphQL support gives gewyvern an application-RPC view above HTTP and WebSocket transport without requiring schema access.

Default entry: `query`

Protocol aliases: `gql`, `graphql-query`, `graphql_query`, `gql-query`, `gql_query`, `graphql-mutation`, `graphql_mutation`, `gql-mutation`, `gql_mutation`, `graphql-subscription`, `graphql_subscription`, `gql-subscription`, `gql_subscription`

Navigation: [protocol surface](docs/book/reference-protocol-surface.md), [IR lowering](docs/book/reference-ir-lowering.md)

## Entries

- [`query`](docs/book/reference-graphql-query-surface.md) tracks read-style GraphQL request paths over HTTP GET or POST.
- [`mutation`](docs/book/reference-graphql-mutation-surface.md) tracks write-style GraphQL request paths over HTTP POST.
- [`subscription`](docs/book/reference-graphql-subscription-surface.md) tracks live GraphQL setup and frame traffic, usually through WebSocket.

## Operator Use

Start with `query` for read failures and general endpoint reachability. Move to `mutation` when the failure is write-specific. Use `subscription` when the issue only appears in live updates or event streams.

## Limits

This surface does not parse GraphQL ASTs or validate schema fields. It is a transport-aware debugger entry point for routing, lifecycle, and directionality.
