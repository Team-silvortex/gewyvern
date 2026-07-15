# Leselang Language Contract

This reference is the authoritative, model-oriented contract for the currently
implemented Leselang slice. The broader destination is defined by the
[Leserpent 2.0 architecture](leserpent-2-architecture.md); unimplemented roadmap
syntax is not part of this contract.

Status: **Gate 2, evolving contract 0.2.0**. The current vertical slice parses,
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
a source-size limit, fuel limit, 24-hour maximum deadline budget, and
10,000-item output limit.
Restored continuations advance their token sequence so a restart cannot reuse a
live token.

`Vm::new` provides an ephemeral journal for tests and embedded one-shot use.
`Vm::open_journal` opens the SQLite effect journal for service operation. It
atomically persists pending continuations before exposing an effect, restores
them after restart, and commits the first terminal step as authoritative.
Duplicate or competing completion after a process restart replays that durable
step rather than accepting a later result. Sequence allocation is transactional
across concurrent VM connections.

The journal is schema-versioned, uses full synchronous commits and a five-second
lock timeout, rejects symbolic-link final paths, and creates Unix files with
`0600` permissions. It is bounded to 10,000 records, 8 MiB per terminal step,
and 64 MiB of total logical payload.

This is a durable continuation guarantee, not yet an exactly-once guarantee for
arbitrary external mutation. The current `runtime.list` effect is read-only.
Future mutating effects must journal dispatch and acknowledgement in the same
protocol. Typed cancellation, wall-clock timeout enforcement, retry policy,
retention/compaction, and deterministic structured merge remain Gate 2 work.

## Diagnostics

Diagnostics use stable subsystem prefixes:

| Prefix | Owner | Examples |
| --- | --- | --- |
| `LSE` | lexer and parser | malformed input, source limit |
| `LSH` | HIR and authorization | unknown effect, duplicate argument, missing capability |
| `LSV` | VM, continuation, and journal | invalid image, revision conflict, persistence failure |

Consumers must branch on diagnostic codes rather than English messages. Spans
use byte offsets into the original UTF-8 source.

## Integration Sequence

For deterministic model or CLI integration:

1. Parse source and report all syntax diagnostics.
2. Lower the syntax tree into HIR and report semantic diagnostics.
3. Authorize required capabilities before starting the VM.
4. Open the durable journal, then call `Vm::start`; pending state is committed before return.
5. Execute the typed effect through `leserpent-domain`.
6. Call `Vm::resume` with the token, expected revision, and typed result.
7. Enumerate pending continuations after restart and safely redrive read-only effects.
8. Treat repeated completion as replay, not another external operation.

The implementation lives in `crates/leselang-syntax`, `crates/leselang-hir`,
and `crates/leselang-vm`. Delivery progress is tracked by the
[project status tensor](project-status-system.md), not inferred from future
examples in architecture documents.
