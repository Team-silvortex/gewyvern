# Reference: SNMP Failure Semantics

Read this page when SNMP traffic is present but the operator question is no
longer "which entry matched?" and has become "what kind of failure is this?"

Read it alongside the broader UDP control-family guide:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)

Use this page for:

- SNMP request timeout interpretation
- distinguishing missing replies from explicit report PDUs
- understanding why `unauthorized` is modeled as a direct denial result
- keeping stale control-plane snapshots separate from on-wire SNMP semantics

## Two Failure Classes

### 1. Missing-transition timeout

For request-style SNMP paths such as:

- `get`
- `get-next`
- `bulk`
- `set`
- `inform`

the normal failure shape is often:

- request datagram observed
- expected reply datagram not observed inside the active window

In gewyvern this should be represented as a diagnosis finding over a missing
transition, not as a fake reply protocol.

Canonical summary labels:

- `primary_failure_mode = no_response`
- `primary_failure_detail = request_sent_no_reply`
- `primary_failure_basis = missing_transition`

Typical transition examples:

- `send_get_request->receive_get_response`
- `send_get_next_request->receive_get_next_response`
- `send_bulk_request->receive_bulk_response`
- `send_set_request->receive_set_response`
- `send_inform_notification->receive_inform_response`
- `send_engine_sync_probe->receive_engine_sync_report`

### 2. Direct result PDU

Some SNMP outcomes are already explicit on the wire and should stay modeled as
real result entries.

- `report`
  use when a report PDU is actually observed
- `unauthorized`
  use when the observed report clearly expresses authorization failure

For `report`, keep it as an explicit observed result surface first. It should
not be collapsed into a timeout diagnosis. In the current `1.17.x` line, the
expected diagnosis posture is:

- `primary_failure_mode = semantic_error`
- `primary_failure_detail = protocol_error`
- `primary_failure_basis = direct_protocol_signal`

For `unauthorized`, the expected diagnosis posture is:

- `primary_failure_mode = server_denied`
- `primary_failure_detail = access_denied`
- `primary_failure_basis = direct_protocol_signal`

## What "stale" means here

If the control plane says the latest snapshot is stale, that is a runtime or
snapshot freshness issue, not a new SNMP wire-level entry.

Treat it separately from:

- a request timeout
- a report PDU
- an authorization failure report

Read next:

1. [docs/book/reference-snmp-surface.md](docs/book/reference-snmp-surface.md)
2. [docs/book/reference-snmp-result-surface.md](docs/book/reference-snmp-result-surface.md)
3. [docs/book/reference-protocol-operator-playbook.md](docs/book/reference-protocol-operator-playbook.md)
