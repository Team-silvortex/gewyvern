# Authenticated runtime deployment

Leserpent can submit a typed deployment intent directly to an already-running gewyvern runtime. This path uses the runtime API authentication protocol; it does not invoke SSH or expose a general-purpose remote command surface.

## Runtime preparation

Start gewyvern with an explicit remote API bind and admin token:

```bash
GEWY_API_ADMIN_TOKEN='<strong-random-token>' gewyvern \
  --protocol http --entry request --serve \
  --tcp-socket 0.0.0.0:9000 \
  --api-socket 0.0.0.0:9100 \
  --allow-remote-api
```

Register that runtime in Leserpent using the same value in `pairingToken`. The token is held in Leserpent memory only: it is not included in API responses, JSON state, SQLite, or deployment request bodies. A Leserpent restart therefore requires the operator to pair the runtime again.

Capability discovery must report `control.authenticated_deployment` as `fully_supported` before Leserpent enables direct deployment.

## Submit a deployment

```http
POST /v1/runtimes/{runtimeId}/deployments
Content-Type: application/json
X-Leserpent-Intent: mutate

{
  "pipelineKind": "http/request",
  "requestedBy": "operator@example",
  "confirmed": true,
  "requestId": "deploy-20260713-001",
  "target": "pid:4242"
}
```

Leserpent forwards the request to `POST /v1/deployments` with `X-Gewyvern-Admin-Token`, then records the accepted operation in Orchestra history. `requestId` is the idempotency key: an identical retry returns the original deployment, while reuse for a different payload returns a conflict.

## Current status boundary

The first protocol version returns `status: accepted`. This means the authenticated runtime validated and registered the deployment intent. It does not yet claim that an eBPF program has attached or that a debugging session is producing events. Future execution states will extend this contract without changing its authentication and idempotency boundary.

The runtime accepts at most 16 KiB request bodies, rejects unknown fields and control characters, and requires a valid runtime admin token even for loopback deployment requests.
