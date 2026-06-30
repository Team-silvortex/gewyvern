# Reference: Redis KV Surface

Use this page when you need the current exact lookup surface for Redis
key-value and session-oriented protocol entries in the built-in shelf.

## Covered Entries

### Connection And Health

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `session` | establish or observe a Redis session | simple string |
| `ping` | health check the server | simple string |

### Single-Key Read And Write

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `get` | read one key | bulk string |
| `set` | write one key | simple string |

### Counter Mutation

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `incr` | increment one counter key | integer |
| `decr` | decrement one counter key | integer |

### Multi-Key Read And Write

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `mget` | read multiple keys | array |
| `mset` | write multiple keys | simple string |

### Existence And Deletion

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `exists` | check whether keys are present | integer |
| `del` | delete one or more keys | integer |

### Expiry And TTL

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `expire` | assign key expiry | integer |
| `ttl` | inspect TTL in seconds | integer |
| `pttl` | inspect TTL in milliseconds | integer |

## Aliases

### Connection And Health Aliases

- `connect -> session`
- `roundtrip -> session`
- `health -> ping`
- `redis-ping -> ping`
- `redis_ping -> ping`

### Single-Key Read And Write Aliases

- `read -> get`
- `kv-read -> get`
- `write -> set`
- `kv-write -> set`

### Counter Mutation Aliases

- `increment -> incr`
- `count-up -> incr`
- `decrement -> decr`
- `count-down -> decr`

### Multi-Key Read And Write Aliases

- `multi-read -> mget`
- `bulk-read -> mget`
- `multi-write -> mset`
- `bulk-write -> mset`

### Existence And Deletion Aliases

- `present -> exists`
- `key-check -> exists`
- `delete -> del`
- `remove -> del`

### Expiry And TTL Aliases

- `set-ttl -> expire`
- `expiry -> expire`
- `time-to-live -> ttl`
- `key-ttl -> ttl`
- `precise-ttl -> pttl`
- `ms-ttl -> pttl`

## Operator Reading Order

If you are reading this as an operator, the shortest useful map is:

1. confirm health with `ping`
2. read and write with `get` / `set`
3. mutate counters with `incr` / `decr`
4. fan out with `mget` / `mset`
5. verify presence with `exists`
6. expire and inspect lifetime with `expire`, `ttl`, and `pttl`

## Stability Notes

The current shelf keeps:

- canonical entry names as the stable reporting surface
- aliases as a convenience layer for operator lookup
- response shapes at the coarse transport-contract level needed by the current
  built-in protocol shelf
- covered commands grouped by operator task rather than by exhaustive option
  catalog

For the broader family map, see
[docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `bulk-read`
- `bulk-write`
- `connect`
- `count-down`
- `count-up`
- `decrement`
- `delete`
- `expiry`
- `health`
- `increment`
- `key-check`
- `key-ttl`
- `kv-read`
- `kv-write`
- `ms-ttl`
- `multi-read`
- `multi-write`
- `precise-ttl`
- `present`
- `read`
- `redis-get`
- `redis-ping`
- `redis-session`
- `redis-set`
- `redis_get`
- `redis_ping`
- `redis_session`
- `redis_set`
- `remove`
- `roundtrip`
- `set-ttl`
- `time-to-live`
- `write`

<!-- gewyvern:entry-aliases:end -->
