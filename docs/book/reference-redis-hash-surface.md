# Reference: Redis Hash Surface

Use this page when you need the current exact lookup surface for Redis
hash-oriented protocol entries in the built-in shelf.

## Covered Entries

### Single-Field Read And Write

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `hget` | read one field from a hash | bulk string |
| `hset` | write one field into a hash | integer |

### Multi-Field Read And Write

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `hmget` | read multiple fields from a hash | array |
| `hmset` | write multiple fields into a hash | simple string |

## Aliases

### Single-Field Aliases

- `hash-read -> hget`
- `field-read -> hget`
- `hash-write -> hset`
- `field-write -> hset`

### Multi-Field Aliases

- `hash-multi-read -> hmget`
- `fields-read -> hmget`
- `hash-multi-write -> hmset`
- `fields-write -> hmset`

## Operator Reading Order

If you are reading this as an operator, the shortest useful map is:

1. inspect a single field with `hget`
2. update a single field with `hset`
3. batch-read fields with `hmget`
4. batch-write fields with `hmset`

## Stability Notes

The current shelf keeps:

- canonical entry names as the stable reporting surface
- aliases as a convenience layer for operator lookup
- focus on the common single-field and multi-field access patterns needed by
  current protocol lookup and IR/runtime validation
- covered commands grouped by single-field and multi-field operator tasks

For the broader family map, see
[docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md).
