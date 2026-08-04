# Trigger-action editor integration (Phase 6) — implementation record

**Status: done.** All four stages shipped and confirmed against a real
scene file — a binding's "▶ Fire" button started its timeline and moved
the target node exactly as authored. `xrds-editor` compiles clean
(`cargo check -p xrds-editor`), and the frontend type-checks and builds
clean (`npx tsc --noEmit`, `npm run build` in `apps/xrds-editor`). See
[`xrds-trigger-action-v1.md`](xrds-trigger-action-v1.md)'s Phase 6
section for the condensed version of everything below; this doc is the
full detail — architecture decisions, per-stage build notes, and every
follow-up bug found while testing it.

**Follow-up fix: trigger preview.** After Stage 3/4 landed, testing revealed
that authoring a binding in the editor had no way to actually *fire* it —
desktop editing generates no real `ZoneEnter`/`Grabbed`/etc event, and
`XrdsAPI::fire_trigger()` (added in Phase 10 specifically for "an editor's
preview affordance") had never been wired into any UI. Fixed by:
- `XrdsUpdateContext::fire_trigger()` (`crates/xrds-runtime/src/xrds_api/context.rs`)
  — the `update()`-time counterpart to `XrdsAPI::fire_trigger`, since
  `update()` (not `setup()`) is where the editor's command-draining runs.
- `EditorCommand::PreviewFireTrigger{node_id, index}` → `EditorState::pending_fire_trigger`
  → drained in `bevy_scene.rs`'s `update()` — the same pending-field/drain
  pattern already used for `pending_gltf_play`/`pending_gltf_stop`, chosen
  specifically because it's already proven correct in this codebase rather
  than inventing a new mechanism.
- A "▶ Fire" button on each binding row in `TriggersSection` (disabled when
  the binding itself is disabled, or when its hand filter can never match).

**Note on verification for this fix specifically:** GUI click-automation in
this sandbox turned out to be unreliable in a way traced concretely (not
just suspected) — `WindowFromPoint` at the intended click coordinates
resolved to a different process than the editor even immediately after
`SetForegroundWindow` and after moving the window to a fresh screen
position, meaning visual compositing and actual input hit-testing have
diverged in this environment. This fix is therefore verified by compilation
(both `xrds-runtime` and `xrds-editor` build clean) and by exact-pattern
match against `pending_gltf_play`, which is already known-working code in
this same file — not by an interactive click-through. Worth confirming by
hand: select a node, add a trigger binding, click "▶ Fire", and confirm the
binding's runnable/sequence actually executes.

**Follow-up fix: default trigger kind on a new binding.** User feedback
after the fire-button fix: a freshly "+ Add binding"'d row silently started
on `ZoneEnter`, reading as already configured rather than needing a choice.
`XrdsTriggerKind` has no real "none" variant, so `AddTriggerBinding`
(`trigger_action.rs`, editor-side only — the domain type's own `Default`
impl is untouched) now constructs new bindings with `XrdsTriggerKind::Unknown`
instead — it already means "never fires" at runtime, which is exactly a
None state's behavior. `TriggersSection`'s dropdown lists it first, labeled
`"— none selected —"`. The Rust-side diagnostic for `Unknown`
(`trigger_diagnostics()` in `trigger_action.rs`, `xrds-scene-graph`) was
reworded to cover both real causes (not yet picked in the editor, or a
kind from a newer editor build) instead of only the latter. `xrds-scene-graph`
still 95/95.

**Follow-up fix: viewport click-selection during Play mode (pre-existing
bug, not introduced by this feature, found while testing it).** User
report: clicking the Cube in the viewport only ever selected the Plane —
but only while Play mode was active. Root cause:
`apply_camera_selection_system` (`viewport_camera.rs`) deactivates the
editor camera during play mode (a player pawn camera renders instead) but
leaves its `Transform` in place; `viewport_ray_selection`
(`viewport_selection.rs`) was still raycasting through that now-stale
editor camera every click, so the ray no longer corresponded to what was
actually on screen. Fixed by adding the same `if state.is_playing { return; }`
guard `orbit_camera_system` already uses for the identical reason — matches
existing precedent rather than inventing a new mechanism.

**Follow-up fix: no way to rename from the overlay.** `TriggerActionLibraryPanel`
had double-click-to-rename (copied from `HudLibraryPanel`), but
`TriggerActionEditorOverlay` — where a user actually spends their time
while authoring — had none at all. Added the same double-click-to-rename
inline edit directly to the overlay's title bar (Runnable targets only; a
binding has no name of its own to rename).

Stage 3 folded in one architecture change beyond what was originally
planned: `TriggerActionEditorOverlay` was generalized to accept a
`StepTarget` prop (`Runnable{name}` or `Binding{node_id, binding_index}`)
instead of a bare `runnableName: string`, so the same overlay component
serves both "edit a registry runnable" (from the sidebar panel) and "edit
one binding's inline sequence" (from the new Inspector `TriggersSection`'s
"Edit inline sequence…" button) — exactly the reuse the original plan
called for but left as an open question ("worth deciding the exact prop
shape when Stage 2 is implemented"). Stage 4 (`WatchersSection`) was folded
into the same change as Stage 3, per the plan's own note that this was fine
if it didn't add much review weight — it didn't; it's the same list-CRUD
shape with no step editor involved. This started as the approved
implementation plan for the `xrds-editor` authoring UI (kept here, rather
than only in an ephemeral plan-mode file, so it would survive context
compaction) and is now the as-built record of the same — nothing below
changed in substance once implementation started, only status markers.

**One real bug found and fixed while testing Stage 2:** selecting a step
immediately after adding it showed the "click a step to edit" hint instead
of its field editor. Root cause: `AddActionStep`/`AddTimelineKey` round-trip
through Rust before `snapshot.runnables` actually grows, so the effect that
populates the selected-step draft — watching only `[selected, runnableName]`
— saw the pre-growth (shorter) array at the moment `selected` was set to the
new index, and never re-ran once the real data arrived a frame later. Fixed
in `TriggerActionEditorOverlay.tsx` by adding a `JSON.stringify` of the
selected item itself as a third dependency, so the effect re-syncs once the
server-confirmed data lands — not just when `selected` changes.

## Context

The trigger-action system (sequences, timelines, the named-runnable registry,
`XrdsAction::Run` interop, and static diagnostics) is fully implemented and
tested in `xrds-scene-graph`/`xrds-runtime`, verified by 112+95 tests and a
visual example (`examples/xrds_first/trigger_action_timeline.rs`). It has no
editor authoring surface at all yet — every `XrdsTriggerBinding`,
`XrdsNamedRunnable`, and `XrdsThresholdWatcher` in this session's example had
to be hand-written as Rust struct literals. Phase 6 (`xrds-editor` UI) was
deliberately deferred until Phase 9a settled the binding schema
(`runnable: Option<String>` vs inline `sequence`) — that's now done.

Two Explore passes over `apps/xrds-editor` found the exact precedents this
plan builds from:
- The **document-level named registry** (HUD templates, `hud_library.rs` +
  `HudLibraryPanel.tsx`) is structurally identical to the runnable registry
  list — same "named things, create/rename/delete, document-scoped" shape.
- The **ordered array of typed variants with a kind picker, per-kind fields,
  and reorder/remove** (world-panel widgets, `WorldPanelCanvasOverlay.tsx`'s
  widget editor + `inspector.rs`'s `AddWorldPanelWidget`/`MoveWorldPanelWidget`
  handlers) is the exact shape an `XrdsAction` step list or `XrdsTimelineKey`
  list needs.
- The **"pick a foreign id/name with a dangling-reference warning"** pattern
  (`PlayerAnchorSection`'s HUD-template `<select>`, `Inspector.tsx:777-797`)
  is the template for a binding's `runnable: Option<String>` picker.

**A third question this session raised: should sequence/timeline editing get
its own separate window, since it's a genuinely bigger authoring task than
fits an Inspector sidebar?** A third Explore pass specifically checked
whether this editor has any precedent for a second WebView or a second OS
window. It does not, and there's a documented reason: an earlier version of
this editor (`apps/xrds-editor-tauri`, removed in commit `e207a97`) *did* run
two separate windows — a Tauri-managed window on the main thread and a
Bevy/winit window on a background thread — and the project deliberately
migrated away from that two-event-loop design to the current single-OS-window
architecture (`apps/xrds-editor/README.md`) specifically to avoid that
complexity and latency. Every piece of the current wry integration —
`EDITOR_WV` (`wry_overlay.rs:137-139`), `WryEditorReady`, `EditorBridge`, the
`ipc_handler` closure, and the `SetWindowRgn`/XShape/CAShapeLayer viewport-hole
singletons (`wry_overlay.rs:167-199`) — assumes exactly one WebView. Adding a
real second one (same window or a new OS window) means turning ~8 singleton
statics into per-window state, or re-solving the exact two-event-loop
teardown/sync problems this codebase already hit and reverted from once.

That's the wrong trade for this feature. What actually gets the "dedicated,
spacious editing surface" feeling the user wants, at effectively zero
architectural risk, is the pattern **already used three times** in this
codebase for exactly this purpose: `ApkExportDialog.tsx`, `HudCanvasOverlay.tsx`,
and `WorldPanelCanvasOverlay.tsx` are all same-WebView React components that
render as a full-viewport takeover, using one existing IPC message
(`{type:"set_viewport_hole", enabled:true}`) to tell Rust to temporarily
clear the clip region so this one WebView can paint over the 3D viewport.
No new window, no new WebView, no new singleton refactor — just another
component mounted the same way `ApkExportDialog` is in `App.tsx:177-185`.
This is what the plan below builds: a `TriggerActionEditorOverlay` that
*feels* like its own window (full-screen, modal, dismissable) but is
architecturally identical to a dialog this codebase already has three of.

## Decisions made

- **Data model stays exactly as shipped** — `XrdsSceneDocument.runnables`,
  `XrdsSceneNode.triggers`/`.watchers` continue to save/load as part of the
  ordinary scene document. No separate file format, no independent
  save/load system. (Confirmed with the user: a truly independent
  library-file format was considered and explicitly deferred as
  out-of-scope for this pass.)
- **Full-viewport same-WebView overlay**, not a second WebView or OS window
  — per the architectural finding above.
- **Bespoke DTOs**, not raw domain types, for the wire format — consistent
  with every existing feature (`HudTemplateDto` mirrors `XrdsHudTemplate`
  the same way), keeps the wire contract decoupled from the domain model.
- **Every mutation goes through `session.0.edit(|doc| ...)`** — this is what
  gives undo/redo for free (`XrdsSceneDocumentSession`, whole-document
  snapshot stack); there is no other correct way to mutate the document.
- **Diagnostics ride along for free**: `XrdsSceneDocument::trigger_diagnostics()`
  already exists and was explicitly shaped like `XrdsSceneAssetDiagnosticEntry`
  for this purpose. The Rust snapshot builder calls it once per frame; the
  UI just renders what comes back (per-node ones inline on each binding row,
  node-less registry ones — unknown `Run` targets, cycles — in the overlay).

## Data model recap (no changes needed here — already shipped)

`crates/xrds-scene-graph/src/scene/trigger_action.rs` and `timeline.rs`:
- `XrdsTriggerBinding { trigger: XrdsTriggerKind, sequence: XrdsSequence, disabled: bool, hand: Option<XrGrabHand>, runnable: Option<String> }`
- `XrdsSequence { steps: Vec<XrdsAction> }`, `XrdsAction` = adjacently-tagged enum (`PlayGltfAnimation`, `StopGltfAnimation`, `SetVisible(bool)`, `Teleport{destination}`, `ModifyHealth{target,delta}`, `Wait{seconds}`, `FireCustomEvent{name}`, `Run{runnable,wait}`, `Unknown`)
- `XrdsTimeline { keys: Vec<XrdsTimelineKey>, duration_secs: Option<f32>, looping: bool }`, `XrdsTimelineKey { at_secs: f32, action: XrdsAction }`
- `XrdsNamedRunnable { name: String, runnable: XrdsRunnable }`, `XrdsRunnable = Sequence(XrdsSequence) | Timeline(XrdsTimeline)`
- `XrdsSceneDocument.runnables: Vec<XrdsNamedRunnable>` (document-level), `XrdsSceneNode.triggers: Vec<XrdsTriggerBinding>` / `.watchers: Vec<XrdsThresholdWatcher>` (per-node)
- `XrdsSceneDocument::trigger_diagnostics() -> Vec<XrdsSceneTriggerDiagnostic { node_id: Option<XrdsSceneNodeId>, severity, title, detail }>`

## Stage 1 — Bridge plumbing (no UI yet)

Foundation for everything else; verifiable with `cargo check` alone before
any TSX exists.

**New file `apps/xrds-editor/src-tauri/src/trigger_action.rs`**, mirroring
`hud_library.rs`'s two-function shape:
- `build_runnables_dto(doc) -> Vec<NamedRunnableDto>` — full body included
  (steps or keys), not a summary-then-fetch-detail split: the whole bridge
  relies on "the snapshot already has everything" (`selected_node` works
  this way too), and this data is authoring-scale, not runtime-scale.
- `build_trigger_diagnostics_dto(doc) -> Vec<TriggerDiagnosticDto>` — the
  full `trigger_diagnostics()` output; the overlay filters to `node_id: None`
  rows itself, `inspector.rs` filters to `node_id == Some(this node)`.
- `apply_trigger_action_command(cmd, session, state) -> bool` handling every
  command below via `session.0.edit(|doc| ...)`.

Add a `runnable_mut(&mut self, name: &str) -> Option<&mut XrdsNamedRunnable>`
helper on `XrdsSceneDocument` (`document/core.rs`, next to `hud_template_mut`
— **done**). A `resolve_step_target_mut<'a>(doc: &'a mut XrdsSceneDocument, target: &StepTargetDto) -> Option<&'a mut Vec<XrdsAction>>`
helper centralizes step-list lookup so every step command handler is a
one-liner against it. Steps live in one of two places, so one addressing
enum covers all step commands (timeline keys only ever live in a registry
runnable, so key commands just take a plain `name: String`):
```rust
enum StepTargetDto { Runnable { name: String }, Binding { node_id: u64, binding_index: usize } }
```

**Commands** (all new `EditorCommand` variants, `bridge.rs` + hand-mirrored
in `bridge.ts` — no codegen exists in this repo, confirmed convention):
- Registry: `CreateRunnable{name, kind: "sequence"|"timeline"}` (reject
  duplicate names, log+no-op like HUD's convention), `DeleteRunnable{name}`,
  `RenameRunnable{old_name,new_name}`, `SetTimelineLooping{name,looping}`,
  `SetTimelineDuration{name,duration_secs}`.
- Steps: `AddActionStep{target: StepTargetDto, kind: String}` (default-valued
  per kind, mirrors `AddWorldPanelWidget`'s per-kind defaults,
  `inspector.rs:571-588`), `RemoveActionStep{target,index}` (bounds-checked
  remove), `MoveActionStep{target,index,delta}` (bounds-checked swap, same
  shape as `MoveWorldPanelWidget`, `inspector.rs:608-627`),
  `SetActionStep{target,index,action: XrdsActionDto}` (whole-step replace).
- Timeline keys: `AddTimelineKey{name,at_secs,kind}`, `RemoveTimelineKey{name,index}`,
  `SetTimelineKey{name,index,key: XrdsTimelineKeyDto}`.
- Per-node bindings: `AddTriggerBinding{node_id}`, `RemoveTriggerBinding{node_id,index}`,
  `SetTriggerBindingTrigger{node_id,index,trigger: XrdsTriggerKindDto}`,
  `SetTriggerBindingHand{node_id,index,hand: Option<String>}`,
  `SetTriggerBindingDisabled{node_id,index,disabled}`,
  `SetTriggerBindingRunnable{node_id,index,runnable: Option<String>}`.
- Per-node watchers: `AddWatcher{node_id}`, `RemoveWatcher{node_id,index}`,
  `SetWatcher{node_id,index,watcher: ThresholdWatcherDto}`.

**DTOs** — added to `bridge.rs` (**done**): `XrdsActionDto` mirrors
`XrdsAction` (one variant per kind, same adjacent-tag JSON shape the real
type already has). `XrdsTimelineKeyDto{at_secs,action}`.
`NamedRunnableDto{name, body: RunnableBodyDto::Sequence{steps}|Timeline{keys,duration_secs,looping}}`.
`TriggerBindingDto` mirrors `XrdsTriggerBinding` field-for-field.
`ThresholdWatcherDto` mirrors `XrdsThresholdWatcher`. `TriggerDiagnosticDto{node_id: Option<u64>, severity, title, detail}`.
`StepTargetDto` addresses either a registry runnable's body or one binding's
inline sequence.

**Wiring**: extend `NodeInspectorDto` (`bridge.rs`) with
`triggers: Vec<TriggerBindingDto>`, `watchers: Vec<ThresholdWatcherDto>`, and
`trigger_diagnostics: Vec<TriggerDiagnosticDto>` (**done**), populated in
`build_node_inspector()` (`inspector.rs:21-49`) — rides the existing
per-frame snapshot, no new fetch needed. `EditorSnapshot` gains
`runnables: Vec<NamedRunnableDto>` and `trigger_diagnostics: Vec<TriggerDiagnosticDto>`
(document-level subset, `node_id: None`). `bevy_bridge.rs`: call the two
`build_*` functions inside `broadcast_editor_snapshot_system`, OR
`apply_trigger_action_command` into the dispatcher chain the same way
`apply_hud_library_command` is (`bevy_bridge.rs:33-39`).

## Stage 2 — `TriggerActionEditorOverlay` (the "dedicated window")

**New file `apps/xrds-editor/src/components/TriggerActionEditorOverlay.tsx`**,
mounted in `App.tsx` exactly like `ApkExportDialog` (`App.tsx:177-185`):
`{showTriggerEditor && <TriggerActionEditorOverlay snapshot={snapshot} send={send} onClose={...} />}`,
sending `set_viewport_hole:true` on mount / `false` on unmount (same effect
`ApkExportDialog.tsx:21-27` already uses). Opened via a new toolbar/menubar
entry ("Trigger-Action Library") — `Toolbar.tsx`/`Menubar.tsx` already have a
button-triggers-a-`useState`-flag convention (`showApkExport` etc. in `App.tsx`)
to copy.

Two-pane layout, list + detail, same shape `HudCanvasOverlay.tsx` already
uses for its item list + selected-item editor:
- **Left**: the runnable list (name, a "Sequence"/"Timeline" kind badge,
  step/key count) — create/rename/delete, copied directly from
  `HudLibraryPanel.tsx`'s row conventions (double-click-to-rename, `confirm()`
  before delete). Registry-level diagnostics (`node_id: None` rows from
  `trigger_diagnostics`) render as a warning list here, reusing the
  `var(--red)` styling `Inspector.tsx:793-797` already establishes — no new
  diagnostic-display component needed.
- **Right**: whichever runnable is selected. If `Sequence`, an
  `ActionStepEditor` (steps below); if `Timeline`, the same editor plus
  `duration_secs`/`looping` fields and each row also showing its `at_secs`.

**`ActionStepEditor` as an internal sub-component of the overlay** (not a
separate file yet — promote it later only if the per-binding inline-sequence
case in Stage 3 needs to reuse it standalone): copies
`WorldPanelCanvasOverlay.tsx`'s widget editor structure directly (lines
472-584 per the earlier exploration) — a row of "+ kind" buttons to add a
step, a selected-step detail panel switching on `step.kind` with small
field-builder helpers (`numField`/`vec3Field`/`textField`/`boolField`,
optimistic local state, commit on blur — same convention as the HUD item
editor), and up/down/remove buttons identical in shape to
`WorldPanelCanvasOverlay.tsx:565-584`. `Run`'s `runnable: String` field is
itself a name-picker into `snapshot.runnables` with the same
dangling-reference-warning `<select>` pattern used for the HUD-template
picker — reused a third time in this plan.

## Stage 3 — Per-node Triggers section (Inspector)

Stays a *small* list in the Inspector sidebar — the heavy editing surface is
the Stage 2 overlay, not this section.

**`Inspector.tsx`**: new `TriggersSection({ node, snapshot, send })` rendered
unconditionally right after `<PayloadSection .../>` (line 269) — a new
pattern in this file (nothing today renders per-node regardless of payload
kind), but small and self-contained. Each binding row: trigger-kind `<select>`
(for `Custom(name)`, an extra text input appears — same conditional-field
idiom as `TextSection`'s anchor switch), hand-filter `<select>`, disabled
checkbox, then either the `runnable` name-picker `<select>` (dangling-warning
included) **or** an "Edit inline sequence…" button that opens the Stage 2
overlay pre-focused on `StepTargetDto::Binding{node_id, binding_index}` instead
of a registry runnable — mutually exclusive in the UI, matching the data
model's actual runtime priority (`runnable` wins when set). Render this
node's diagnostics (`node_id == Some(node.id)` rows) inline per binding, e.g.
"⚠ this binding can never fire" for a hand-filter-on-handless-kind Error.

This means the overlay from Stage 2 needs to accept an optional
"open focused on this specific binding" entry point, not only "open to the
registry list" — worth deciding the exact prop shape when Stage 2 is
implemented (e.g. `initialTarget?: StepTargetDto`), rather than building two
divergent code paths.

## Stage 4 — Per-node Watchers section

Same shape as Stage 3 but simpler — a watcher is one flat struct, not a
sequence, so no step editor involved: `WatchersSection` in `Inspector.tsx`,
using the `AddWatcher`/`RemoveWatcher`/`SetWatcher` commands from Stage 1.
Small enough to fold into the Stage 3 change if it doesn't add much review
weight, or ship as its own quick follow-up once Stage 3 is actually done.

## Verification

- After each stage: `cargo check -p xrds-editor` (scoped, not
  `--workspace --all-targets` — this repo's build is heavy; linker/debug-info
  settings were already tuned this session for exactly this reason) plus
  the project's TS build check in `apps/xrds-editor` to catch mirror drift
  between `bridge.rs` and `bridge.ts`.
- Run the editor and, per stage, visually confirm: the overlay opens/closes
  cleanly via `set_viewport_hole` (viewport reappears correctly on close —
  this is the one part of the "full-viewport overlay" approach worth
  actually watching happen, since it's the one piece of platform-specific
  plumbing involved), create a runnable, add a step, verify undo (Ctrl+Z)
  reverts it, and verify the resulting scene document actually drives the
  live viewport the same way the hand-authored `trigger_action_timeline.rs`
  example does for an equivalent binding.
- No existing Rust test suite covers the editor bridge (`hud_library.rs` has
  no `#[test]`s either) — consistent with the existing convention of
  verifying editor features by running them, not unit tests; this feature
  doesn't need to introduce a new testing bar for itself.
