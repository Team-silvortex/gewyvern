# Reference: Management UDP Diagnosis Matrix

Use this page when you want a compact, structured mapping from:

- protocol family
- canonical entry
- suspicious transition
- expected diagnosis labels

This is the maintenance table for the current `1.14.x` management-UDP shelf.

Read alongside:

- [docs/book/reference-management-udp-failure-semantics.md](docs/book/reference-management-udp-failure-semantics.md)
- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

## Timeout Matrix

| family | entry | transition | expected mode | expected detail | expected basis |
| --- | --- | --- | --- | --- | --- |
| SNMP | `get` | `send_get_request->receive_get_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| SNMP | `get-next` | `send_get_next_request->receive_get_next_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| SNMP | `bulk` | `send_bulk_request->receive_bulk_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| SNMP | `set` | `send_set_request->receive_set_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| SNMP | `inform` | `send_inform_notification->receive_inform_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| SNMP | `engine-sync` | `send_engine_sync_probe->receive_engine_sync_report` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| NTP | `query` | `send_query->receive_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| NTP | `sync` | `send_sync_request->receive_sync_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| DHCP | `discover` | `send_discover->receive_offer` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| DHCP | `request` | `send_request->receive_ack` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| STUN | `binding` | `send_request->receive_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| STUN | `allocate` | `send_allocate_request->receive_allocate_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |
| STUN | `refresh` | `send_refresh_request->receive_refresh_response` | `no_response` | `request_sent_no_reply` | `missing_transition` |

## Direct-Signal Matrix

| family | entry | observed terminal phase | expected mode | expected detail | expected basis |
| --- | --- | --- | --- | --- | --- |
| DHCP | `nak` | `receive_nak` | `server_denied` | `request_rejected` | `direct_protocol_signal` |
| SNMP | `report` | `receive_report_pdu` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |
| SNMP | `unauthorized` | `receive_authorization_failure_report` | `server_denied` | `access_denied` | `direct_protocol_signal` |
| STUN | `binding-error` | `receive_error_response` | `semantic_error` | `protocol_error` | `direct_protocol_signal` |

## Result-Surface Matrix

These entries are important observed outcomes, but should not automatically be
collapsed into denial or timeout without extra context.

| family | entry | observed phase | operator stance |
| --- | --- | --- | --- |
| SNMP | `report` | `receive_report_pdu` | treat as explicit result, not timeout |
| DHCP | `discover` | `receive_offer` | treat as lease-progress result |
| DHCP | `request` | `receive_ack` | treat as lease-confirmation result |
| STUN | `allocate` | `receive_allocate_response` | treat as relay-state result |
| STUN | `refresh` | `receive_refresh_response` | treat as relay-maintenance result |

## Notes

- `stale snapshot` is intentionally excluded from the matrix because it is a
  control-plane freshness issue, not a wire-level protocol outcome.
- If a future family needs a stable `not_sent` row, add it only when the
  runtime clearly distinguishes "follow-up never emitted" from "request emitted
  but reply missing" for that path.
