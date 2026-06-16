# Reference: Redis Protocol Surface

Use this page when you want the Redis portion of the built-in protocol shelf
as a stable lookup surface instead of a tutorial.

This page groups the current Redis coverage into seven narrower shelves so the
lookup path stays predictable as the protocol family grows.

## What This Shelf Covers

The current Redis surface is organized by operator intent:

- session and key-value traffic
- publish and subscribe traffic
- set membership traffic
- hash field reads and writes
- list push/pop/move flows
- sorted-set mutation and ranked lookup
- stream publish, backlog, group, and claim flow

Each subpage focuses on:

- canonical entry names
- aliases accepted by the current registry
- coarse request and response shape
- operator reading order
- current validation/lowering posture

## Family Aliases

The current registry also accepts these family-level spellings for Redis entry
selection:

- `redis-get`
- `redis-ping`
- `redis-session`
- `redis-set`
- `redis_get`
- `redis_ping`
- `redis_session`
- `redis_set`

Default entry: `ping`

## Redis Surface Map

### Key-Value And Session

- [docs/book/reference-redis-kv-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-kv-surface.md)
  Session, ping, single-key read/write, counters, multi-key reads/writes,
  existence checks, delete, and TTL control.

Typical entries:

- `session`
- `ping`
- `get`
- `set`
- `incr`
- `decr`
- `mget`
- `mset`
- `exists`
- `del`
- `expire`
- `ttl`
- `pttl`

### Hash

- [docs/book/reference-redis-hash-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-hash-surface.md)
  Single-field and multi-field hash operations.

Typical entries:

- `hget`
- `hset`
- `hmget`
- `hmset`

### Publish And Subscribe

- [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
  Channel publish and subscribe flow, kept on the family hub until a narrower
  pub/sub subpage is warranted.

Typical entries:

- `publish`
- `subscribe`

### Set

- [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
  Set member addition and full member listing, kept on the family hub until a
  narrower set subpage is warranted.

Typical entries:

- `sadd`
- `smembers`

### List

- [docs/book/reference-redis-list-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-list-surface.md)
  Left/right push-pop flows, blocking variants, move flows, and multi-pop.

Typical entries:

- `lpush`
- `rpush`
- `lpop`
- `rpop`
- `blpop`
- `brpop`
- `rpoplpush`
- `brpoplpush`
- `lmove`
- `blmove`
- `lmpop`
- `blmpop`

### Sorted Set

- [docs/book/reference-redis-sorted-set-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-sorted-set-surface.md)
  Score mutation, score lookup, ranked reads, and pop/multi-pop flows.

Typical entries:

- `zadd`
- `zcard`
- `zcount`
- `zincrby`
- `zrank`
- `zrem`
- `zrevrangebyscore`
- `zrevrank`
- `zscore`
- `zrange`
- `zrangebyscore`
- `zpopmin`
- `zpopmax`
- `bzpopmin`
- `bzpopmax`
- `zmpop`
- `bzmpop`

### Stream

- [docs/book/reference-redis-stream-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-stream-surface.md)
  Append/read flow, group management, pending inspection, consumer reads,
  claim/takeover, and stream metadata inspection.

Typical entries:

- `xadd`
- `xread`
- `xack`
- `xpending`
- `xgroup`
- `xreadgroup`
- `xclaim`
- `xautoclaim`
- `xdel`
- `xinfo`

## Reading Order

If you are checking current Redis capability coverage, the shortest useful
order is:

1. [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
2. [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
3. one narrower Redis subpage for the command family you care about
4. [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Stability Note

This page is the lookup hub for the current Redis family in the `0.15.x` line.
New Redis command families should prefer landing behind this shelf instead of
being linked from multiple higher-level pages independently.
