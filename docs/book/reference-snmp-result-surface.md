# Reference: SNMP Result Surface

Read this page when the interesting signal is not the requested SNMP action
itself, but the outcome the remote side reported back.

Canonical entries covered here:

- `report`
- `unauthorized`

Current accepted aliases:

- `engine-report`
- `report-pdu`
- `auth-failed`
- `access-denied`

Operational split:

- `report` models a generic SNMPv3 report response
- `unauthorized` models a report-shaped authorization failure outcome

Protocol package aliases also remain accepted:

- `snmp-report`
- `snmp_report`
- `snmp-unauthorized`
- `snmp_unauthorized`

Return to the family hub:

- [docs/book/reference-snmp-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-snmp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `access-denied`
- `auth-failed`
- `engine-report`
- `report-pdu`
- `snmp-report`
- `snmp-unauthorized`
- `snmp_report`
- `snmp_unauthorized`

<!-- gewyvern:entry-aliases:end -->
