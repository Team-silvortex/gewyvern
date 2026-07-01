# Reference: ZooKeeper Session Surface

Navigation: [ZooKeeper surface](docs/book/reference-zookeeper-surface.md), [protocol surface](docs/book/reference-protocol-surface.md)

This shelf covers the `connect` and `auth-denied` entries for ZooKeeper.

Use it to distinguish transport reachability from usable ZooKeeper session
state. A client can connect to tcp/2181 and still fail because ACLs, auth
schemes, or ensemble session handling reject the operation.

## Entries

- `connect`
- `auth-denied`

## Signals

- Session connect request and response direction.
- Auth or ACL-denial request/response direction.
- Process and route lineage for the client talking to an ensemble member.

## Operator Notes

Session churn, reconnect storms, and ACL denials often look like application
flakiness. Start here before blaming znode read/write behavior.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `acl-denied`
- `acl_denied`
- `authfailed`
- `connect-request`
- `connect_request`
- `handshake`
- `noauth`
- `session`
- `zk-auth-denied`
- `zk-connect`
- `zk_auth_denied`
- `zk_connect`
- `zookeeper-auth-denied`
- `zookeeper-connect`
- `zookeeper_auth_denied`
- `zookeeper_connect`

<!-- gewyvern:entry-aliases:end -->
