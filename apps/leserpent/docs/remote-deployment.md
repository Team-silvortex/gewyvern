# Authenticated runtime deployment

Leserpent can submit a typed deployment intent directly to an already-running gewyvern runtime. This path uses the runtime API authentication protocol; it does not invoke SSH or expose a general-purpose remote command surface.

## Runtime preparation

Use the authenticated installer flow or put a same-host TLS reverse proxy in
front of a loopback Gewyvern API. For the proxy pattern, keep Gewyvern itself
on loopback and use a strong token:

```bash
GEWY_API_ADMIN_TOKEN="$(openssl rand -hex 32)" gewyvern \
  --protocol http --entry request --serve \
  --tcp-socket 0.0.0.0:9000 \
  --api-socket 127.0.0.1:9100
```

Terminate TLS on the same host and expose an `https://` runtime endpoint with
a certificate trusted by the Leserpent host. Leserpent refuses to forward a
runtime or sidecar admin token over non-loopback HTTP. The legacy
`--allow-remote-api` switch is plaintext and is suitable only inside an
authenticated encrypted overlay; it is not the recommended deployment path.

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

Leserpent forwards the request to `POST /v1/deployments` with
`X-Gewyvern-Admin-Token` only over HTTPS or loopback HTTP, then records the
accepted operation in Orchestra history. `requestId` is the idempotency key:
an identical retry returns the original deployment, while reuse for a
different payload returns a conflict.

## Current status boundary

The first protocol version returns `status: accepted`. This means the authenticated runtime validated and registered the deployment intent. It does not yet claim that an eBPF program has attached or that a debugging session is producing events. Future execution states will extend this contract without changing its authentication and idempotency boundary.

The runtime accepts at most 16 KiB request bodies, rejects unknown fields and control characters, and requires a valid runtime admin token even for loopback deployment requests.
