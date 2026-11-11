# External Analysis Providers

Etragon consumes existing inference, not a self-developed model stack. Its job is
to prepare evidence, call a selected provider and validate an optional opinion.
Model training, model serving and model downloads belong to the external service.

```text
Gewyvern diagnosis snapshot
  -> Etragon allowlisted evidence
  -> operator-selected API / local model service
  -> validated advisory augmentations
  -> operator review (not automatic execution)
```

## Local Model Service

Start your own compatible model service separately, then create `provider.json`:

```json
{
  "id": "local-advisor",
  "endpoint": "http://127.0.0.1:11434/v1/chat/completions",
  "model": "your-installed-model",
  "timeout_ms": 30000,
  "max_tokens": 2048
}
```

```bash
cargo run -p etragon -- analyze-json analysis.json --provider provider.json
cargo run -p etragon -- analyze-json - --provider provider.json < analysis.json
```

The driver uses a small non-streaming chat-completions subset: `model`, `messages`,
`stream: false`, `max_tokens`, and `response_format: {"type":"json_object"}`.
The endpoint must support this subset and the chosen model must follow the
advisory schema. Ollama documents a compatible local route in its
[official API guide](https://docs.ollama.com/api/openai-compatibility).
This is protocol compatibility, not a claim that every model passes evaluation.
No provider SDK, Python, Node, shell worker or external HTTP command is required.

## Remote API

Remote requests require HTTPS and explicit permission. Set the API key through
your environment/secret manager, not the JSON configuration:

```json
{
  "id": "remote-advisor",
  "endpoint": "https://your-provider.example/v1/chat/completions",
  "model": "your-selected-model",
  "allow_remote": true,
  "api_key_env": "ETRAGON_API_KEY",
  "ca_bundle": "/etc/ssl/certs/ca-certificates.crt",
  "timeout_ms": 30000,
  "max_tokens": 2048
}
```

`ca_bundle` is a PEM trust-root bundle and is required for HTTPS on every OS.
Use the correct local path; there is no insecure-certificate switch. Hostname and
certificate verification remain enabled. Config paths are relative to the caller's
working directory. Credentials are sent only in the Authorization header. Endpoint
userinfo, query strings, fragments and redirects are rejected. HTTP is permitted
only for literal loopback addresses or `localhost` (pinned to IPv4 loopback).

**Privacy boundary:** only seven diagnosis fields are sent: `primary_module_kind`,
`primary_failure_mode`, `primary_failure_detail`, `primary_failure_confidence`,
`primary_failure_basis`, `ambiguous` and `competing_hypotheses`. Raw packets,
unknown extensions and prior augmentations are excluded. This is data minimization,
not automatic secret redaction: review these fields before permitting remote
egress. A local service can itself forward requests; its policy remains your
responsibility. Etragon's permission covers its direct destination only.

## Advice Contract

The assistant's message content must be exactly a JSON object like:

```json
{
  "schema_version": "etragon-advisory-v1",
  "findings": [
    {
      "summary": "A missing reply may indicate a timeout; this is not proof.",
      "evidence_refs": ["primary_failure_mode", "primary_failure_basis"],
      "recommendation": "Inspect a second bounded capture window."
    }
  ]
}
```

Unknown fields, duplicate typed fields, unsupported versions, nonexistent or
duplicate evidence references, tool/function calls, refusals and truncated
completions are rejected. Empty findings are valid. Citation validation proves
only that a referenced field exists, not that the model's reasoning is correct.
There is no safety guarantee from the prompt alone; the host enforces the schema
and exposes no execution tools to this adapter.

Output uses the existing `augmentations` envelope. Etragon, not the model, stamps
`kind: external-advisory`, `confidence: candidate`, provider/model provenance,
`advisory_only: true` and `operator_review_required: true`. Suggestions cannot
rewrite diagnosis fields, alter permissions or become automatic tool calls.

Bounds: 16 KiB configuration, 256 KiB input snapshot, 128 KiB response body,
16 findings, 4096 bytes per finding text, seven unique reference names. Diagnosis
strings are capped at 4096 bytes and hypotheses at 32 entries of 1024 bytes each.
Timeout is 100-120000 ms, output budget 128-8192 tokens. Response framing accepts
Content-Length, bounded chunked bodies without trailers, or close-delimited JSON;
compressed and streaming responses are not supported. No automatic retries occur.
DNS/connect/TLS/request/response share the network deadline. Up to four outstanding
OS DNS lookups may finish after caller timeout; no unbounded resolver spawning.

Single-input failures return a stable `etragon_provider_*` error code on stderr and exit 2,
with no success output, remote error body, endpoint or credential. HTTP failures
also include the status number. A failed call does not fall back to the memory
baseline and pretend inference succeeded. The caller may keep the original
Gewyvern diagnosis and show an explicit unavailable-advice state.

## Migration Boundary

The Rust `AnalysisProvider` interface is independent of `LearningBackend`.
New inference adapters do not have to implement fake training or memory methods.
The explicit CLI switch, one-shot URL/target batches, single-source and target-index
resident modes are implemented. Use
`protocol-capabilities --provider provider.json` or the provider daemon's own
capability route to discover the read-only provider contract. Calls without the
switch still describe/select the legacy backend. Watch/federation,
the default process hook and Leserpent selection do not yet activate this provider.

Next gates are UI/federation selection, reviewed provider persistence and retained
real-provider quality/failure/cost evaluation. Old memory and training APIs are
compatibility data, not required model research. Do not delete or reinterpret
existing memory stores during this transition.

## One-Shot URL And Target Batch

```bash
etragon analyze-url http://127.0.0.1:9910/v1/latest/analysis.json \
  --provider provider.json
etragon analyze-targets-url http://127.0.0.1:9910/v1/latest/targets \
  --provider provider.json --filter scan: --max-targets 24 --max-inferences 4
```

Both commands use the same verified source transport, optional `--source-ca`,
separate `ETRAGON_SOURCE_ADMIN_TOKEN`, evidence allowlist and provider configuration
as resident mode. Neither opens a listener or reads/writes a memory store.
`analyze-url` returns the ordinary `augmentations` envelope; failures exit 2 with
no success output. The library entry for batches is `provider::batch::analyze_target_batch`.

The batch command reads one index and processes its selected names in sorted order.
Discovery validates the entire index before fetching any target or calling a model;
the same name, origin, filter and size restrictions as target-index resident mode
apply. Each target is a separate observation, not an atomic capture across targets.
The index is not refreshed during the batch. Removed targets can fail on fetch;
new targets are discovered on the next invocation. Index-supplied URLs are ignored.

`--max-inferences` defaults to **1**, allows 1-8, and cannot exceed `--max-targets`
(default 24, allowed 1-64). It counts attempted model calls, including setup and
transport failures, not tokens or billable usage. Source/evidence failures do not
consume inference budget. Once exhausted, remaining targets are marked `skipped`
without fetching them. Calls are serial, without retries or cross-target caching.
Every invocation starts cold; repeated invocations with a small budget start from
the same sorted names, not a durable cursor. Use `--filter` for a selected subset
or resident mode for fair continuous coverage.

The output is a versioned `etragon-provider-batch-v1` report, not merged advice:

- `status`: `ready`, `partial` or `unavailable`; `complete` is true only when every
  selected target is ready. An empty valid selection is complete with zero calls.
- `target_count`, `ready_count`, `failed_count`, `skipped_count`, `analysis_runs`
  and `max_inferences` describe coverage and the attempted-call budget.
- `targets[]` contains `path_segment`, `status` (`ready`, `error`, `skipped`),
  `output` or null, and `error` or null. Errors expose only `stage`, stable `code`
  and an optional HTTP status, never upstream bodies, endpoints or credentials.
- Each augmentation carries host-stamped target provenance with
  `source_scope: one-shot-target-index` and `target_incarnation: null`; it must
  not be mistaken for a resident incarnation or an execution grant.

A complete batch exits **0**. A batch with any error or skipped target exits **2**
but still writes its full partial report to stdout, preserving successful results;
stderr stays empty for these per-target outcomes. Callers must consume stdout even
on exit 2 and inspect `complete` and each target's status. Invalid configuration,
source-index failure or invalid index exits 2 with a stable stderr error and no
batch report. There is no fallback to legacy learning and no hidden retry.

Retained advice is capped at 512 KiB across the batch and the full report at 1 MiB.
If a successful inference would exceed the advice cap, that target reports an
output size error and later targets are skipped without further network calls;
earlier results remain available. That rejected inference still counts against
the budget. Worst-case network wait is bounded by `(1 + max_targets)` five-second
source deadlines plus `max_inferences * provider timeout`; this is a per-batch
upper bound, not a latency target. Only `--provider` and `--source-ca` apply to
single-URL mode; batch mode additionally accepts the three batch flags above.
Resident flags, `--state` and `--daemon-state` are rejected before any network call.

## Single-Source Resident Mode

```bash
etragon serve-url http://127.0.0.1:9910/v1/latest/analysis.json \
  --provider provider.json --bind 127.0.0.1:4321 --interval-ms 1000
etragon protocol-capabilities --provider provider.json
```

The source URL is an explicit read destination. HTTP sources must be loopback;
remote sources require HTTPS and `--source-ca <PEM-bundle>`. The source fetch has
a five-second network deadline and 128 KiB body cap. Its credential comes from
`ETRAGON_SOURCE_ADMIN_TOKEN` and is sent as `X-Gewyvern-Admin-Token`, never to the
model. The model uses only the provider's separately configured key. Redirects,
URL credentials and query strings remain forbidden on both connections.

`--bind` accepts a literal IP and port, including bracketed IPv6. Non-loopback
binding requires `ETRAGON_ADMIN_TOKEN`; remote requests need the matching
`X-Etragon-Admin-Token` header. The resident HTTP listener itself has no TLS:
keep it local behind a trusted TLS proxy or tunnel rather than exposing its
bearer token on an untrusted network. Loopback clients retain the legacy local
trust policy. Request reads have a one-second total deadline and 64 KiB cap.

Available GET routes: `/health`, `/v1/protocol-capabilities.json`,
`/v1/latest/status`, `/v1/latest/meta`, `/v1/latest/output.json`. Health indicates
listener liveness, not model readiness. Output is wrapped in an `output` field
with provider identity, cycle/call/cache counters, timestamps and availability.
During checking/inference or after failure it returns HTTP 503 with `output: null`;
the last successful inference timestamp is historical, not a usable stale result.
Successful advice is HTTP 200 and remains operator-review-only. Mutable routes
are refused, and training/memory/target routes are not advertised.

One successful result is cached per process. The key is the canonical allowlisted
evidence, excluding timestamps, raw packets and previous advice. Cache lookup
happens **before inference**, so identical evidence causes no new model request.
`analysis_runs` counts real attempts, including failed calls. `cache_hits` counts
skipped calls and does not advance `last_success_unix_ms`. `source_checked_unix_ms`
records the last valid source fetch on this host, not the original packet time;
advice is about that observed snapshot, not a promise that the source stayed unchanged
while inference was running.

Every failure clears the cached/current result. Subsequent polling retries with
exponential delay starting at the larger of one second or the configured interval,
capped at the larger of 60 seconds or that interval. There are no inner-call
retries. `retry_delay_ms` reports the scheduled delay, not a live countdown.
A successful inference resets failure backoff. Poll interval is 100-3600000 ms.

Configuration is loaded once; changes to provider/model/credentials/trust material
require a restart. A changed file is not a hot-reload signal. Model aliases can
change on the remote server; the cache does not attest model artifact identity.
Restart to force reanalysis after such a change. Every restart starts cold, with
no persisted input, advice or inherited cache. `--state`, `--daemon-state` and
`--filter` are rejected in single-source provider mode; legacy files are never read or rewritten.
This is intentionally not yet a durable multi-target service or GUI integration.

## Target-Index Resident Mode

```bash
etragon serve-targets-url http://127.0.0.1:9910/v1/latest/targets \
  --provider provider.json --filter scan: --max-targets 24 \
  --max-inferences-per-cycle 1 --bind 127.0.0.1:4321 --interval-ms 1000
```

This manages multiple targets **within one configured Gewyvern index**, not
multiple servers or a federation. Source TLS, credentials, read-only auth and
cold restart rules match single-source mode. `--state` and `--daemon-state` remain
unsupported. Configuration changes require restart.

The index must contain a `target_refs` array with unique `path_segment` strings.
An optional `target_count` must match its unfiltered length. Invalid/duplicate
names, duplicate typed JSON fields and over-limit indexes reject the whole index;
there is no silent truncation. Empty indexes are valid. Discovery accepts at most
256 references and 128 KiB; names are at most 256 UTF-8 bytes. The optional prefix
filter is applied only after all names are validated. Selected targets default to
24, configurable from 1 to 64. Slashes, backslashes, controls and whole `.`/`..`
segments are refused; other reserved characters are encoded exactly once.

The service ignores advertised `url_path` and any other endpoint fields. It
constructs target analysis paths on the configured source origin, never follows
index-supplied URLs, and never forwards source credentials to the model endpoint.

Each target has its own cache, retry deadline, status and local incarnation.
Identical evidence in two targets still requires two independent first inferences.
Advice includes Etragon-stamped target name/incarnation provenance. Disappearance
removes the target/cache immediately after index refresh; reappearance gets a new
cold incarnation. Incarnations are local to one resident lifetime, not global IDs:
consumers must keep the originating resident/session binding.

Scheduling is serial, bounded and round-robin. Every round reads one index,
visits at most eight due targets, and makes at most the configured number of new
inference attempts (default one, allowed 1-8 and no more than `--max-targets`).
Cache hits cost no model call. The cursor advances after the last inference, so
later budget-deferred targets lead the next round even if earlier evidence changes
continuously. A target failure backs off independently; it does not clear another
target's advice. Index failure clears all caches/current advice and backs off the
whole index; recovery reanalyzes cold. There is no parallel inference claim: one
slow call can delay later visits until its deadline. Worst-case network wait per
round is bounded by nine source deadlines plus the configured inference count
times provider timeout, followed by the poll interval.

`/v1/latest/targets`, status and meta return per-target summaries and canonical
`url_path`/`meta_url_path` values. Follow these exact paths for individual
`output.json` or `meta.json`; alternate encodings are not aliases. A ready target
returns HTTP 200 even when others fail; queued, failed, unverified or stale target
output returns HTTP 503 with `output: null`. Removed targets return 404.
Advice expires from visible output if its source has not been checked within the
larger of 60 seconds or two poll intervals, even if that target has not yet been
visited again. Cache reuse requires another successful source check.

Aggregate `/v1/latest/output.json` returns `targets`, not a merged augmentation
list. It returns 503 for partial/unavailable state while preserving explicitly
ready per-target entries. A valid empty index is ready with zero targets, not a
model success. Aggregate output is capped at 1 MiB; larger responses return 413
and direct the caller to individual target routes. Health only proves liveness.

The target daemon's capability route reports its mode and actual configured
limits. The standalone `protocol-capabilities --provider` command describes the
single-source mode and lists both available modes. Top-level `analysis_runs` and
`cache_hits` count lifetime attempts/skips, including removed targets, but reset
on restart. `last_cycle_target_visits` and `last_cycle_inferences` expose the
current round's budget use. Attempts include failed transport/setup, so they are
not a billing or token-usage report. Persistence, GUI selection and cross-server
federation remain separate gates; all advice still requires operator review.
