# Reference: Redis Failure Semantics

Use this page when you want the narrow operator contract for Redis failures
that now have explicit built-in protocol paths.

## Covered Failure Entries

The current Redis family includes these failure-oriented entries:

- `auth-required`
- `auth-denied`
- `error`
- `wrongtype`
- `busygroup`
- `readonly`
- `noscript`
- `moved`
- `ask`
- `tryagain`
- `loading`
- `crossslot`
- `clusterdown`
- `masterdown`
- `oom`
- `busy`
- `execabort`
- `misconf`

They are intentionally narrow. Each one exists so runtime diagnosis can lean
on a direct Redis signal instead of only inferring a timeout or missing stage.

## Current Diagnosis Contract

| Entry | Typical signal | Failure mode | Failure detail | Basis |
| --- | --- | --- | --- | --- |
| `auth-required` | `-NOAUTH` | `server_denied` | `auth_required` | `direct_protocol_signal` |
| `auth-denied` | `-WRONGPASS` | `server_denied` | `access_denied` | `direct_protocol_signal` |
| `error` | `-ERR` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `wrongtype` | `-WRONGTYPE` | `semantic_error` | `protocol_constraint_violation` | `direct_protocol_signal` |
| `busygroup` | `-BUSYGROUP` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `readonly` | `-READONLY` | `server_denied` | `access_denied` | `direct_protocol_signal` |
| `noscript` | `-NOSCRIPT` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `moved` | `-MOVED` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `ask` | `-ASK` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `tryagain` | `-TRYAGAIN` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `loading` | `-LOADING` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `crossslot` | `-CROSSSLOT` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `clusterdown` | `-CLUSTERDOWN` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `masterdown` | `-MASTERDOWN` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `oom` | `-OOM` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `busy` | `-BUSY` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `execabort` | `-EXECABORT` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| `misconf` | `-MISCONF` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |

## Operator Reading Order

1. [docs/book/reference-redis-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-redis-surface.md)
2. this page
3. [docs/book/reference-diagnosis-spine.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-diagnosis-spine.md)

## Scope Note

These entries do not try to model every Redis error string yet. They provide a
stable first shelf for the most actionable failure classes in the `0.16.x`
line: authentication pressure, generic command errors, type/constraint
mismatch, cluster redirects, failover windows, and write-path refusal signals.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `backoff-retry`
- `cluster-ask`
- `cluster-redirect`
- `cluster-retry`
- `cluster-slot-conflict`
- `cluster-unavailable`
- `command-error`
- `consumer-group-exists`
- `evalsha-miss`
- `failover-window`
- `loading-window`
- `login-denied`
- `login-required`
- `lua-blocked`
- `memory-limit`
- `multi-exec-abort`
- `multi-key-slot-conflict`
- `noauth`
- `persistence-misconfig`
- `primary-unavailable`
- `readonly-replica`
- `replica-write-denied`
- `resp-error`
- `script-busy`
- `script-missing`
- `slot-ask`
- `slot-map-down`
- `slot-moved`
- `stream-group-exists`
- `transaction-abort`
- `type-conflict`
- `warmup-busy`
- `write-guarded`
- `write-over-capacity`
- `wrong-type`
- `wrongpass`

<!-- gewyvern:entry-aliases:end -->
