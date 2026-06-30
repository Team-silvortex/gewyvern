# Reference: SNMP Security Surface

Read this page when the path is clearly SNMPv3 and the main question is whether
traffic is authenticated only or privacy-protected.

Canonical entries covered here:

- `v3-auth`
- `v3-priv`

Current accepted aliases:

- `auth-user`
- `auth-session`
- `private-session`
- `encrypted-session`

Operational split:

- `v3-auth` models authenticated SNMPv3 exchanges without privacy protection
- `v3-priv` models privacy-protected SNMPv3 exchanges where payload secrecy is expected

Protocol package aliases also remain accepted:

- `snmp-v3-auth`
- `snmp_v3_auth`
- `snmp-v3-priv`
- `snmp_v3_priv`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `auth-session`
- `auth-user`
- `encrypted-session`
- `private-session`
- `snmp-v3-auth`
- `snmp-v3-priv`
- `snmp_v3_auth`
- `snmp_v3_priv`

<!-- gewyvern:entry-aliases:end -->
