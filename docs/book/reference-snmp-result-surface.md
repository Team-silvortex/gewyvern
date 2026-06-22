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

## Machine-Readable Surface Semantics

The `protocol_surface("snmp", "unauthorized")` contract now publishes
`entry_semantics` so tooling can classify explicit authorization failure without
scraping this page.

Current denial semantics:

- `category = failure-path`
- `operator_focus = authorization failure report after SNMPv3 access evaluation`
- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

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
