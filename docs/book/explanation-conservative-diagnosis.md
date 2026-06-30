# Explanation: Why gewyvern Prefers Conservative Diagnosis

`gewyvern` is intentionally biased toward conservative diagnosis.

That choice can look restrained at first, especially if you come from tools
that always try to produce one strong answer. This page explains why the
project does not do that by default.

## Book Path

This chapter lives in Part III: The Runtime Spine.

Read it after:

- [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)
- [docs/book/reference-diagnosis-spine.md](docs/book/reference-diagnosis-spine.md)

Then continue with:

- [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)
- [docs/book/explanation-stack-topology.md](docs/book/explanation-stack-topology.md)

## The Short Version

`gewyvern` is not trying to be a clever guesser.

It is trying to be a useful operator tool that:

- avoids overclaiming from partial evidence
- keeps uncertainty visible
- produces next-step guidance that is safe to act on

In practice, that means it would rather say:

- "the strongest current lead is HTTP request/response, but TLS still matters"

than pretend:

- "this is definitely an HTTP problem"

## Why This Matters

The runtime often works from incomplete evidence:

- advisory socket ingest
- partial protocol paths
- missing transitions
- mixed flows across DNS, connect, TLS, proxy, and application layers
- process attribution that may be suggestive rather than sovereign

If the system aggressively collapsed all of that into one definitive story, it
would feel smarter in the short term and less trustworthy in the long term.

The cost of a wrong confident diagnosis is usually higher than the cost of a
careful one.

## The Core Design Choice

The project therefore treats diagnosis as a layered summary rather than a
winner-takes-all verdict.

That is why the core runtime spine is structured around:

- `primary_module_kind`
- `primary_failure_stage`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `primary_failure_basis`
- `ambiguous`
- `competing_hypotheses`
- `operator_guidance_*`

The design intent is:

- one best current lead
- explicit evidence strength
- explicit ambiguity when needed
- explicit next-step guidance

Not:

- one unqualified answer no matter what the input looked like

## Why Missing Transitions Are Treated Carefully

Many practical results are built from missing transitions:

- request observed, response absent
- connect attempt observed, establishment absent
- auth prompt observed, follow-up auth absent

These are useful, but they are not the same thing as a direct protocol
statement.

That is why `gewyvern` keeps `primary_failure_basis` visible and distinguishes
between:

- `direct_protocol_signal`
- `missing_transition`
- `phase_level_inference`

This lets the system stay honest about the difference between:

- "the server denied this"
- and
- "the next expected thing never happened"

Those are both useful findings, but they should not feel equally certain.

## Why Ambiguity Is A Feature

`ambiguous=true` is not a failure of the runtime.

It is often the correct result.

That is especially true in mixed-flow scenarios such as:

- `DNS + TLS + HTTP`
- `SOCKS5 + upstream request`
- `QUIC + HTTP/3`
- partial setup plus partial application exchange

In those cases, the runtime is often doing the right thing when it says:

- here is the strongest current primary path
- but these competing hypotheses are still alive

This is why `competing_hypotheses` exists. It is not decorative. It is the
system's way of making residual uncertainty visible instead of hiding it.

## Why Operator Guidance Sits Beside Diagnosis

Another deliberate choice is that built-in operator guidance does not replace
the diagnosis spine.

Instead, it sits beside it:

- diagnosis answers "what seems to be happening?"
- guidance answers "what is the safest next move?"

That separation matters because evidence strength and action strength should
not always track one-to-one.

For example:

- a PID-shaped result may be informative
- but still not strong enough for forceful PID-scoped automation

That is why guidance can legitimately say things like:

- `avoid_pid_strong_actions`
- `keep_multiple_hypotheses`
- `collect_more_runtime_evidence`

even when the diagnosis spine already has a reasonable primary story.

## Why This Fits v0.15.0

The `v0.15.0` line is supposed to be usable on purpose, not theatrically
confident.

A conservative diagnosis model helps with that because it:

- scales better to real operator use
- survives partial evidence more honestly
- makes later automation safer
- gives sidecars and downstream systems a stable narrow core to respect

It also matches the current evidence posture of the repository: the project
already has enough runtime, packaged, and Docker validation to be useful, but
it is still deliberately evolving within the `0.15.x` line.

## What This Does Not Mean

Conservative does not mean weak.

`gewyvern` still absolutely tries to produce:

- one primary module family
- one primary failure path
- one operator guidance action

It just refuses to pretend those conclusions are stronger than the evidence
allows.

That is the key idea:

- decisive when the runtime has earned it
- explicit about uncertainty when it has not

## Practical Reading Rule

If a result feels “less certain than you hoped,” read it in this order:

1. `primary_module_kind`
2. `primary_failure_mode`
3. `primary_failure_detail`
4. `primary_failure_confidence`
5. `primary_failure_basis`
6. `ambiguous`
7. `competing_hypotheses`
8. `operator_guidance_action`

That order reveals not only the runtime's answer, but how much trust it is
asking you to place in that answer.

## Continue With

If you want to see how this conservative runtime posture constrains packaged
protocol work, go to:

- [docs/book/explanation-protocol-package-spine.md](docs/book/explanation-protocol-package-spine.md)

If you want to see how it constrains the broader multi-project stack, go to:

- [docs/book/explanation-stack-topology.md](docs/book/explanation-stack-topology.md)
