# Leselang Language Contract

This reference is the authoritative, model-oriented contract for the currently
implemented Leselang slice. The broader destination is defined by the
[Leserpent 2.0 architecture](leserpent-2-architecture.md); unimplemented roadmap
syntax is not part of this contract.

Status: **Gate 2, evolving contract 0.1.0**. The current vertical slice parses,
lowers, authorizes, suspends, serializes, restores, and resumes one read-only
effect: `runtime.list`.

## Canonical Program

```leselang
fn main() = runtime.list(
  environment: "production",
  cluster: none,
  role: "edge"
)
```

`runtime.list` returns a runtime list and requires the `runtime.read`
capability. Each optional filter accepts a string or `none`; empty strings are
normalized to `none` during HIR lowering.

## Grammar

```ebnf
program       = function, EOF ;
function      = "fn", identifier, "(", ")", "=", effect-call ;
effect-call   = identifier, ".", identifier, "(", [ arguments ], ")" ;
arguments     = argument, { ",", argument }, [ "," ] ;
argument      = identifier, ":", value ;
value         = string | "none" ;
identifier    = ( letter | "_" ), { letter | digit | "_" } ;
string        = '"', { character | escape }, '"' ;
escape        = "\\", ( '"' | "\\" | "n" | "r" | "t" ) ;
```

Whitespace and `//` line comments are retained as lossless tokens, including
their byte spans. Reassembling token text must reproduce the original source.
Source is UTF-8 and limited to 256 KiB.

The implemented surface deliberately excludes general expressions, local
bindings, mutation, loops, concurrency syntax, raw HTTP, shell execution, and
host-language reflection. Synchronous source semantics do not expose
`async`/`await`.

## HIR And Authorization

The syntax tree lowers into a typed `RuntimeList` effect. Lowering rejects
unknown effects, duplicate named arguments, unknown arguments, and values with
the wrong shape.

Authorization is explicit and occurs before VM execution. A caller without
`runtime.read` receives a capability diagnostic; the VM does not emit an
effect request for unauthorized code.

## Execution Protocol

The stackless VM advances through four protocol states:

- `Done`: evaluation completed with bounded output
- `Effect`: the host must execute a typed request and resume the continuation
- `Yield`: cooperative suspension reserved by the protocol
- `Fault`: evaluation stopped with a stable VM diagnostic

For the current slice, `start` emits one `RuntimeList` effect. The effect carries
a continuation token and expected revision. The host executes the shared
Leserpent domain query, then calls `resume` with the typed result.

Continuation images are versioned and capped at 64 KiB. Execution also enforces
a source-size limit, fuel limit, deadline budget field, and output-item limit.
Restored continuations advance their token sequence so a restart cannot reuse a
live token.

Duplicate completion in the same VM instance is idempotent: the first completed
step is replayed without executing the effect again. This is not yet a durable
exactly-once guarantee. A process crash loses the in-memory pending/completed
journal; durable effect journaling, cancellation, timeout enforcement, retry,
and deterministic structured merge remain Gate 2 work.

## Diagnostics

Diagnostics use stable subsystem prefixes:

| Prefix | Owner | Examples |
| --- | --- | --- |
| `LSE` | lexer and parser | malformed input, source limit |
| `LSH` | HIR and authorization | unknown effect, duplicate argument, missing capability |
| `LSV` | VM and continuation | invalid image, revision conflict, execution limit |

Consumers must branch on diagnostic codes rather than English messages. Spans
use byte offsets into the original UTF-8 source.

## Integration Sequence

For deterministic model or CLI integration:

1. Parse source and report all syntax diagnostics.
2. Lower the syntax tree into HIR and report semantic diagnostics.
3. Authorize required capabilities before starting the VM.
4. Call `Vm::start` and persist any returned continuation image.
5. Execute the typed effect through `leserpent-domain`.
6. Call `Vm::resume` with the token, expected revision, and typed result.
7. Treat repeated completion as replay, not another external operation.

The implementation lives in `crates/leselang-syntax`, `crates/leselang-hir`,
and `crates/leselang-vm`. Delivery progress is tracked by the
[project status tensor](project-status-system.md), not inferred from future
examples in architecture documents.
