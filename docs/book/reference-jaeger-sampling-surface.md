# Reference: Jaeger Sampling Surface

The Jaeger sampling surface tracks sampling strategy requests made by clients,
agents, or collectors.

Family hub: [Jaeger surface](docs/book/reference-jaeger-surface.md)

Canonical entries: `sampling`

## Debugging Focus

- Sampling strategy route selection.
- Whether clients receive current sampling posture before emitting spans.
- Distinguishing sampling suppression from ingest or query failures.

## Typical Question

Use this surface when tracing appears enabled but the observed span volume is
much lower than expected.

<!-- gewyvern:entry-aliases:start -->
## Current Entry Aliases

This generated block tracks the aliases that currently resolve into this custom surface.

- `jaeger-sampling`
- `jaeger_sampling`
- `sampling-strategies`
- `sampling-strategy`
- `sampling_strategies`
- `strategy`

<!-- gewyvern:entry-aliases:end -->
