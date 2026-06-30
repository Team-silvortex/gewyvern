# Pathological Container Fixtures

These fixtures are intentionally small malformed clients for runtime ingest
resilience testing.

They are not exploit kits and they do not target third-party services. Each
scenario connects only to the `gewyvern` runtime container started by:

```bash
bash scripts/validation/pathological_container_validation.sh
```

## Scenarios

- `truncated-json`: sends a partial JSON object and closes the socket.
- `empty-disconnect`: opens a TCP connection and closes it without sending a
  valid fact line.
- `oversize-line`: sends one malformed line larger than the runtime ingest
  safety limit.
- `slow-drip`: sends an incomplete JSON prefix slowly, then closes the socket.

The validation script treats these as bad-client fixtures. The expected result
is not a process crash; it is structured runtime degradation, log evidence, and
continued service after a later healthy ingest.
