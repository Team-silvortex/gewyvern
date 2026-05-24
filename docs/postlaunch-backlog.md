# Postlaunch Backlog

This note records the small set of issues that are already visible during the
`v0.10.0` field-validation phase, but are better treated as postlaunch work
than as prelaunch blockers.

It is intentionally narrow.

It is not a place to dump speculative future ideas.

Use it only for items that are:

- already visible in real validation work
- non-blocking for the conservative prelaunch line
- still important enough to track explicitly

## 1. Richer Negative-Path Semantics For Denied Demo Entries

Observed today:

- `socks5 auth-denied`
- `socks5 auth-connect-denied`
- `smtp rcpt-denied`
- `smtp data-denied`

currently stay in a conservative setup-shaped posture:

- `primary_failure_basis = "missing_transition"`
- DNS/setup-oriented stage selection
- `operator_guidance_action = "collect_more_runtime_evidence"`

Why it is not a prelaunch blocker:

- this does not create false strong conclusions
- packaged validation confirms these paths do not over-collapse
- the current line is intentionally prioritizing conservative trustworthiness

Why it still matters after launch:

- these entries should eventually express richer denial semantics when the
  synthetic path genuinely drives far enough to support them
- operator-facing negative-path diagnosis would become more informative

## 2. Stronger Failure-Oriented Packaged Operator Paths

Observed today:

- packaged operator-path validation is now strong on conservative mixed and
  setup paths
- it is still thinner on richly-denial-shaped packaged paths

Why it is not a prelaunch blocker:

- the most important current guarantee is that packaged installs behave
  coherently and do not invent overconfident conclusions

Why it still matters after launch:

- packaged field validation should eventually include stronger negative-path
  cases that end in explicit refusal, denial, or semantic rejection

## 3. Better Separation Between Conservative Advisory Paths And Stronger Final Diagnoses

Observed today:

- the runtime is currently more trustworthy as a conservative diagnosis engine
  than as an aggressively collapsing one
- this is good for launch trust, but it also means some demo paths stay more
  advisory than an operator may hope

Why it is not a prelaunch blocker:

- it is safer to ship a runtime that is cautious than one that overclaims

Why it still matters after launch:

- high-value protocol families can be revisited one by one to decide where a
  stronger final diagnosis is actually justified by evidence

## Working Rule

Do not pull an item from this page back into prelaunch work unless one of these
becomes true:

1. field validation shows drift or instability rather than conservatism
2. packaged installs begin over-collapsing into stronger claims
3. the item starts affecting the narrow machine-facing contract

Otherwise, keep the launch line narrow and treat these as deliberate
postlaunch improvements.
