# How-To: Add Or Debug A Protocol Package

Use this guide when the question is:

- how do I add one more protocol package without destabilizing the shelf?
- how do I debug a drifting `main.gewy` package?
- which checks should I run before I trust a new package?

This page assumes you already understand the basics of `gewylang`.
If not, start with
[docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md).

## What A Protocol Package Is

In the current repository shape, a protocol package usually means:

- one directory under [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
- one [gewy.pkg](/Users/Shared/chroot/dev/gewyvern/protocols/http/request/gewy.pkg)-style manifest
- one `main.gewy` entry
- optional included helper modules

The package is not just source code. It is also:

- a registry-discoverable runtime entry
- a compiler-facing package root
- part of the built-in validation shelf

That is why “it compiles on its own” is not enough.

## Step 1: Start From A Nearby Existing Package

Before authoring a new package, find the closest working sibling:

- same protocol family
- same request/auth/session shape
- same denial or timeout style if possible

Examples:

- HTTP-like request paths:
  [protocols/http/request](/Users/Shared/chroot/dev/gewyvern/protocols/http/request)
- PostgreSQL query/auth paths:
  [protocols/postgres/query](/Users/Shared/chroot/dev/gewyvern/protocols/postgres/query)
- SOCKS5 auth/connect paths:
  [protocols/socks5/session](/Users/Shared/chroot/dev/gewyvern/protocols/socks5/session)

This keeps drift low because you are extending an existing shelf shape instead
of inventing a new one from scratch.

## Step 2: Create The Smallest Useful Package Shape

Prefer the current stable package form:

1. `gewy.pkg`
2. `main.gewy`
3. optional included helper files only when reuse actually helps

Keep the first version narrow:

- one entry
- one clear operation family
- one readable diagnosis path

Avoid trying to encode every protocol branch in one first package.

## Step 3: Make `main.gewy` Read Like A Focused Binding

The best built-in packages usually read as:

- a template id
- one window
- one reason profile
- a small group of fragments
- one operation/program model
- a small number of rules or params

That means:

- prefer one clear function-unit reuse boundary
- prefer one clear include boundary
- avoid “god packages” with too many unrelated helper files

If the package already feels hard to read in `main.gewy`, it will usually be
hard to validate and harder to debug later.

## Step 4: Check The Frontend Shape Before You Blame The Runtime

Run:

```bash
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/protocols/<family>/<entry>/main.gewy --focus graph
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/protocols/<family>/<entry>/main.gewy --json
```

Look for:

- `include_sources`
- `function_nodes`
- `use_edges`
- parse/validation/diagnostics failures

This is the fastest way to answer:

- did the package root resolve?
- did `include(...)` expand where expected?
- did the reused function units actually bind?
- is the failure in package structure rather than runtime semantics?

## Step 5: Validate The Package As A Runtime Entry

Once the frontend shape looks sane, run the package through the runtime shell:

```bash
cargo run -- --protocol <family> --entry <entry> --json --summary-only
```

Read:

- `primary_module_kind`
- `primary_failure_mode`
- `primary_failure_detail`
- `primary_failure_confidence`
- `operator_guidance_action`

The question here is not “is this perfect yet?”.

The question is:

- does this package land in the right diagnosis family?
- does it stay conservative when evidence is thin?

## Step 6: Register-Level Validation Matters

Do not stop at “one command worked”.

Run:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/registry_validation.sh
```

This tells you whether your package:

- is discovered by the shelf
- compiles as part of the registry
- passes validation
- passes diagnostics
- emits the JSON shape the validator expects

If your new package passes a direct `cargo run -- --protocol ...` path but
fails here, treat that as a real issue. It means the registry shelf is not as
healthy as it looks from one narrow command.

## Step 7: Use High-Frequency Validation When The Package Is Important

If the package lives in a high-frequency family, also run:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/high_frequency_validation.sh
```

This is especially important for:

- `HTTP / HTTPS / TLS`
- `DNS`
- `SSH`
- `SOCKS5`
- `MySQL / PostgreSQL`
- `QUIC / HTTP/3`

Those families carry more release weight than long-tail protocol coverage.

## Step 8: How To Debug A Drifting Package

Use this order:

1. `gewyc frontend --focus graph`
2. `gewyc explain --json`
3. direct runtime command
4. `registry_validation.sh`

Then classify the failure.

### Parse Failure

Usually means:

- broken package structure
- broken `include(...)`
- broken function body or call shape

Look first at:

- package root path
- `gewy.pkg`
- `main.gewy`
- included helper files

### Validation Failure

Usually means:

- unsupported payload offsets
- fragment coverage mismatch
- binding shape outside current compiler/runtime support

Look first at:

- fragment selection
- params and evidence overrides
- rule shape

### Diagnostics Failure

Usually means:

- rules require facts the fragment set does not support
- the package compiles, but the diagnosis layer cannot support it safely

Look first at:

- `reason_rule(...)`
- `program_rule(...)`
- current fragment inventory

### Runtime Drift

Usually means:

- the package compiles and validates
- but lands in the wrong `primary_module_kind` or failure family

Look first at:

- operation/program model choice
- stage naming
- rule narrative/predicate alignment
- expected conservatism around missing transitions

## Step 9: Keep The Package Shelf Coherent

Before calling a package “done”, ask:

1. does it match the naming and structure of nearby built-ins?
2. does it compose through the current stable `gewylang` subset?
3. does it pass direct runtime invocation?
4. does it pass registry validation?
5. does it avoid overconfident collapse when evidence is incomplete?

If not, keep trimming.

The best built-in packages are usually the ones that feel smallest and most
predictable.

## Step 10: Document The New Package

If the package adds a meaningful new family or entry, update at least one of:

- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/process-profiles.md](/Users/Shared/chroot/dev/gewyvern/docs/process-profiles.md)
- [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)

The goal is simple:

- if someone else finds the package later, they should know why it exists
- and how it is supposed to behave

## Where To Go Next

- For the package authoring tutorial:
  [docs/book/tutorial-gewylang-package.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-gewylang-package.md)
- For the current runtime validation line:
  [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)
- For module and source layering:
  [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)
