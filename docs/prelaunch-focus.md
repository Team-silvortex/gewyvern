# Prelaunch Focus

This note keeps the prelaunch scope intentionally narrow.

The question is not "what else could `gewyvern` support".

The question is:

- what most improves trust before the first stable release line
- what should stay frozen until after that release

## Do Next

### 1. Deepen High-Frequency Protocol Stability

Prioritize repeated real-world validation and targeted fixes for:

- `HTTP / HTTPS / TLS`
- `DNS`
- `SSH`
- `SOCKS5 / proxy`
- `MySQL / PostgreSQL`
- `QUIC / HTTP/3`

The goal is not more protocol breadth.

The goal is:

- more stable diagnosis on common operator paths
- more consistent `primary_failure_*`
- more coherent `operator_guidance_*`

### 2. Strengthen Mixed-Flow Conservatism

Keep validating and tuning these scenario families:

- `DNS + TLS + HTTP`
- `proxy auth + upstream request`
- `QUIC + HTTP/3`

Success here means:

- no silent over-collapse
- `ambiguous` remains acceptable when evidence is mixed
- stronger direct-signal conclusions appear only when evidence is actually stronger

### 3. Tune Built-In Operator Guidance

Only make small adjustments that improve standalone usefulness:

- `observe_more`
- `manual_review`
- `targeted_ready`

This is about making standalone `gewyvern` feel reliable as an agent/service,
not about adding a second diagnosis system.

### 4. Accept Only Small IR Improvements

Prelaunch IR work should be limited to:

- reducing repeated `main.gewy` or package boilerplate
- improving lowering or diagnostics stability
- removing obvious duplication in stable protocol/package paths

If an IR change would force a broad rethink of package shapes, diagnostics, or
registry entries, it should wait until after the prelaunch line.

## Do Not Expand Right Now

The following should stay out of the prelaunch scope:

- adding whole new protocol families for coverage alone
- introducing new major IR layers
- renaming the core diagnosis spine
- widening the DSL in ways that would reactivate registry churn

## Working Rule

Before taking a protocol or IR change, ask:

1. does this improve trust in a common operator workflow?
2. does this reduce ambiguity or drift in an already-supported high-frequency path?
3. can this be done without reactivating broad surface churn?

If the answer is "no" to those questions, it probably belongs after the
prelaunch line rather than before it.
