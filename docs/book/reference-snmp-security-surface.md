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

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)
