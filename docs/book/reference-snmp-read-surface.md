# Reference: SNMP Read Surface

Read this page when the path is polling or walking rather than mutating.

Canonical entries covered here:

- `bulk`
- `get`
- `get-next`

Current accepted aliases:

- `bulk-walk`
- `query`
- `read`
- `table-read`
- `walk`
- `next`

Operational split:

- `bulk` expands one request across broader table-style retrieval
- `get` reads one requested value set
- `get-next` advances one step through ordered OID traversal

Protocol package aliases also remain accepted:

- `snmp-bulk`
- `snmp_bulk`
- `snmp-get-next`
- `snmp_get_next`

## Evidence Boundary

The read packages use `snmp_pdu_observed`, not a fixed byte-13 PDU fingerprint.
They require a complete community BER message in at most 128 consecutive
sampled payload bytes. GET/GETNEXT cover v1/v2c shapes; GETBULK is v2c-only.
The generic Response PDU is shared by these operations and can carry error
statuses or exception values. A receive phase alone does not prove a successful
read or identify which request caused it. Sparse collectors must retain explicit
unsupported-offset diagnostics rather than silently falling back to fingerprints.

The [Linux SNMP campaign](../script-entrypoints.md) uses a real read-only
Net-SNMP agent and independent native client checks. It covers one-varbind reads,
one-repetition bulk, noSuchObject, wrong-community timeout/recovery and separate
sockets reusing an ID. It does not establish a complete MIB walk, v3 security,
SET/notifications, general throughput, or production request correlation.

Return to the family hub:

- [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `bulk-walk`
- `next`
- `query`
- `read`
- `snmp-bulk`
- `snmp-get-next`
- `snmp_bulk`
- `snmp_get_next`
- `table-read`
- `walk`

<!-- gewyvern:entry-aliases:end -->
