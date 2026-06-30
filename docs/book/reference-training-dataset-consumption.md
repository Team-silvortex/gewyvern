# Reference: Training Dataset Consumption

Use this page when you are wiring a trainer, reranker, or sibling engine to the
machine-facing training export surface.

It is intentionally narrow:

- how to enumerate samples
- how to fetch one concrete sample
- which IDs are stable
- how to use the built-in split hints conservatively

For the broader contract candidate, see
[docs/machine-contract.md](docs/machine-contract.md).

For the external-engine boundary, see
[docs/external-engine-contract.md](docs/external-engine-contract.md).

## Preferred Fetch Order

For batch collection:

1. fetch `/v1/latest/training-dataset.json`
2. read `samples[]`
3. fetch each `sample_path`
4. cache or train against the returned `training-example.json` payload

For one explicit target:

1. resolve the target path segment from `/v1/latest/targets`
2. fetch `/v1/latest/targets/<path-segment>/training-dataset.json`
3. fetch `/v1/latest/targets/<path-segment>/training-example.json`

Do not invent target paths from display names when `/v1/latest/targets` is
available. Use the emitted `path_segment`.

## Stable Identity

Each training sample now has one stable ID:

- `training-example.json -> sample_id`
- `training-dataset.json -> samples[].sample_id`

Those two values are expected to match exactly for the same target.

Current shape:

- prefix: `gewy:`
- body: deterministic FNV-1a 64-bit hash of the target name

Use `sample_id` as the trainer-side:

- cache key
- dedup key
- replay lookup key
- join key between manifest rows and fetched sample payloads

Prefer `sample_id` over:

- display `name`
- raw `sample_path`
- ad hoc target indexes

because the sample ID is the narrowest stable identity surface.

## Manifest Fields

The dataset manifest declares:

- `sample_format`
- `sample_schema_version`
- `split_policies`
- `supervision_heads`
- `samples[]`

Use `sample_schema_version` to gate parser behavior if you are building a
long-lived consumer.

`supervision_heads` tells you which built-in prediction heads are intentionally
published today:

- `diagnosis`
- `guidance`
- `automation`
- `ranking`

Treat that section as the source of truth for the currently declared target
heads instead of hard-coding assumptions in multiple places.

## Sample Row Fields

Each manifest row currently includes:

- `name`
- `sample_id`
- `path_segment`
- `group_key`
- `split_hints`
- `sample_path`
- `dataset_path`

Recommended use:

- `sample_id`: stable join key
- `group_key`: coarse family/group hint for stratified sampling
- `split_hints`: deterministic built-in split candidates
- `sample_path`: fetch path for the concrete sample
- `dataset_path`: manifest self-path for the same target

## Split Hints

The manifest publishes built-in deterministic split hints rather than a single
mandatory split assignment.

Current policies:

- `name_bucket_mod_10`
- `protocol_bucket_mod_10`

Current labels:

- `train`
- `validation`
- `test`

Recommended default behavior:

1. start with `split_policies.default`
2. read `samples[].split_hints[default_policy]`
3. use that bucket unless your trainer owns a stricter policy

Recommended conservative upgrade path:

1. use `protocol_bucket_mod_10` when you want related protocol families to
   stay grouped more consistently
2. keep the emitted value alongside your trainer-side split so runs remain
   explainable and replayable

Do not rewrite the built-in hint in place. If your trainer overrides the split,
store your override separately.

## Concrete Sample Fields

Each `training-example.json` currently exposes:

- `kind`
- `schema_version`
- `name`
- `sample_id`
- `template_id`
- `input`
- `supervision`
- `provenance`

Recommended consumption pattern:

1. treat `input` as model-facing features
2. treat `supervision.targets.*` as the built-in prediction heads
3. treat `provenance` as ingest/replay posture, not primary label material

The operator-oriented guidance strings are still useful, but if you are
training structured heads, prefer:

- `supervision.targets.diagnosis`
- `supervision.targets.guidance`
- `supervision.targets.automation`
- `supervision.targets.ranking`

over re-deriving those heads from free-form summaries.

## Caching And Replay

Minimal durable cache layout:

- key by `sample_id`
- keep the raw `training-example.json`
- keep the manifest row that pointed to it

That gives you:

- deterministic re-fetch
- stable joins across manifest refreshes
- a narrow replay/debug spine

If the same `sample_id` reappears with different payload contents, treat that as
a contract change worth surfacing in validation or CI rather than silently
merging.

## Minimal Example

Batch flow:

```text
GET /v1/latest/training-dataset.json
  -> read samples[0].sample_id
  -> read samples[0].sample_path
GET /v1/latest/targets/<path-segment>/training-example.json
  -> confirm sample_id matches
  -> consume input / supervision / provenance
```

Single-target flow:

```text
GET /v1/latest/targets
  -> choose target_refs[i].path_segment
GET /v1/latest/targets/<path-segment>/training-dataset.json
GET /v1/latest/targets/<path-segment>/training-example.json
  -> confirm sample_id matches
```

## Reference Script

There is also a minimal executable consumer-roundtrip reference:

- [scripts/demos/training_dataset_roundtrip_demo.sh](scripts/demos/training_dataset_roundtrip_demo.sh)

It will:

1. fetch `training-dataset.json`
2. iterate the declared `samples[]`
3. fetch each `training-example.json`
4. verify `sample_id` consistency
5. write a small local summary plus the fetched sample payloads

This is intentionally narrower than the full runtime validation scripts.
Use it when you specifically want to validate the machine-facing training
surface as a consumer would see it.

Example:

```bash
cargo run --quiet --bin gewyvern_validate -- training-roundtrip --api-addr 127.0.0.1:9910 --out-dir /tmp/gewyvern-training-demo
```

Single-target example:

```bash
cargo run --quiet --bin gewyvern_validate -- training-roundtrip --api-addr 127.0.0.1:9910 --out-dir /tmp/gewyvern-training-demo --target-path-segment dsl_demo --limit 1
```

## Consumption Rules

- Prefer `sample_id` as the identity boundary.
- Prefer `sample_path` over handcrafted target URLs.
- Prefer `supervision.targets.*` over ad hoc label inference.
- Prefer the built-in split hints as defaults, not mandates.
- Keep trainer-side overrides additive so replay remains explainable.
