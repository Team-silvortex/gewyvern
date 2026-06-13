# Reference: Redis Hash Surface

Use this page when you need the current exact lookup surface for Redis
hash-oriented protocol entries in the built-in shelf.

For the broader family/entry contract, see:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)

## Canonical Entries

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

This page keeps the Redis hash shelf intentionally conservative:

- canonical entry names are the stable reporting surface
- aliases are a convenience layer for CLI/operator lookup
- the exposed shelf focuses on common single-field and multi-field access
  patterns needed by current protocol lookup and IR/runtime validation

## Validation Surface

The current repository validates this Redis hash shelf through:

- [/Users/Shared/chroot/dev/gewyvern/tests/redis_protocol_registry_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/redis_protocol_registry_tdd.rs)

For IR-level support confirmation, use:

- [/Users/Shared/chroot/dev/gewyvern/src/bin/gewyc_ir_snapshot.rs](/Users/Shared/chroot/dev/gewyvern/src/bin/gewyc_ir_snapshot.rs)
