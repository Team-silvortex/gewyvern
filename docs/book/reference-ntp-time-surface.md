# Reference: NTP Time Surface

Read this page when observing NTP probes or candidate time replies. These
packages observe traffic; they do not adjust the local clock.

Canonical entries covered here:

- `query`
- `sync`

Current accepted aliases:

- `probe`
- `check`
- `clock-sync`
- `time-sync`

Operational split:

- `query` probes or checks remote time state
- `sync` identifies candidate time replies with usable LI/stratum and nonzero timestamps, not a clock-following action or proof of synchronization

Both entries accept NTPv3 and NTPv4 requests. The version bits do not distinguish
query intent from clock discipline. `query` retains generic responses including
unsynchronised/KoD status, while `sync` excludes those from its receive phase.
The predicate requires the complete 48-byte base header, does not join sparse
samples, correlate originate timestamps with requests, or authenticate trailing
extensions/MACs. Local-reference servers can appear usable without accurate time.

The [Linux campaign](../script-entrypoints.md) runs private chrony instances
with and without a local reference, with `-x` and no `SYS_TIME` capability.
Neither the daemon nor its Rust test client sets the host clock; no external
time source is configured. This is bounded protocol observation, not NTP
accuracy, clock-discipline or NTS certification.

Protocol package aliases also remain accepted:

- `ntp-query`
- `ntp_query`
- `ntp-sync`
- `ntp_sync`

Return to the family hub:

- [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `check`
- `clock-sync`
- `ntp-query`
- `ntp-sync`
- `ntp_query`
- `ntp_sync`
- `probe`
- `time-sync`

<!-- gewyvern:entry-aliases:end -->
