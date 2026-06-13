# Reference: Redis List Surface

Use this page when you need the current exact lookup surface for Redis
list-oriented protocol entries in the built-in shelf.

## Covered Entries

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

The canonical entries remain the stable reporting surface, while aliases stay
human-oriented for operator lookup and CLI ergonomics.
Covered commands stay grouped by push/pop, move, and multi-pop behavior so the
lookup path remains easy to scan.

For the broader family map, see
[docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md).
