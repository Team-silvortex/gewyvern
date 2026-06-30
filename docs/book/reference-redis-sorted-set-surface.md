# Reference: Redis Sorted Set Surface

Use this page when you need the current exact lookup surface for Redis
sorted-set-oriented protocol entries in the built-in shelf.

## Covered Entries

### Mutation

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `zadd` | add scored members | integer |
| `zrem` | remove members | integer |
| `zincrby` | bump one member score | bulk string |

### Point And Rank Lookup

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `zscore` | read one member score | bulk string |
| `zrank` | read one member rank | integer |
| `zrevrank` | read one member reverse rank | integer |
| `zcard` | count members in the set | integer |

### Range And Score Window

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `zrange` | range-read by rank | array |
| `zrangebyscore` | range-read by score window | array |
| `zrevrangebyscore` | reverse score-window read | array |
| `zcount` | count a score window | integer |

### Pop And Multi-Pop

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `zpopmin` | pop lowest-scored member | array |
| `zpopmax` | pop highest-scored member | array |
| `zmpop` | pop from one of several sorted sets | array |
| `bzpopmin` | blocking lowest pop | array |
| `bzpopmax` | blocking highest pop | array |
| `bzmpop` | blocking multi-pop | array |

## Aliases

### Mutation Aliases

- `sorted-add -> zadd`
- `score-add -> zadd`
- `sorted-remove -> zrem`
- `score-remove -> zrem`
- `sorted-score-increment -> zincrby`
- `score-bump -> zincrby`

### Point And Rank Aliases

- `sorted-member-score -> zscore`
- `score-read-member -> zscore`
- `sorted-member-rank -> zrank`
- `score-rank-member -> zrank`
- `sorted-member-revrank -> zrevrank`
- `score-revrank-member -> zrevrank`
- `sorted-count -> zcard`
- `score-count -> zcard`

### Range And Score Window Aliases

- `sorted-read -> zrange`
- `score-read -> zrange`
- `sorted-range-score -> zrangebyscore`
- `score-window-read -> zrangebyscore`
- `sorted-revrange-score -> zrevrangebyscore`
- `score-window-read-reverse -> zrevrangebyscore`
- `sorted-range-count -> zcount`
- `score-window-count -> zcount`

### Pop And Multi-Pop Aliases

- `sorted-pop-min -> zpopmin`
- `score-pop-lowest -> zpopmin`
- `sorted-pop-max -> zpopmax`
- `score-pop-highest -> zpopmax`
- `sorted-multi-pop -> zmpop`
- `score-pop-many -> zmpop`
- `sorted-blocking-pop-min -> bzpopmin`
- `score-blocking-pop-lowest -> bzpopmin`
- `sorted-blocking-pop-max -> bzpopmax`
- `score-blocking-pop-highest -> bzpopmax`
- `sorted-blocking-multi-pop -> bzmpop`
- `score-blocking-pop-many -> bzmpop`

## Operator Reading Order

If you are reading this as an operator, the shortest useful map is:

1. insert or update with `zadd`
2. inspect members with `zscore`, `zrank`, or `zrange`
3. track windows with `zrangebyscore` and `zcount`
4. increment scores with `zincrby`
5. drain extremes with `zpopmin` or `zpopmax`
6. batch-drain across sets with `zmpop` or `bzmpop`

## Stability Notes

The current shelf deliberately keeps:

- one canonical entry per main sorted-set command family
- aliases as human-facing lookup sugar
- response-shape modeling at the coarse transport level only
- covered commands grouped by mutation, rank lookup, score-window lookup, and
  pop behavior

That means this reference is meant for resolution and operator lookup, not for
full Redis command-option documentation.

For the broader family map, see
[docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md).

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `score-add`
- `score-blocking-pop-highest`
- `score-blocking-pop-lowest`
- `score-blocking-pop-many`
- `score-bump`
- `score-count`
- `score-pop-highest`
- `score-pop-lowest`
- `score-pop-many`
- `score-rank-member`
- `score-read`
- `score-read-member`
- `score-remove`
- `score-revrank-member`
- `score-window-count`
- `score-window-read`
- `score-window-read-reverse`
- `sorted-add`
- `sorted-blocking-multi-pop`
- `sorted-blocking-pop-max`
- `sorted-blocking-pop-min`
- `sorted-count`
- `sorted-member-rank`
- `sorted-member-revrank`
- `sorted-member-score`
- `sorted-multi-pop`
- `sorted-pop-max`
- `sorted-pop-min`
- `sorted-range-count`
- `sorted-range-score`
- `sorted-read`
- `sorted-remove`
- `sorted-revrange-score`
- `sorted-score-increment`

<!-- gewyvern:entry-aliases:end -->
