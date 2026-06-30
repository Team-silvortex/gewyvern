# Reference: Management UDP Role Matrix

Use this page when the operator already knows the traffic belongs to the
management-UDP shelf, but still needs to answer one framing question before
drilling into a specific family page:

- is this a request-and-reply control path?
- a lease / progress path?
- an explicit denial path?
- a one-way notification path?
- a management synchronization path?
- or a result packet that should stay distinct from timeout?

Read this alongside:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)

## Role Matrix

| family | role | canonical entries | exchange shape | operator stance | read next |
| --- | --- | --- | --- | --- | --- |
| SNMP | read | `get`, `get-next`, `bulk` | request and reply | ordinary query flow; timeout means missing reply | [docs/book/reference-snmp-read-surface.md](docs/book/reference-snmp-read-surface.md) |
| SNMP | write | `set` | request and reply | mutation flow; timeout means missing acknowledgement-style reply | [docs/book/reference-snmp-set-surface.md](docs/book/reference-snmp-set-surface.md) |
| SNMP | notify-send | `trap` | one-way notification | outbound notification, usually toward `162` | [docs/book/reference-snmp-trap-surface.md](docs/book/reference-snmp-trap-surface.md) |
| SNMP | notify-send-with-reply | `inform` | notification plus expected reply | notification semantics, but a response still matters | [docs/book/reference-snmp-notify-surface.md](docs/book/reference-snmp-notify-surface.md) |
| SNMP | notify-recv | `trap-recv` | inbound notification receive | local trap listener, usually on `162` | [docs/book/reference-snmp-manage-surface.md](docs/book/reference-snmp-manage-surface.md) |
| SNMP | management | `engine-sync` | request and report-style reply | engine discovery / synchronization | [docs/book/reference-snmp-manage-surface.md](docs/book/reference-snmp-manage-surface.md) |
| SNMP | explicit result | `report`, `unauthorized` | observed terminal result | treat as explicit result or denial, not plain timeout | [docs/book/reference-snmp-result-surface.md](docs/book/reference-snmp-result-surface.md) |
| NTP | query | `query`, `client` | request and reply | normal time query posture | [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md) |
| NTP | sync | `sync` | request and reply | time discipline / sync follow-up | [docs/book/reference-ntp-time-surface.md](docs/book/reference-ntp-time-surface.md) |
| DHCP | client | `client`, `discover` | discovery and offer progress | treat offer as lease progress, not denial | [docs/book/reference-dhcp-client-surface.md](docs/book/reference-dhcp-client-surface.md) |
| DHCP | lease-confirm | `request` | request and acknowledgement | treat `ack` as lease confirmation | [docs/book/reference-dhcp-lease-surface.md](docs/book/reference-dhcp-lease-surface.md) |
| DHCP | denial | `nak` | explicit terminal reply | explicit rejection from server | [docs/book/reference-dhcp-failure-semantics.md](docs/book/reference-dhcp-failure-semantics.md) |
| STUN | binding | `binding` | request and reply | reachability / mapping check | [docs/book/reference-stun-binding-surface.md](docs/book/reference-stun-binding-surface.md) |
| STUN | binding-result | `binding-error` | explicit terminal reply | explicit protocol-side error, not timeout | [docs/book/reference-stun-failure-semantics.md](docs/book/reference-stun-failure-semantics.md) |
| STUN | relay | `allocate`, `refresh` | request and reply | relay state / maintenance path | [docs/book/reference-stun-relay-surface.md](docs/book/reference-stun-relay-surface.md) |

## Fast Picks

- If you see a missing reply after a control datagram left the runtime, move to:
  [docs/book/reference-management-udp-diagnosis-matrix.md](docs/book/reference-management-udp-diagnosis-matrix.md)
- If you already know the family and just need the family hub, jump to:
  [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md),
  [docs/book/reference-ntp-surface.md](docs/book/reference-ntp-surface.md),
  [docs/book/reference-dhcp-surface.md](docs/book/reference-dhcp-surface.md),
  [docs/book/reference-stun-surface.md](docs/book/reference-stun-surface.md)
- If the question is not "what role is this?" but "what failed?", switch to:
  [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)

Return path:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)
