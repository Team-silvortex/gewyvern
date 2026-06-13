# Reference: Redis List Surface

Use this page when you need the current exact lookup surface for Redis
list-oriented protocol entries in the built-in shelf.

For the broader family/entry contract, see:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)

For the Redis stream-specific shelf, see:

- [docs/book/reference-redis-stream-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-stream-surface.md)

## Canonical Entries

### Push And Append

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `lpush` | prepend to a list | integer |
| `rpush` | append to a list | integer |

### Single-Item Pop

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `lpop` | pop from the left side | bulk string |
| `rpop` | pop from the right side | bulk string |
| `blpop` | block until a left-side pop succeeds | array |
| `brpop` | block until a right-side pop succeeds | array |

### Directional Move

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `rpoplpush` | fixed right-to-left move | bulk string |
| `brpoplpush` | fixed blocking right-to-left move | bulk string |
| `lmove` | directional move with configurable source/destination side | bulk string |
| `blmove` | blocking directional move | bulk string |

### Multi-Pop

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `lmpop` | multi-pop from one of several lists | array |
| `blmpop` | blocking multi-pop | array |

## Aliases

### Push And Append Aliases

- `list-prepend -> lpush`
- `left-push -> lpush`
- `list-append -> rpush`
- `right-push -> rpush`

### Single-Item Pop Aliases

- `list-pop-left -> lpop`
- `left-pop -> lpop`
- `list-pop-right -> rpop`
- `right-pop -> rpop`
- `list-blocking-pop-left -> blpop`
- `left-blocking-pop -> blpop`
- `list-blocking-pop-right -> brpop`
- `right-blocking-pop -> brpop`

### Directional Move Aliases

- `list-move-right-to-left -> rpoplpush`
- `right-pop-left-push -> rpoplpush`
- `list-blocking-move-right-to-left -> brpoplpush`
- `right-blocking-pop-left-push -> brpoplpush`
- `list-move -> lmove`
- `list-directional-move -> lmove`
- `left-right-move -> lmove`
- `right-left-move -> lmove`
- `list-blocking-move -> blmove`
- `list-blocking-directional-move -> blmove`
- `blocking-left-right-move -> blmove`
- `blocking-right-left-move -> blmove`

### Multi-Pop Aliases

- `list-multi-pop -> lmpop`
- `list-pop-many -> lmpop`
- `list-blocking-multi-pop -> blmpop`
- `blocking-list-pop-many -> blmpop`

## Operator Reading Order

If you are reading this as an operator, the shortest useful map is:

1. write queue head with `lpush`
2. write queue tail with `rpush`
3. drain one item with `lpop` or `rpop`
4. wait for work with `blpop` or `brpop`
5. transfer ownership with `lmove` or `rpoplpush`
6. batch-drain with `lmpop` or `blmpop`

## Stability Notes

The current shelf intentionally treats:

- `rpoplpush` and `brpoplpush` as fixed-direction legacy-style entries
- `lmove` and `blmove` as the more semantic directional family
- `lmpop` and `blmpop` as the batch-oriented family

The canonical entries are the stable reporting surface.
Aliases are there for human lookup and CLI ergonomics.

## Validation Surface

The current repository validates this Redis list shelf through:

- [/Users/Shared/chroot/dev/gewyvern/tests/redis_list_move_protocol_registry_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/redis_list_move_protocol_registry_tdd.rs)
- [/Users/Shared/chroot/dev/gewyvern/tests/redis_blocking_pop_protocol_registry_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/redis_blocking_pop_protocol_registry_tdd.rs)

For IR-level support confirmation, use:

- [/Users/Shared/chroot/dev/gewyvern/src/bin/gewyc_ir_snapshot.rs](/Users/Shared/chroot/dev/gewyvern/src/bin/gewyc_ir_snapshot.rs)
