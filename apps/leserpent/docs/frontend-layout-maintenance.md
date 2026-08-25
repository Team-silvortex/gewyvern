# Leserpent Frontend Layout Maintenance

## Purpose

This note exists to keep the control-plane dashboard maintainable as the UI
gets denser.

The dashboard is intentionally optimized for:

- single-page shell navigation
- fast first-screen usability
- low-scroll or no-scroll operation in tighter desktop windows
- stable deep-link restoration into tabs, panes, and selected runtimes

This document is the working contract for future layout changes.

## Source Of Truth

The layout system is split across a few clear files:

- `src/Leserpent/frontend/15-preferences-bootstrap.ts`
  - bootstraps preference and resize behavior
- `src/Leserpent/frontend/20-security-transport.ts`
  - resolves layout mode and restores URL-driven state
- `src/Leserpent/frontend/40-runtime-inspector.ts`
  - owns runtime child-window lifecycle and keyed window rendering
- `src/Leserpent/frontend/47-runtime-list-renderer.ts`
  - renders the runtime list and child-window entry action
- `src/Leserpent/frontend/app.ts`
  - owns shared state and DOM references
- `src/Leserpent/wwwroot/styles.css`
  - owns almost all density, spacing, and anti-overlap rules

When changing layout behavior, update the TypeScript source first, then rebuild
the static output.

## Operator Policy Boundary

The browser may render confirmations and collect operator input, but it must not
decide which runtimes a destructive fleet command targets. Runtime cleanup uses
the control-plane-owned `GET /v1/runtimes/cleanup-plan` response for:

- failed and unobserved target membership
- protected-slice classification
- affected runtime and session counts
- the clear-slice challenge
- a plan token that rejects stale confirmations

The matching delete request sends the plan token to the control plane. The
daemon owns target membership, while managed session IDs are included in the
token as explicit cascade impact. A changed target or session set returns `409
runtime_cleanup_plan_changed`; the dashboard reloads the fleet before the
operator confirms again. In particular, `idle_ready` runtimes are not
unobserved-cleanup targets even when they have no latest snapshot.

Keep presentation-only state such as open panes, local windows, focus, and theme
in TypeScript. Move target selection, authorization, revisions, risk levels, and
command preconditions behind typed control-plane contracts.

Runtime registration follows the same boundary. The browser debounces a
secret-free `POST /v1/runtimes/registration-plan` request and submits the returned
plan token with the real registration. The control plane owns these rules:

- a new name and endpoint produce `create`
- an existing name produces an idempotent `update` that preserves runtime ID
- an endpoint already owned by another name produces `reject`
- canonically equivalent HTTP endpoints cannot bypass uniqueness
- daemon-backed plans expose the planned runtime ID and expected revision
- plan-token v2 binds runtime, sidecar, action, authority, ID, and revision
- a daemon-backed registration must submit a current plan token
- an ID reserved for deletion produces `runtime_deletion_in_progress`
- a create/update transition after preview invalidates the stale plan token

Suggested names and immediate URL-shape hints may remain browser-local because
they are presentation aids. Pairing tokens and sidecar credentials must never be
sent to the preview endpoint.

The registration POST is a transport adapter over the shared control-plane
execution coordinator. Browser code must not reproduce discovery ordering,
authority receipt validation, compatibility writes, or recovery classification.
It must not synthesize command or idempotency IDs either: the control plane
derives both from the reviewed revision and canonical secret-free registration
intent.
If registration returns `runtime_registration_outcome_ambiguous`, adapters must
retain the entered credentials only in their existing transient UI scope and
request the plan again. An exact target receives a
`runtime_registration_recovery_pending` recovery plan bound to the original
revision; adapters must display and submit that plan rather than constructing a
new revision or repeating discovery themselves. A rejected recovery plan means
another pending intent owns the overlapping name or endpoint.

Runtime recovery is also server-orchestrated. The browser submits one typed
`POST /v1/runtimes/{runtimeId}/recovery` command with `all`, `status`,
`capabilities`, or `sidecar`. The response reports each executed step and its
outcome. The browser must not reconstruct `all` from multiple legacy endpoints
or decide whether a paired sidecar participates.

Suggested attention actions carry `commandKind` directly from the control plane.
Cooldown, priority, step selection, aggregate outcome, and recovery-history
recording therefore remain stable when the renderer is replaced. The older
fine-grained refresh endpoints remain compatibility surfaces, not the dashboard
workflow authority. Those compatibility surfaces and Fleet refresh now resolve
the same server-side command execution context: daemon membership, endpoints,
and revision select the target, while local credential slots are attached only
inside the effect boundary. The browser must never supply or reconstruct those
coordinates from previously rendered runtime data.

## Layout Modes

The dashboard no longer relies only on scattered media queries.

Instead, runtime layout is normalized into explicit modes on
`document.documentElement.dataset.layoutMode`.

Current modes:

- `default`
- `compact`
- `safe-compact`
- `emergency`

The mode is computed from viewport width and height, then reflected into CSS
selectors like:

- `:root[data-layout-mode="compact"]`
- `:root[data-layout-mode="safe-compact"]`
- `:root[data-layout-mode="emergency"]`

This gives us a predictable place to compress:

- shell spacing
- card padding
- toolbar density
- sidebar width
- form heights
- runtime panel action strips
- register page preview density

## Responsive Maintenance Rules

When we tighten or extend the UI, prefer these rules:

1. Prefer `data-layout-mode` rules over adding another full media-query fork.
2. Keep the first screen useful at common laptop sizes before optimizing for
   long-page scrolling.
3. Make action rows horizontally scrollable before allowing them to wrap into
   tall multi-line stacks.
4. Reduce density by shrinking gaps, padding, and min-heights before deleting
   useful information.
5. Keep blank, loading, and degraded states compact; these happen often and
   should not consume more height than real data.
6. Avoid repeating the same status labels in multiple stacked cards unless that
   duplication improves orientation.

In practice, the dashboard should remain comfortable around:

- `1366x768`
- `1180x820`
- `1024x720`
- `900x650`

The smaller the viewport, the more important it is that the shell remains
understandable without vertical trapping or overlapping controls.

## Deep-Link Contract

URL state must remain authoritative for restorable navigation.

That means query parameters such as:

- `tab`
- `runtimeId`
- `runtimePane`
- `runtimeMode`
- `runtimeSide`
- view-specific sub-tab parameters

must win over default in-memory state when we hydrate the page.

Important rule:

- defaults may fill missing URL state
- defaults must not overwrite URL intent

If a deep-link is broken, check hydration order before changing rendering code.

## Runtime Workspace Structure

The runtime workspace is intentionally shell-like.

The outer shell chooses the major area:

- `Select`
- `Register`
- `Detail`
- `Child Panel`

Each major area should be able to occupy the primary content region cleanly,
rather than fighting for space with sibling panes.

This is especially important because the dashboard is designed around
full-screen desktop use where overgrown stacked blocks quickly become unusable.

### Child-window workspace

`Child Panel` is a multi-window workspace rather than a single replaceable
iframe. Each open runtime owns its current view and embedded frame. The shared
source/view toolbar controls only the active window.

Layout rules:

- two or more windows use a two-column desktop grid when the viewport permits
- `920px` and below always use one column
- window bodies keep bounded heights instead of growing with embedded content
- title-bar actions must remain visible without overlapping status chips
- keyed updates must preserve sibling iframe DOM and browsing state
- off-screen windows should retain lazy-loading and rendering containment

State rules:

- URL `runtimeId` and `runtimeView` win for the active deep-linked window
- `leserpent.runtimeWindows` restores the wider browser-local window set
- deleted or filtered-out runtime IDs must not leave orphan window DOM

The detailed behavior contract is in
`docs/runtime-window-workspace.md`.

## Compact Blank States

When no runtime panel or embedded child content can be shown, blank states
should feel operational rather than decorative.

Good fallback states should:

- explain what is missing
- tell the operator what to do next
- avoid giant empty illustrations
- preserve room for the rest of the control surface

If a placeholder is visually louder than the surrounding runtime controls, it
is probably too large.

## Validation Workflow

Before landing layout-sensitive changes:

1. Run `npm run check:frontend`
2. Run `npm run package:frontend`; unchanged inputs return from the content-hash fast path
3. Run `npm run verify:frontend-package` to prove the checked manifest still matches every asset
4. Start the local server and verify real pages
5. Capture a few small-window screenshots when the change is risky

Release builds invoke the same package coordinator before .NET discovers static
web assets. MSBuild tracks the coordinator's Rust source and locked dependency
graph as explicit inputs, incrementally builds its native executable, and then
invokes that executable directly. The unchanged Release path therefore starts
neither Cargo nor Node while still hashing every package input and asset. A
stale TypeScript output or language-pack catalog is rebuilt from the locked
dependency graph instead of being silently copied into a release. The package
manifest is bounded, rejects symlinks, and records SHA-256 plus byte size for
every published frontend asset.

## Adaptive Shell Contract

The control shell uses width-first adaptation rather than treating a tall,
narrow viewport as a compact desktop:

- At `920px` and below, fleet filters move behind an explicit disclosure so
  operational content remains visible without deleting filter capability.
- From `601px` through `820px`, the repeated Workspace brand is removed and all
  five primary destinations share one equal-width navigation row.
- At `600px` and below, the shell enters the `mobile` layout mode, compacts the
  brand, keeps refresh and security controls visible, and moves the five primary
  destinations into a safe-area-aware fixed bottom bar.
- Compact and safe-compact desktop modes keep vertical document scrolling as a
  low-height fallback, so a short window cannot clip the workspace below the
  browser viewport.
- Mobile controls retain a minimum `44px` target; coarse pointers receive the
  same minimum regardless of viewport width.
- Runtime tables observe their own panel instead of the browser viewport. At
  `920px` of available panel width and below they retain the table DOM while rows
  become labeled cards; never restore horizontal scrolling as the primary narrow
  layout. Keep the ResizeObserver path working when tabs reveal or resize.
- Runtime rows expose one roving tab stop, Up/Down plus Home/End navigation,
  Enter/Space selection, contextual action labels, and at most one open action
  menu. Escape closes that menu and restores focus to its summary.
- Tab groups expose roving keyboard focus, `aria-selected`, linked tab panels,
  and Home/End plus direction-aware arrow navigation.

Keep overlays above the mobile navigation and preserve bottom content padding;
a usable panel that is hidden behind fixed chrome is still a layout failure.

Useful audit routes are usually of the form:

- `/?tab=runtimes`
- `/?tab=runtimes&runtimePane=detail&runtimeId=...`
- `/?tab=runtimes&runtimePane=panel&runtimeId=...`
- `/?tab=runtimes&runtimePane=panel&runtimeId=...&runtimeView=health`
- `/?tab=runtimes&runtimePane=register`

Suggested spot checks:

- light theme
- dark theme
- one observed runtime
- one degraded runtime
- register form with preview visible
- child panel with no embedded content available
- two or more child windows with different active views
- reload restoration and URL-over-local active-window precedence

## Editing Checklist

Use this checklist whenever the dashboard starts feeling crowded again:

- Did the change reduce first-screen usability?
- Did we accidentally reintroduce duplicated media-query logic?
- Does the URL still restore the same tab and pane after reload?
- Are action rows scrollable instead of colliding?
- Do dark-mode text and chip contrasts still pass a basic visual check?
- Is the degraded or blank state compact enough to stay out of the way?
- Does changing one runtime window leave sibling iframe nodes intact?
- Does the multi-window grid collapse to one column without horizontal overflow?

If the answer to any of these is no, adjust layout-mode rules before adding
more structure.
