# Reference: Redis Stream Surface

Use this page when you need the current exact lookup surface for Redis
stream-oriented protocol entries in the built-in shelf.

## Covered Entries

### Data Path

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `xadd` | append one or more entries to a stream | bulk string |
| `xread` | read from one or more streams | array |
| `xrange` | inspect stream history forward | array |
| `xrevrange` | inspect stream history backward | array |
| `xdel` | delete one or more entries | integer |
| `xtrim` | trim a stream | integer |
| `xlen` | read stream length | integer |

### Group And Delivery Control

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `xack` | acknowledge delivered entries | integer |
| `xpending` | inspect pending delivery backlog | array |
| `xgroup` | manage consumer-group lifecycle | simple string |
| `xinfo` | inspect stream, group, or consumer metadata | array |

### Consumer-Group Read And Takeover

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `xreadgroup` | consume through a group cursor | array |
| `xclaim` | explicitly reassign pending entries | array |
| `xautoclaim` | reclaim idle entries automatically | array |

## Aliases

These aliases are intended to make CLI use and operator lookup friendlier.
Machine-facing reporting should still prefer the canonical entry names above.

### Data Path Aliases

- `stream-append -> xadd`
- `stream-write -> xadd`
- `stream-read -> xread`
- `stream-consume -> xread`
- `stream-range -> xrange`
- `stream-history -> xrange`
- `stream-range-reverse -> xrevrange`
- `stream-history-reverse -> xrevrange`
- `stream-delete -> xdel`
- `stream-prune-entry -> xdel`
- `stream-trim -> xtrim`
- `stream-prune -> xtrim`
- `stream-length -> xlen`
- `stream-count -> xlen`

### Delivery And Group Aliases

- `stream-ack -> xack`
- `stream-acknowledge -> xack`
- `stream-pending -> xpending`
- `stream-delivery-backlog -> xpending`
- `stream-group -> xgroup`
- `stream-consumer-group -> xgroup`
- `stream-group-manage -> xgroup`
- `stream-group-create -> xgroup`
- `stream-group-destroy -> xgroup`
- `stream-group-create-consumer -> xgroup`
- `stream-group-drop-consumer -> xgroup`
- `stream-group-setid -> xgroup`
- `stream-group-help -> xgroup`
- `stream-group-list-consumers -> xgroup`
- `stream-group-list-groups -> xgroup`
- `stream-info -> xinfo`
- `stream-inspect -> xinfo`
- `stream-info-stream -> xinfo`
- `stream-info-groups -> xinfo`
- `stream-info-consumers -> xinfo`

### Group Read And Takeover Aliases

- `stream-group-read -> xreadgroup`
- `stream-consumer-read -> xreadgroup`
- `stream-claim -> xclaim`
- `stream-reassign -> xclaim`
- `stream-auto-claim -> xautoclaim`
- `stream-idle-reassign -> xautoclaim`

## Operator Reading Order

If you are reading this as an operator, the shortest useful mental map is:

1. write or append with `xadd`
2. inspect backlog with `xpending`
3. manage lifecycle with `xgroup`
4. inspect topology with `xinfo`
5. read through groups with `xreadgroup`
6. recover stuck work with `xclaim` or `xautoclaim`
7. confirm completion with `xack`

## Stability Notes

The current shelf is intentionally conservative:

- canonical names are the stable reporting surface
- aliases are a convenience surface
- command options and subcommand nuances are not individually exposed as
  separate canonical entries yet
- response shapes are modeled only at the coarse transport-contract level
  needed by the current `stable-subset`
- covered commands grouped by data path, delivery control, and group-driven
  consumption behavior

That means `xgroup` remains one canonical entry even when aliases point at
actions like create, destroy, or setid.
Likewise, `xinfo` remains one canonical entry even when aliases point at
stream, groups, or consumers.

For the broader family map, see
[docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `stream-ack`
- `stream-acknowledge`
- `stream-append`
- `stream-auto-claim`
- `stream-claim`
- `stream-consume`
- `stream-consumer-group`
- `stream-consumer-read`
- `stream-count`
- `stream-delete`
- `stream-delivery-backlog`
- `stream-group`
- `stream-group-create`
- `stream-group-create-consumer`
- `stream-group-destroy`
- `stream-group-drop-consumer`
- `stream-group-help`
- `stream-group-list-consumers`
- `stream-group-list-groups`
- `stream-group-manage`
- `stream-group-read`
- `stream-group-setid`
- `stream-history`
- `stream-history-reverse`
- `stream-idle-reassign`
- `stream-info`
- `stream-info-consumers`
- `stream-info-groups`
- `stream-info-stream`
- `stream-inspect`
- `stream-length`
- `stream-pending`
- `stream-prune`
- `stream-prune-entry`
- `stream-range`
- `stream-range-reverse`
- `stream-read`
- `stream-reassign`
- `stream-trim`
- `stream-write`

<!-- gewyvern:entry-aliases:end -->
