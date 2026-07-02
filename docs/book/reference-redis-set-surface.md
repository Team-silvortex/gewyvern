# Reference: Redis Set Surface

Use this page when you need the current exact lookup surface for Redis set
membership traffic.

For the broader family map, see
[docs/book/reference-redis-surface.md](docs/book/reference-redis-surface.md).

## Covered Entries

| Canonical entry | Typical intent | Response shape |
| --- | --- | --- |
| `sadd` | add one or more members to a set | integer |
| `smembers` | read all members from a set | array |

## Aliases

- `set-add -> sadd`
- `member-add -> sadd`
- `set-read -> smembers`
- `members-read -> smembers`

## Operator Reading Order

If you are reading this as an operator, the shortest useful map is:

1. confirm Redis reachability with `ping`
2. add members with `sadd`
3. inspect membership with `smembers`
4. switch to sorted-set surfaces when score ordering matters

## Stability Notes

This surface keeps unordered set membership separate from key-value writes and
sorted-set ranking. It is intentionally compact until intersection, union, scan,
and removal entries become first-class protocol paths.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `member-add`
- `members-read`
- `set-add`
- `set-read`

<!-- gewyvern:entry-aliases:end -->
