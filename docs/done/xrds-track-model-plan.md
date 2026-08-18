# Track model — sequencer rework plan

Supersedes the Sequencer-v2 work recorded in
`docs/done/xrds-sequencer-v2-implementation-plan.md`. That document's Phase A–D
history is still accurate as history; this one replaces its *model*.

This file exists to survive a context reset. It records the decisions, the
evidence behind them, the exact current build state, and what remains — so
work can resume without re-deriving anything.

---

## 1. Terminology

| Term | Meaning |
|---|---|
| **Timeline** | The *ruler* — the time axis a Track is authored against. Not a data type. Has no representation in the schema. |
| **Track** | A timeline-based sequence: a set of assets with action events pinned to absolute local times. Runs on its own clock, started by a trigger. |
| **Asset row** | One node's lane inside a Track. A node appears **at most once per Track**, so all of that node's events live on one row. |

"Action Chain" is **retired**. It was introduced earlier in this work as a
display name for `XrdsRunnable::Sequence`; that concept has since been
deleted outright (§2). Nothing should reintroduce the name.

---

## 2. Why Action Chains / `XrdsSequence` were deleted

A Sequence was a relative, completion-chained queue; a Timeline was
absolute-time. These were believed to be two genuinely different execution
models. They were not — **for the action set that actually exists**.

What actually blocked a Sequence
(`crates/xrds-runtime/src/xrds_api/trigger_action.rs`, `fn is_finished`):

| Action | Blocks for |
|---|---|
| `Wait { seconds }` | authored seconds |
| `AnimateTransform { duration_secs }` (now `SetTransform`) | authored duration |
| `Run { wait: true }` | until the child agent despawns |
| **everything else** | **nothing — completes instantly** |

`PlayGltfAnimation` is explicitly fire-and-forget: the comment in
`is_finished` reads *"the sequence advances as soon as playback is requested
rather than waiting for the clip to finish."*

So **every blocking duration in a Sequence was known at author time.** The
one thing completion-chaining exists for — waiting on something whose length
is not knowable until runtime — did not occur, because the only
unknown-length action deliberately does not block. Any Sequence therefore
converts to absolute times by accumulating authored durations, mechanically
and losslessly. That the conversion is mechanical *is* the proof the two were
never separate models.

Only `Run` resisted conversion, and `Run` was already being banned from
Tracks.

### What went with it

- `XrdsSequence`, `XrdsRunnable`, `XrdsNamedRunnable`, `scene/timeline.rs`.
- `XrdsAction::Wait` (a key carries its own time), `XrdsAction::Run`,
  `XrdsAction::FireCustomEvent`.
- `XrdsTriggerBinding::sequence` — the inline path. A binding now names a
  Track, so there is one way to author instead of two plus a diagnostic for
  authors who set both.

### The cost, stated plainly

Removing `Run` removes the only way one runnable starts another. **A Track
cannot start another Track.** Composition is "bind several Tracks to one
trigger" — concurrent only, and subject to the disjoint-asset rule (§4).

In practice this bites less than it sounds, because concurrency lives
*inside* a Track: two nodes changing together is two asset rows; one node
moving and recolouring on the same beat is two keys at the same `at_secs` on
one row. The normal shape is **one Track per trigger**, doing everything.

What is genuinely lost is *sequential* Track→Track chaining. When that is
needed, the clean way back is a **`TrackComplete` trigger kind** — not
reintroducing `Run`. Do not reintroduce `Run`.

Threshold watchers are unaffected: a watcher's `fires` is its own field, not
the `FireCustomEvent` action.

The `bevy-sequential-actions` machinery stays, purely internal — each Track
key is still spawned as its own one-step agent.

---

## 3. Schema (landed)

`crates/xrds-scene-graph/src/scene/track.rs`:

```rust
pub struct XrdsTrackKey   { pub at_secs: f32, pub action: XrdsAction }
pub struct XrdsTrackAsset { pub target: XrdsActionTarget, pub keys: Vec<XrdsTrackKey> }
pub struct XrdsTrack {
    pub assets: Vec<XrdsTrackAsset>,
    pub duration_secs: Option<f32>,
    pub looping: bool,
}
pub struct XrdsNamedTrack { pub name: String, pub track: XrdsTrack }
```

A key carries **no** target of its own — the owning row names the node. That
is what makes rows node-scoped instead of action-category-scoped, which was
the original bug (`deriveLanes` grouped by action category, and for a
registry timeline every row collapsed to "Self" because no key had any node
identity to offer).

Helpers on `XrdsTrack`: `flattened_keys()`, `key_count()`,
`effective_duration_secs()`, `owned_nodes()`.

Helpers added elsewhere so the editor and diagnostics cannot drift apart:
- `XrdsAction::self_duration_secs()` — non-zero only for `SetTransform`;
  drives both the Track duration fallback and the editor's dot-vs-bar.
- `XrdsAction::own_target()` — `SetMaterial`/`ModifyHealth` only.
- `XrdsAction::is_valid_in_track()`.
- `XrdsTriggerKind::carries_hand()`.

`XrdsSceneDocument`: `runnables` → `tracks`, plus `track()` / `track_mut()`.
No serde alias and no `normalize()` — see below.

### No migration — deliberately

**Nothing had been persisted in the old schema when it changed, so there is
nothing to convert.** Verified: the only `scene.json` in the tree is under
`android/quest/build/`, which is gitignored, untracked, and contains no
sequencer data at all. No `.xrds` files exist.

So there is no `legacy_keys` field, no `XrdsTrack::normalize()`, no
`XrdsSceneDocument::normalize()`, no `#[serde(alias = "runnables")]`, and no
Sequence→Track converter. All of that was written and then removed once this
was confirmed — carrying a migration path for a format nothing is written in
would be dead code plus a standing "did every load path remember to call
`normalize()`?" obligation, which was risk #3 in an earlier draft of this
plan. That risk no longer exists.

Forward compatibility is a *separate* concern and is still handled:
`XrdsAction::Unknown` catches action variants written by a newer editor
(realistic — scenes get pushed to a Quest APK that may lag the editor), and
`track_diagnostics` reports them as unrecognized rather than dropping them
silently.

If a document ever does need migrating in future, the version field
(`XRDS_SCENE_DOCUMENT_VERSION`) is the hook — not a serde alias.

---

## 4. Cross-Track asset conflict policy — decided

**Reject the newcomer, atomically.**

When a trigger tries to start a Track and any of its assets are already held
by a running Track, refuse to start it **at all**. Do not start it
partially; do not interrupt the incumbent.

Rationale:
- **Atomic beats partial.** Running a Track while skipping its contended
  rows produces choreography that plays *wrong* rather than not at all. A
  door that opens while its hinge stays put is far harder to diagnose than a
  door that did not open.
- **Protect the incumbent.** A scenario that completes is worth more than one
  that starts. Preemption is the same corruption that motivated banning
  `Run` from Tracks, arriving from outside.
- **Cheapest correct option.** Reject is a registry lookup at spawn.
  Queueing needs deferred-start bookkeeping, deadlock handling, and yields a
  Track whose internal timing no longer relates to the trigger that fired it.

Because reject is otherwise a *silent* no-op, it must be observable:
- runtime `warn!` naming both Tracks and the contended asset;
- the editor snapshot reports the last conflict so the Sequencer status bar
  can say *"Crane Lift blocked by Dock Sequence (asset: crane_arm)"*.

The **primary** defence is authoring-time, not runtime: `track_diagnostics`
warns on every Track pair sharing an asset. The runtime guard is the backstop.

### Three consequences that must be implemented

1. **Looping Tracks never release their assets**, so anything sharing one can
   never run. Permanent rather than situational → **error** severity, naming
   the looping side. (Landed in diagnostics.)
2. **Re-firing the same Track restarts it** from zero. That is not a
   conflict. The guard must key on the agent and exempt same-Track re-fire,
   or a Track will reject itself.
3. **The guard keys on resolved `Entity`, not authored target.** `SelfNode`
   and `TriggerSource` only become concrete at fire time, so two Tracks both
   using `SelfNode` on different nodes must not false-conflict. This also
   means **the registry must be cleaned up when an agent despawns**,
   otherwise it leaks held assets and blocks everything forever. This is the
   single most likely bug in this work — it gets an explicit test.

Deferred deliberately: a per-Track `on_conflict: Reject | Preempt` field.
There is a real future need ("cutscene overrides ambient idle"), so the
decision lives in **one function** to keep adding it a one-line branch.

---

## 5. Preview / play — decided

The Sequencer's play button is **separate from simulation play**. Sim Play
(`SetPlayMode`) stays untouched.

Key finding: `advance_timelines` is registered unconditionally in `Update`
(`crates/xrds-runtime/src/xrds_api/install.rs:210`) and is **not** gated on
play mode — only physics is paused. So previewing needs no play mode at all,
just an agent spawned. The editor already has the precedent:
`PreviewFireTrigger` → `pending_fire_trigger` → `ctx.fire_trigger`.

`XrdsTimelineAgent`'s `elapsed_secs`, `keys`, `next_key_index`,
`duration_secs`, `looping` are all **private**; `target`, `source`,
`chain_depth`, `is_recovery` are `pub`.

### Sandbox model

Entering Sequencer view snapshots the scene; preview mutates freely.

Important distinction that the "ask to save on exit" idea must respect:

- **Authoring edits** (adding Tracks, rows, keys) are *document* changes and
  go through the normal dirty/undo flow. A preview prompt must **never**
  discard these.
- **Preview side-effects** (a running Track moving a transform) only touch
  *runtime entities*. The document still holds the authored values.

Because the document is authoritative, restoring is just re-applying its
values to the affected entities — the path `pending_translations` /
`set_translation_for_node` already uses. So restore is **automatic on preview
stop**, no prompt. In-flight `XrdsTransformTween` components must be stripped
too, or they keep running after stop.

**Playhead:** now feasible and cheap. Earlier in this work it was called
impossible; that was right about *scrubbing* and wrong about *display*. Once
the editor owns the agent and can read `elapsed_secs`, a live playhead during
preview is nearly free. Dragging it still needs a real seek — not in scope.

---

## 6. Other settled decisions

- **Display:** one Track's rows at a time against the ruler, with the left
  panel listing Tracks. *Not* all Tracks stacked on a shared master ruler —
  a trigger-fired Track has no fixed start time, so a block on a master
  timeline would have no honest x-position (the inconsistency the design
  assessment already flagged).
- **Mute / Solo / Lock: removed.** Deliberate simplification.
- **Trigger → Track only.** A Trigger fires a Track. Nothing fires a Track
  from inside another Track.

---

## 7. Build state — all phases landed, plus post-launch polish/fixes

| Target | State |
|---|---|
| `cargo check --workspace --all-targets` | **0 errors** |
| `xrds-scene-graph` | **117 tests pass**, 0 ignored |
| `xrds-runtime` | **124 tests pass** |
| `apps/xrds-editor` frontend | `tsc` clean, **56 vitest pass**, `vite build` clean |

Landed after the model itself, in response to real editor use:

- **§9's `Unknown` bug fixed** — hand-written `Deserialize` on `XrdsAction`.
- **`SetMaterial`'s color field is a real color swatch + alpha/metallic/
  roughness sliders**, not four raw 0–1 number boxes — the boxes' native
  range-validation tooltip ("must be ≤ 1") was confusing, and sliders can't
  go out of range at all.
- **Sequencer transport got a ⏮ Restart button**, wired to the same
  stop-then-replay `preview_play_track_in_world` already does when called
  for whatever is currently previewing.
- **Restore-on-stop/restart was missing material** — it only ever put
  transform/visibility back, silently never touching color/metallic/
  roughness. Fixed by `restore_track_nodes_from_document` in
  `bevy_scene.rs`, which reads a Track's own authored `Node` rows straight
  from the document (not from what a live agent reports touching, so it
  works even after natural completion, which is what Restart needed).
- **Pause did not pause in-flight interpolation** — `advance_tracks` skipping
  a paused agent only stopped *new* keys firing; an already-running
  `SetTransform`'s tween lives on the target with no link back to its
  agent, so `advance_transform_tweens` kept driving it regardless of pause.
  Fixed by freezing tweens on entities held by any currently-paused agent.
  Regression test: `pausing_a_track_also_freezes_its_own_in_flight_interpolation`.
- **Position/Rotation/Scale fields support drag-to-scrub** (`DragNumber` in
  `SequencerInspector.tsx`) — click still types, drag adjusts by `step` per
  pixel.
- **Overlapping events on one row now stack into sub-lanes** instead of
  literally rendering on top of each other. `stackKeys()` in `lib/sequencer.ts`
  is a greedy interval-scheduling layout — first-fit into the lowest lane
  whose last occupant has already "ended" — using a minimum visual footprint
  derived from the lane's actual rendered pixel width (measured live via
  `ResizeObserver`), not a fixed time window, so it degrades correctly at any
  zoom level. Row height (`SUBLANE_STEP` per extra lane) is computed once and
  shared between the asset-name column and the lane column so they stay
  aligned — the same pre-existing alignment invariant every other row
  property already depends on.

  **Shipped broken the first time**, worth recording because the failure mode
  was invisible rather than loud. Two compounding bugs: (a) the
  `ResizeObserver` effect was keyed `[]`, but the element it observes only
  exists while a Track is open — and the Sequencer opens on "No Track open",
  so it attached nothing and never retried, leaving the measured width at 0
  forever; (b) a 0 width produced a 0 dot footprint, and a 0 footprint makes
  a dot's interval *degenerate* (`[at, at]`), which `stackKeys` correctly
  treats as non-overlapping — so stacking silently did nothing at all, even
  for two dots on the exact same timestamp. The unit tests all passed
  because every one of them hardcoded a non-zero `minSeconds`; nothing tested
  the value the component actually computes. Fixed by extracting
  `dotFootprintSecs()` with a fallback width so it can never return 0, keying
  the effect on whether a Track is open, and measuring once directly rather
  than waiting on the observer. The guarantee now has its own test, plus one
  that pins the 0-footprint trap so it reads as known rather than latent.

  **Then it was reported broken again**, and the structural lesson is the one
  worth keeping: both regressions were in *component wiring*, never in
  `stackKeys`, which was correct and unit-tested throughout. Untested wiring
  around tested logic is where this kept failing. So the layout maths moved
  out of `SequencerWorkspace.tsx` into `lib/sequencer.ts` —
  `layoutAssetRow()`, `keyTopPx()`, `ROW_H`/`SUBLANE_STEP`/`DOT_FOOTPRINT_PX`
  — leaving the component to do nothing but measure a width and paint the
  numbers it is handed. The px the component renders are now directly
  assertable without a DOM (there is no jsdom in this project).

  **Mutation-tested, and it mattered.** Reintroducing the original bug on
  purpose showed only *one* test failing, because the `layoutAssetRow`
  fixture happened to contain a 0.5s bar — which overlaps on authored
  duration alone, and so stacks correctly even with a broken footprint. The
  case actually reported (two *instant* events sharing a timestamp, whose
  only overlap is the pixel footprint) was unguarded at the integration
  level. Added `twoInstantDots` cases; the bug now fails 2 tests. Worth
  remembering that a green suite said "covered" while the reported scenario
  was not.
- **`serve_dist`'s missing `Cache-Control` header** (`wry_overlay.rs`) — real
  bug, found because "I rebuilt and relaunched and it's still stale" kept
  happening. WebView2's HTTP cache is heuristic by default without an
  explicit header, and — the actual trap — that cache lives in a **persistent
  user-data folder that survives process restarts**
  (`target/debug/xrds-editor.exe.WebView2/`), not just within one running
  window. So neither rebuilding `dist/` nor relaunching the app was ever
  enough on its own; the browser was replaying a cached response without ever
  asking the handler again. Fixed with `Cache-Control: no-store` (every
  response here already re-reads from disk, so nothing wants caching, ever);
  the pre-existing stale cache folder had to be deleted once by hand to clear
  what was already poisoned.
- **`SetMaterial`/`ModifyHealth`'s own `target` field removed** — the last
  leftover from before rows were asset-scoped. Every other action (
  `SetTransform`, `SetVisible`, `PlayGltfAnimation`, `StopGltfAnimation`)
  already applies to whichever row it sits on; these two alone still carried
  an independent target that defaulted to `SelfNode` and so almost never
  matched the row it was just added to — surfacing as the "Action escapes its
  asset row" diagnostic on essentially every authored `SetMaterial`/
  `ModifyHealth` event. Removed the field end-to-end (domain, both DTOs, the
  TARGET picker in `SequencerInspector.tsx`) rather than fix the default,
  since the diagnostic's own text was describing a real hazard the row model
  exists to prevent (an action touching a node the conflict check can't see).
  The runtime handlers now resolve directly against `self.target` — the
  row-resolved entity every `XrdsActionRunner` already carries — the same way
  `SetTransform`/`SetVisible` always have. `BRIDGE_VERSION` 2 → 3.

### Texture slots on `SetMaterial` — landed

`XrdsAction::SetMaterial` gained `texture: Option<XrdsActionTexture>`
(`{ slot, texture_asset_id }`). `BRIDGE_VERSION` 3 → 4.

- **The runtime needed almost nothing.** `SetMaterial`'s handler already did a
  read-modify-write of `XrdsMaterialParams`, which already had a `textures`
  field that `apply_authored_material_to_entity` already resolved into real
  `Handle<Image>`s off the imported asset catalog. The whole runtime change is
  one `params.textures.set(slot, …)` call.
- **One slot per event, not a whole slot set.** Replacing the set would make
  "assign a base-colour map" silently drop an authored normal map. Driving
  several slots at one instant is several events sharing a timestamp on one
  row — which the sequencer now stacks into sub-lanes, so it reads fine.
- **`texture_asset_id: None` clears the slot**, which is why it is nullable
  rather than the whole `texture` being absent: "clear the normal map at t=2s"
  is a real thing to author, and is distinct from "leave every slot alone".
- **No new asset-catalog DTO.** `EditorSnapshot.asset_catalog` already existed
  (`palette::build_asset_catalog`, whose `kind` is already `"Texture"`), so the
  picker consumes that. A second parallel type was written and then deleted —
  worth noting because the near-miss was a duplicate source of truth for the
  same data.
- Diagnostics: a texture id absent from the catalog, **or present but not of
  kind `Texture`**, is an error (it resolves to no image, so the slot silently
  keeps what it had). `"Material change sets nothing"` now counts a texture
  assignment as setting something — otherwise it flagged correct authoring.
- UV/sampler stay at defaults. Per-event UV offset/tiling wants interpolation
  to be useful, and this action applies instantly — a separate feature.

### Looping restores its assets' initial state — landed

A loop is a repeated Track: it does **not** rewind the world, it puts the
assets *it owns* back to the state they were in when the Track started, then
replays.

- **Captured at spawn, not read from the document.** A Track may be fired while
  its assets sit somewhere the document never described (another Track moved
  them, gameplay moved them, the previous lap moved them). "Repeat this
  choreography from where it began" is what an author means; snapping to
  authored values would teleport assets at the first lap boundary. Tested
  explicitly by parking an asset off-document before starting.
- **Presentation state only** — transform, visibility, material. **Health is
  deliberately excluded:** `ModifyHealth` accumulates gameplay state, and
  restoring it every lap would make a looping health drain a permanent no-op,
  undoing exactly what it just did.
- **Only looping Tracks capture.** A one-shot Track can never lap, so it does
  not pay the per-asset material read. Asserted both ways.
- **In-flight tweens are stripped first.** A `SetTransform` still mid-glide at
  the wrap would keep interpolating toward last lap's destination and overwrite
  the restore a frame later. Same trap the preview stop path already had.
- **Queued *before* the new lap's keys**, so at the command flush the restore
  lands first and the lap's events apply on top of a clean slate — queueing it
  after would undo the lap it just began.
- **No restore in the `duration_secs <= 0` branch.** There a "lap" is one
  frame and every key fires every frame, so the restore would never be visible.
  That configuration is already diagnosed.
- **Mutation-tested:** deleting the restore block fails both behavioural tests.
  They were also rewritten to poll (`spin_until`) rather than assert after a
  fixed update count — action effects are deferred and wall-clock time jitters
  under parallel test load, which made them pass alone and fail in the full
  run. Verified stable over 5 consecutive full-suite runs.

### Authored edits reach running agents — landed

Reported symptom: tick Loop, then change the total duration, and the Track
keeps lapping at the *old* duration.

Cause: [`XrdsTrackAgent`] is a **snapshot** taken at spawn. That is what keeps
the hot path cheap (no registry lookup per agent per frame), but it also meant a
running Track ignored every authored change — not just duration; `looping`
itself and key timings were equally stale.

Fix: a `sync_live_track_agents` system, explicitly `.before(advance_tracks)` so
an edit lands on the same frame rather than a lap later. It re-reads each live
agent's Track from `XrdsTrackRegistry` and adopts `looping`, `duration_secs`
and key timings/actions.

Two details that are easy to get wrong:

- **A shortened duration must wrap the clock.** Otherwise `elapsed_secs` sits
  past the new end and the Track waits out one more full lap at the old length
  before the change is felt — the original bug wearing a different hat.
- **`next_key_index` is re-derived from the clock**, not carried over: keys now
  in the past must not re-fire, and keys an edit moved into the future must
  still be able to.

**Structural edits are deliberately *not* adopted.** If the set of resolved
asset entities changes (a row added, removed, or re-pointed), the agent is left
alone — whole, never half-adopted. Adopting it would mean rewriting
`XrdsTrackAssetLocks` mid-flight, and an error there leaks a lock and blocks
that asset for the rest of the session (§10's first risk). It would also
invalidate the loop-restore `initial` baseline, captured for the old set. The
author re-fires (⏮) to pick a structural change up. Tested both ways.

`schedule_track_keys()` was extracted so spawn and re-sync share one
implementation — a running agent re-reading its Track must schedule it
*identically* to a fresh spawn, and two copies would drift.

Mutation-tested: unregistering the system fails 3 tests. Stable over 5
consecutive full-suite runs (these tests advance real wall-clock time).

### Preview transport — landed

Design notes worth keeping:

- **`XrdsUpdateContext::world` is `pub(super)`**, so the editor cannot reach the
  world directly. The transport goes through four new context methods
  (`preview_play_track`, `preview_pause_track`, `preview_stop_track`,
  `track_preview_state`) wrapping `*_in_world` helpers — the shape
  `fire_trigger` already used.
- **Preview is single.** An `XrdsTrackPreview` marker identifies the editor's own
  agent, and starting a new preview stops the old one *first*, so its locks are
  freed before the new Track claims them. Without that ordering a preview could
  refuse itself.
- **Preview goes through the ordinary conflict guard.** Previewing a Track whose
  assets a running Track holds is refused exactly as a real firing would be —
  the preview should show what would actually happen, refusal included. On
  refusal the editor raises a status message rather than doing nothing.
- **`SelfNode`/`TriggerSource` rows cannot be previewed.** They only become
  concrete when a trigger supplies them, so the first resolvable `Node` row is
  used as the stand-in target, and a Track with none is refused with a log line
  saying why.
- **Restore is the editor's job, not the runtime's.** `preview_stop_track`
  returns the `XrdsId`s the preview drove; only the editor holds the authored
  document to restore *from*. It re-applies
  translation/rotation/scale/visibility via the existing `set_*_for_node` path.
- **In-flight tweens are stripped on stop.** A `SetTransform` mid-flight
  leaves an `XrdsTransformTween`, and `advance_transform_tweens` would keep
  driving it after the agent is gone — undoing the restore a frame later. The
  subtle one.
- **Restore now covers material, not just transform/visibility** (fixed after
  a real report: a `SetMaterial`-reddened cube stayed red forever, on both
  Stop and the later-added Restart). See §7's "Landed after the model" list.
- **Stop routes through `despawn_agents_releasing_locks`**, never a bare despawn,
  so a preview cannot leak locks.
- The live preview and the last conflict are mirrored from the world into
  `EditorState` once per frame, because the snapshot builder has no world access.
  That mirror animates the transport timecode and the playhead.

### Resolved log (was "Still open")

Everything below landed. Kept struck-through with its original reasoning
intact, because the *why* is the reusable part. For what remains genuinely
unfinished, see §10's "Known gaps".

- ~~**`Teleport` is redundant and should be removed.**~~ **Landed** (`BRIDGE_VERSION` 1 → 2). `Teleport` deleted, `AnimateTransform` renamed to `SetTransform`, and the "Interpolation has no duration" warning **deleted** — with `Teleport` gone, duration 0 is the normal way to author an instant change, so warning on it would flag correct authoring. The editor's Mode control now sets duration 0 vs > 0 instead of switching variants, and remembers the last non-zero duration so flipping back is non-destructive. Note there is an unrelated `XrdsPlayerLocomotionMode::Teleport` that must NOT be renamed. Original reasoning: A zero-duration
  `AnimateTransform` is *exactly* `Teleport`: the `duration_secs <= 0.0` path in
  `on_start` does `*transform = target`, and `target` takes `start.rotation` /
  `start.scale` for unset fields — so it writes back the rotation and scale it
  just read, leaving only translation changed. That is byte-for-byte what
  `Teleport` does, and `AnimateTransform` is *strictly more expressive*, since
  `Teleport` cannot touch rotation or scale at all.

  **The non-obvious consequence:** the "Interpolation has no duration"
  diagnostic must be **deleted**, not reworded. It currently warns on
  `duration_secs <= 0` and says "Use Teleport if that is what you meant."
  Without `Teleport`, zero duration becomes the normal way to author an instant
  change, so that warning would fire on correct authoring.

  Everything else is already free: the editor presents both as one Mode toggle
  already (it just becomes duration 0 vs > 0 underneath), and dot-vs-bar
  rendering already derives from `self_duration_secs()`, so a zero-duration
  event already draws as a dot.

  **Worth doing at the same time:** rename `AnimateTransform` → `SetTransform`.
  Once it absorbs instant changes the "Animate" prefix misdescribes half its
  uses, and renaming is nearly free while the variant is already being touched.

- ~~**§9's `Unknown` forward-compat bug**~~ **Fixed.** See §9 — `XrdsAction` now
  has a hand-written `Deserialize` that checks the tag before touching `data`.
  The acceptance-criterion test is un-ignored and passing.
- ~~**The Rust↔TS bridge has no drift protection.**~~ **Landed.** Two guards:

  1. **`BRIDGE_VERSION`** — a constant in `src-tauri/src/bridge.rs` mirrored in
     `src/types/bridge.ts`, carried in every snapshot as
     `EditorSnapshot::bridge_version`. `BridgeMismatchBanner` compares them and
     shows a hard banner naming both versions, the fix, and the two files to
     check. It renders *ahead of* the editor and returns early, because a
     mismatch means the snapshot shape is untrustworthy and the panels reading
     it may throw — the message has to survive the case where nothing else
     mounts. **Bump the constant on every DTO change**; not bumping it removes
     the only thing that would have told anyone.
     **Verified live** by deliberately setting the TS side to 99 and confirming
     the banner appeared with the right text, then reverting.
  2. **`EditorCommand::ReportBridgeError`** — an internal-only variant the
     frontend never sends. `wry_overlay::ipc_handler` synthesises it when an
     inbound command fails to decode and pushes it through the ordinary command
     queue, purely to reach `pending_status` so the rejection shows in the
     editor's status bar. Previously a rejected command was completely silent:
     `useSendCommand` is fire-and-forget, so the UI never learned, and the
     `warn!` reached nobody watching the window. **Wired and compiling, but not
     exercised** — triggering it needs the frontend to deliberately send a bogus
     command, which was not worth the churn.

  Still worth considering later: **codegen** (`ts-rs`) would make drift a
  compile error rather than a runtime banner. The version check is the cheap
  change-agnostic guard, not a replacement for that.

  *Original problem, for context:* `src/types/bridge.ts` is a
  hand-written mirror of `src-tauri/src/bridge.rs` with nothing linking them, so
  divergence produces **no compile error on either side**. Outbound, a stale
  command is `warn!`ed and dropped — `useSendCommand` is fire-and-forget, so the
  UI gets no feedback at all. Inbound, a missing snapshot field is `undefined`
  and the first `.map()` throws; `defaultSnapshot` does *not* shield this, since
  it is only the initial `useState` value and is replaced wholesale. Neither
  direction is covered by any test. Recommended: a bridge version constant plus
  making dropped commands surface in the editor status bar (cheap, change-
  agnostic); codegen (`ts-rs`) is the real fix but costs a proc-macro dependency
  and the hand-written doc comments.


### Files already changed

- `crates/xrds-scene-graph/src/scene/track.rs` — **new**.
- `crates/xrds-scene-graph/src/scene/timeline.rs` — **deleted**.
- `crates/xrds-scene-graph/src/scene/mod.rs` — module swap.
- `crates/xrds-scene-graph/src/scene/trigger_action.rs` — action variants
  removed, `XrdsSequence` deleted, binding rewritten, helper impls added,
  `trigger_diagnostics` → `track_diagnostics` (+
  `track_conflict_diagnostics`), all `Run`-only free helpers deleted.
- `crates/xrds-scene-graph/src/document/core.rs` — `runnables` → `tracks`,
  `track()` / `track_mut()`.

Also to update (build documents in code, so they need porting, not
migrating): `examples/xrds_first/trigger_action_sequence.rs` and
`examples/xrds_first/trigger_action_timeline.rs`.

---

## 8. Phase plan — all landed (historical)

**Nothing here is outstanding.** Kept as the record of what was done in what
order; see §7 for the state it actually landed in. Two references below went
stale afterwards and are left as written rather than silently rewritten:
`action-escapes-its-row` was later *deleted* along with `SetMaterial`'s own
`target` field, and `AnimateTransform` is now `SetTransform`.

### Phase 1b — scene-graph tests
- Delete tests for `Wait` / `Run` / `FireCustomEvent` / Sequence round-trips
  / `Run`-cycle diagnostics.
- Convert Timeline tests to Track shape; keep the `AnimateTransform` /
  `SetMaterial` coverage.
- **New tests:** duplicate asset row; unknown action in a Track;
  action-escapes-its-row; dangling row target; shared-asset warning; looping
  Track error; `effective_duration_secs` including the interpolation tail;
  `flattened_keys` ordering across rows (including two rows with keys at the
  same `at_secs`).
- No migration tests — there is no migration (§3).

### Phase 2 — runtime
- `fire_timeline_key` → `fire_track_key`, resolving the **row's** target
  rather than the agent-wide one; agent-wide target remains the `SelfNode`
  fallback.
- Agent flattens rows to one `at_secs`-sorted list at spawn.
- Asset-conflict guard per §4, with the three consequences implemented and
  the despawn-cleanup test.
- Public `elapsed_secs()`; per-agent pause gate.
- World helper to spawn a preview agent for a named Track.

### Phase 3 — bridge
- DTO mirror for the Track shape.
- `PreviewPlayTrack { name }` / `PreviewPause` / `PreviewStop`.
- Snapshot: `preview { name, elapsed, duration, playing }`, last-conflict
  report, Track diagnostics.
- Entity restore-from-document on preview stop.

### Phase 4 — frontend
- `SequencerListPanel`: drop the two tabs, list Tracks only.
- Rewrite `lib/sequencer.ts`'s `deriveLanes` to node-keyed asset rows; the
  mockup's two-line row label (`Asset · Aspect` over the node name).
- "+ Asset" node picker; exclude nothing (sharing is allowed) but surface the
  conflict warning.
- Own transport + live playhead; delete mute/solo/lock.
- Retire remaining "Action Chain" copy.

### Phase 5 — verify
Rust tests, vitest, `cargo check`, live editor check driving a real Track.

---

## 9. Bug found in passing — `Unknown` forward-compat was fatal, not lossy — FIXED

Not caused by this rework; found by a test written during it, fixed as a
follow-up once the Teleport-removal work landed.

`XrdsAction::Unknown` exists so a document written by a newer editor degrades
to one skipped action instead of failing the whole scene load — realistic,
because scenes get pushed to a Quest APK that may lag the editor. Measured
behaviour:

| Unknown action JSON | Result |
|---|---|
| `{"kind":"SomeFutureAction"}` | `Ok(Unknown)` ✓ |
| `{"kind":"PlayAudio","data":{"clip":"x.ogg"}}` | **`Err` — entire document fails to load** ✗ |

Cause: `#[serde(other)]` requires a *unit* variant, so `Unknown` cannot
consume the adjacent `data` payload. Every realistic future action carries
fields — the backlog's `PlayAudio { clip }` is the example in `Unknown`'s own
doc comment — so the fallback does not work for the case it was written for.
The doc comment calls this limitation "lossy"; it is actually fatal.

Recorded as an executable spec:
`a_payload_carrying_unrecognized_action_should_not_destroy_the_whole_document`
in `crates/xrds-scene-graph/src/tests/trigger_action.rs`. Was `#[ignore]`d;
now un-ignored and passing — **113 pass, 0 ignored** in `xrds-scene-graph`.

**Landed fix** (`crates/xrds-scene-graph/src/scene/trigger_action.rs`):
`XrdsAction` no longer derives `Deserialize`; it has a hand-written impl
instead. Shape:

1. Capture the wire value generically first (`serde_json::Value::deserialize`)
   — this is the whole trick, it lets the tag be inspected *before* `data` is
   touched at all.
2. Check `value["kind"]` against `KNOWN_ACTION_KINDS`, a `&[&str]` mirroring
   the real variant names. Not in the list → `Ok(Unknown)`, `data` never
   parsed, never errors.
3. In the list → deserialize the same value through `XrdsActionKnown`, a
   private shadow enum holding every real variant but not `Unknown` — so its
   own derive never hits the `#[serde(other)]`-eats-the-payload problem in the
   first place — then `.into()` to `XrdsAction`.

`XrdsActionKnown` and `KNOWN_ACTION_KINDS` must be extended by hand whenever a
new `XrdsAction` variant is added — commented at both definition sites,
pointing at each other.

Still lossy, deliberately: an unrecognized action's payload is discarded, not
retained (`Unknown(serde_json::Value)` was considered and rejected — it
changes the variant shape and every `matches!(…, Unknown)` call site, a
separate change from "loading must not fail," which is what mattered here).

Useful serde shapes confirmed while probing this (all adjacently tagged
except `XrdsActionTarget`):

- `XrdsTriggerKind` → `{"kind":"ZoneEnter"}`, `{"kind":"Custom","data":"c"}`
- `XrdsActionTarget` → `{"Node":7}` (externally tagged)
- `XrdsAction` → `{"kind":"SetTransform","data":{…}}`

## 10. Risks — final status

1. **Conflict-registry leak on agent despawn** — would silently block every
   Track forever. **Mitigated.** Every despawn path routes through
   `despawn_agents_releasing_locks`, with tests for natural completion,
   explicit stop, same-Track re-fire, and pause-keeps-locks. This risk is also
   why a *structural* edit to a running Track is skipped rather than adopted
   (§7) — adopting it would mean rewriting the lock table mid-flight.
2. **Preview mutating authored state** — **mitigated.** Restore-from-document
   on stop, in-flight tweens stripped, and material included in the restore
   (it was missing at first and shipped a visible bug).
3. **Scope** — retired. The tree builds; `cargo check --workspace
   --all-targets` is clean and every suite passes.

*(A previous risk — "`normalize()` not called on some load path" — was
retired by deleting the migration entirely; see §3.)*

### Known gaps, not risks

Deliberate limits, each recorded where it bites rather than only here:

- **Node-level material textures are not authorable.** `MaterialParamsDto`
  carries only base colour / metallic / roughness / emissive, so the Inspector
  cannot give a primitive a texture — only a Track's `SetMaterial` event can
  swap one at runtime. The document schema and runtime both support it; it is
  the editor DTO and UI that stop short.
- **Widget triggers are unreachable.** `ButtonPress`/`ButtonRelease`/
  `SliderChange`/`ToggleChange` target a widget's ephemeral entity, and widgets
  are authored inside a `WorldPanel`'s `widgets` array rather than as their own
  nodes, so no document binding can receive them. Pre-existing; the picker now
  says so instead of offering them silently.
- **No seek/scrub.** The preview playhead is read-only; seeking would need
  every crossed event re-evaluated.
- **Structural edits need a re-fire** while a Track runs (§7).
- **`ReportBridgeError` is wired but never exercised** — triggering it needs
  the frontend to send a deliberately bogus command.
- **`ts-rs` codegen** would turn bridge drift into a compile error rather than
  a runtime banner. `BRIDGE_VERSION` is the cheap guard, not a replacement.
