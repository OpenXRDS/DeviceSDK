> **Superseded in part.** The *model* described here — two execution models
> (`XrdsSequence` / `XrdsTimeline`), "Action Chain" as a display name,
> action-category lanes — has been replaced by the Track model. See
> **`docs/done/xrds-track-model-plan.md`** for the current design, the evidence
> for collapsing the two execution models into one, the cross-Track asset
> conflict policy, and the live build state. Everything below stays
> accurate as a record of how the work got here.

# Sequencer redesign — implementation plan

Checklist form of the decisions in
[`xrds-sequencer-v2-design-assessment.md`](xrds-sequencer-v2-design-assessment.md).
Phases are ordered by dependency — runtime/schema first, since the editor
UI for the new actions can't be built against types that don't exist yet;
editor-side derived views next, since they need no schema change and can
land independently of the runtime work; frontend redesign last, since it
consumes both.

Check items off as they land. Each phase should end green (tests/build)
before the next starts.

**Crate structure — staying put.** Considered and declined splitting
trigger-action into its own crate. The real boundary here is already
document-schema (`xrds-scene-graph`, engine-agnostic) vs. runtime
execution (`xrds-runtime`, Bevy-dependent) — that split is intentional
and already correct. Total trigger-action code today is ~2,120 lines
across three files; this phase adds a few hundred more, which is normal
module growth, not crate-boundary territory. No independent reuse case
exists yet (nothing needs trigger-action's types without the rest of
`xrds-scene-graph`). Revisit only if a real standalone-consumer shows up
(e.g. a headless scene validator) — and even then, likely just the
schema half, not the Bevy runtime side too.

## Bevy engine feasibility — what's native, what's a workaround, what's a real gap

Checked against Bevy 0.17.2 and this repo's actual dependency tree/runtime
code (not assumed) before going further. Four categories:

**A — Fully native, already built on existing Bevy/runtime capability.**
`Teleport`, `SetVisible`, `PlayGltfAnimation`/`StopGltfAnimation`,
`ModifyHealth`, `FireCustomEvent`, `Run`, and the new `SetMaterial` all
just mutate an existing Bevy component or reuse an existing runtime
helper (`set_material_params_for_entity_in_world`, etc.) — nothing here
needed anything Bevy doesn't already provide. Same for all pure-Rust
logic (`trigger_diagnostics()`, the diagnostics additions in Phase A).

**B — Frontend-only, Bevy irrelevant.** Everything in Phase C (lane
derivation, the reverse index, trigger-kind filtering) and most of Phase
D (Sequencer panel chrome, the docked Action Sequence Editor, the
Hierarchy Triggers grouping, diagnostics surfacing, the dual ruler's
*drawing*) is React/TypeScript in the wry webview — no Bevy engine
question applies at all.

**C — Bevy has no built-in feature for this; we already built our own
(or would extend the same one).** Confirmed via the dependency tree: no
`bevy_tweening`-style crate anywhere in this workspace, and Bevy's own
`bevy_animation` is asset/clip-based (pre-authored skeletal/glTF curves),
not ad-hoc runtime-authored tweening. `AnimateTransform`'s
`XrdsTransformTween` component + `advance_transform_tweens` system
(`crates/xrds-runtime/src/xrds_api/trigger_action.rs:863-905`) is a
hand-rolled fill for that real gap — a plain Bevy `Component` + `System`
+ `Res<Time>`, not a hack, just not something Bevy ships. Any future
animated `SetMaterial` (color fade / metallic ramp, already noted as a
deferred follow-up) would extend this same infrastructure, not need new
engine capability.

**D — Real gaps: no existing hook, need new work before the feature can
exist at all.** Found while checking Phase D's remaining items against
actual runtime code, not assumed from the mockups:

1. **No timeline/sequence seek/scrub API.** `XrdsTimelineAgent::elapsed_secs`
   (`trigger_action.rs:658-671`) is private and only ever advances forward
   by real `Time::delta_secs()` each frame (`advance_timelines`,
   `trigger_action.rs:678-724`) — there is no way to jump to an arbitrary
   time and have intervening keys apply instantly. **This blocks
   interactive playhead-drag preview** (drag the scrubber in the Sequencer
   panel, see the scene update live at that moment) — the mockups both
   show a draggable playhead, but today the only way to "preview" a
   moment is to actually run real time forward to it. Needs a new `seek()`
   path before that specific interaction can be built; everything else in
   Phase D works without it (you can still author, just not scrub-preview).
2. **No audio action, and no imperative audio control at all.**
   `XrdsAction::PlayAudio`/`StopAudio` is listed in
   `docs/xrds-trigger-action-backlog.md` as a *future* sketch, not
   implemented. More fundamentally: there is no imperative play/stop/pause
   method anywhere in `xrds-runtime`/`xrds-components` for an
   `XrdsAudioClip` today — playback is purely declarative (`autoplay` at
   import time only). `crates/xrds-audio/src/lib.rs` is an empty file.
   **This means the "Audio" tab floated in the Sequencer chrome
   (`Sequencer Editor.dc.html`) has nothing to schedule** — it would need
   a new action variant, new imperative playback-control methods (a
   bigger lift than `SetMaterial`, since none exist even outside the
   trigger system), and runtime dispatch, essentially from zero. Confirmed
   already flagged as an inert-placeholder candidate in Phase D; this is
   why.
3. **Mute/Solo has no live-execution hook — only two readings are
   possible, and they cost differently.** Confirmed: the *only* existing
   suppression flag anywhere is `XrdsTriggerBinding::disabled`
   (`trigger_action.rs:338`, checked in `consume_triggers` at
   `trigger_action.rs:370`); no separate mute/solo mechanism exists.
   - **Reading 1 — pure visual dimming, no execution effect** (a muted
     track just *looks* muted while you edit; Play mode ignores it
     entirely): this is Category B, free, no runtime change. This is what
     "ephemeral/session-only" was decided to mean when Mute/Solo/Lock was
     discussed.
   - **Reading 2 — mute actually suppresses firing during live preview**
     (what this doc's own Mute/Solo explanation upstream implied — "stops
     its effects from firing... while you scrub"): this needs a small but
     real runtime hook — an editor-session-only "shadow disable" set,
     checked in `consume_triggers`/`fire_trigger_in_world` *alongside*
     `disabled` without ever touching the persisted flag. Category D, not
     free. **Decide which reading is wanted before building the Mute/Solo
     buttons in Phase D** — the UI looks identical either way, but only
     one of them needs a runtime change.

None of D's three items block the rest of the plan — Phases A-C are done
and unaffected, and Phase D's panel/grouping/diagnostics work doesn't
depend on any of them. They're specifically the interactive-preview and
audio-scheduling affordances the mockups show that don't have a home in
today's runtime yet.

## Phase A — Runtime & schema (Rust) — ✅ done

`crates/xrds-scene-graph/src/scene/trigger_action.rs` unless noted.

- [x] Add `XrdsEaseCurve` enum: `Linear`, `Quad`, `Cubic` (`#[default]`),
      each implicitly ease-out only (no In/InOut variants for v1).
- [x] Add `XrdsAction::AnimateTransform { position: Option<[f32;3]>,
      rotation: Option<[f32;3]>, scale: Option<[f32;3]>, duration_secs:
      f32, ease: XrdsEaseCurve }`. `Teleport` stays unchanged (the
      "instant jump" mode; `AnimateTransform` is the "interpolate" mode —
      the two present together as one Mode toggle in the redesigned
      Inspector, not as unrelated action-type picker entries).
- [x] Add `XrdsAction::SetMaterial { target: XrdsActionTarget,
      base_color: Option<[f32;4]>, metallic: Option<f32>, roughness:
      Option<f32> }` (instant-only, reuses the existing
      `XrdsActionTarget` enum).
- [x] `trigger_diagnostics()`: added Info diagnostics for
      `AnimateTransform` with `duration_secs <= 0` and for either new
      variant with every field unset (no-op step), plus extended the
      existing dangling-node-target Error check to cover `SetMaterial`'s
      target alongside `ModifyHealth`'s.
- [x] `crates/xrds-runtime/src/xrds_api/trigger_action.rs`: runtime
      interpolator —
  - [x] `XrdsTransformTween` component (not agent/runner state — inserted
        on the *target* entity, so overlapping tweens on different
        targets never collide) + `advance_transform_tweens` system,
        registered in `Update` (`install.rs`). `is_finished` polls for
        the component's absence, same pattern as `Run { wait: true }`
        polling for its child agent's despawn.
  - [x] `ease_out()` for `Linear`/`Quad`/`Cubic`.
  - [x] Blocks the queue inside a `Sequence` (no `wait` flag needed,
        unlike `Run` — `AnimateTransform` never spawns another agent, so
        fire-and-forget isn't a meaningful mode). Inside a `Timeline`,
        each key already runs as its own one-step agent
        (`fire_timeline_key`), so blocking there is harmless.
  - [x] `XrdsActionRunner` dispatch for `SetMaterial` — reuses the
        existing `material_params_for_entity_in_world`/
        `set_material_params_for_entity_in_world` helpers (already
        `pub(super)` in `xrds_api::helper`, visible from `trigger_action`
        via the existing `use super::*`) rather than duplicating material
        application logic.
- [x] Tests — 9 new (`xrds-scene-graph`: round-trip, ease/target JSON
      defaults, 6 diagnostics cases; `xrds-runtime`: zero-duration instant
      path + partial-field isolation, full duration reaching target while
      blocking the queue, `SetMaterial` applying only provided fields).
      All existing `Run`-chain/`RunawayDetected` tests unaffected.
- [x] `cargo test -p xrds-scene-graph -p xrds-runtime` green — 115/115
      (`xrds-runtime`), 103/103 (`xrds-scene-graph`).

## Phase B — Editor bridge mirror (Rust ⇄ TypeScript) — ✅ done

This codebase hand-mirrors `EditorCommand`/DTOs between
`apps/xrds-editor/src-tauri/src/bridge.rs` and
`apps/xrds-editor/src/types/bridge.ts` — no codegen, so both sides need
the same edit twice.

- [x] Mirrored `XrdsAction::AnimateTransform` and `XrdsAction::SetMaterial`
      into `XrdsActionDto` (`bridge.rs`) and the TS `XrdsAction` union
      (`bridge.ts`), plus `action_to_dto`/`action_from_dto` conversions and
      a `default_action_for_kind` entry for each.
- [x] `XrdsEaseCurve` mirrored as a plain `String` ("Linear"/"Quad"/
      "Cubic") on both sides — matches the existing convention for other
      small closed sets (`repeat`, `hand`, `crossing`), not a new nested
      DTO enum. `ease_curve_to_dto`/`ease_curve_from_dto` helpers added
      alongside `action_target_to_dto`/`_from_dto`.
- [x] Confirmed `AddActionStep`/`SetActionStep`/etc. needed no changes —
      generic over `XrdsAction` already, new variants flow through
      unchanged. `cargo check -p xrds-editor` clean.
- [x] `TriggerActionEditorOverlay.tsx`'s `ACTION_KINDS`/`ACTION_ICONS`/
      `summarizeAction()` updated — both new kinds are addable and
      summarized (showing which fields are set vs. still unset, so an
      unfinished step reads as unfinished at a glance). Per-field editors
      (Mode toggle, ease picker, color swatch) deliberately deferred to
      Phase D — this is the stepping stone, not the final UI.
- [x] `npx tsc --noEmit` and `npm run build` clean.

## Phase C — Editor-side derived views (no schema change)

Pure frontend computation over data already present in `EditorSnapshot`
— plus one small additive snapshot field, discovered mid-phase to be
necessary (see below). All in `apps/xrds-editor/src/lib/sequencer.ts` +
`sequencer.test.ts`, ready to run via `npm test` (introduced `vitest` —
this frontend had no test runner at all before this phase).

- [x] **Snapshot addition, discovered necessary while implementing the
      reverse index**: `EditorSnapshot` only ever carried the *selected*
      node's `.triggers` — a hierarchy-wide view had no per-node binding
      data for any node other than whichever one happened to be selected.
      Added `EditorSnapshot.all_node_bindings: NodeBindingSummary[]`
      (`node_id`, `node_name`, `binding_index`, `binding`), built by
      `build_all_node_bindings_dto(doc)` (new, in `trigger_action.rs`,
      alongside `build_runnable_diagnostics_dto`). Purely a read-only
      snapshot-serialization addition — the persisted `.xrds` document
      schema is unchanged, same category of change as
      `runnable_diagnostics`.
- [x] **Track/lane derivation**: `deriveLanes(body: RunnableBody):
      Lane[]`, keying each step/key by (resolved target, coarse action
      category). `Teleport`/`AnimateTransform`→Transform,
      `SetVisible`→Visibility, `PlayGltfAnimation`/`StopGltfAnimation`→
      Animation (all implicitly "Self" — no explicit target field);
      `ModifyHealth`/`SetMaterial`→Health/Material keyed by their actual
      `XrdsActionTarget`. `Wait`/`FireCustomEvent`/`Run`/`Unknown` (no
      meaningful target) fall into one shared "Flow" lane.
- [x] **Triggers hierarchy reverse index**: `buildTriggerReverseIndex(
      allNodeBindings): TriggerReverseIndex` — `byRunnable` (which
      bindings name runnable R) and `byNode` (which bindings live on node
      N), built from the new `all_node_bindings` field above.
- [x] **Contextual trigger-kind filtering** — `validKindsFor(node):
      string[]`. **Corrected from the original plan after checking the
      actual runtime dispatch code**:
  - [x] `ZoneEnter`/`ZoneExit`: only on `Other{kind:"InteractionZone"}`
        (InteractionZone has no dedicated `NodePayload` variant — it's
        the generic `Other` catch-all, not a type string on its own).
  - [x] `Grabbed`/`Dropped`: only when `node.grabbable`.
  - [x] `HoverEnter`/`HoverExit`: only on a `WorldPanel` node — these
        resolve via `self.panel_id` (a real `XrdsId`), confirmed in
        `trigger_action.rs`.
  - [x] **`ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange`:
        never offered, on any node** — corrected from the plan's original
        "only on a WorldPanel widget" assumption. These events target the
        individual widget's own ephemeral runtime `Entity`
        (`XrdsTriggerRef::Entity(button_entity)`, not an `XrdsId`), and
        widgets are authored as plain data inside `WorldPanel.widgets`
        (`WorldWidget`), never as their own importable `XrdsSceneNode`.
        `consume_triggers` looks for `XrdsTriggerBindings` on that exact
        entity, which no document-authored binding can ever be attached
        to. **This is a real, pre-existing gap, not introduced here**: a
        binding with one of these kinds silently never fires today, and
        `trigger_diagnostics()` doesn't flag it — worth a follow-up
        diagnostic (see Open follow-ups below), out of scope for this
        pass, which only decides what the picker offers.
  - [x] `AnimationComplete`: only on `GltfAsset`.
  - [x] `Custom`/`RunawayDetected`: always offered.
- [x] **Hand-filter visibility**: `isHandFilterVisible(kind): boolean` —
      kept matching `trigger_diagnostics()`'s rule exactly (all 8
      hand-carrying kinds, including the four `validKindsFor` now
      excludes) rather than narrowing it too — a document binding
      authored via expert code or an older editor build can still have
      one of the excluded kinds, and its hand picker must still render
      correctly for that case.
- [x] Unit tests — 16, all passing: 4 `deriveLanes`, 3
      `buildTriggerReverseIndex`, 7 `validKindsFor`, 2
      `isHandFilterVisible`.
- [x] `npx tsc --noEmit`, `npm test`, `npm run build`, `cargo check
      -p xrds-editor` all clean.

## Phase D — Frontend redesign

`apps/xrds-editor/src/components/` unless noted. Visual direction:
design #2 (`Sequencer Editor.dc.html`)'s chrome; Action Sequence Editor
panel restored per the revised requirement.

- [x] **Trigger-kind picker wired to Phase C's functions** (pulled forward
      from below — this was ready to wire in immediately). `TriggersSection`
      (`Inspector.tsx`) now calls `validKindsFor(node)` for the kind
      dropdown's options (still unions in the binding's own current kind if
      it's no longer offered, so a pre-existing/legacy binding doesn't
      silently mismatch its Select value) and `isHandFilterVisible(kind)`
      to show/hide the hand picker — replacing the file's own local
      `HANDLESS_KINDS` set, now a single source of truth shared with
      `sequencer.test.ts`. `tsc`/`vitest`/`vite build`/`cargo check` clean.
- [x] **Terminology sweep — checked, nothing to change.** Searched
      existing copy for "Pin" or "Sequencer" in the current trigger-action
      components: neither appears anywhere. The current panels
      ("Trigger-Action Editor"/"Trigger-Action Library") predate this
      redesign and don't yet claim to *be* "the Sequencer" — renaming them
      now, before the new bottom-docked Sequencer panel below actually
      exists, would create a new confusion (two things called "Sequencer")
      rather than resolve one. This sweep is preemptive guidance for the
      new components below to follow when built, not a fix for drift that
      doesn't exist yet.
- [x] **Sequencer panel — ✅ done** (new `Sequencer.tsx`, bottom-docked,
      full-width, sits above `Palette` only while a runnable/binding is
      open):
  - [x] Tabs: Timeline (active) / Curves / Audio — the latter two
        rendered `disabled` with a tooltip explaining exactly why, rather
        than either building fake functionality or silently omitting them:
        Curves has nothing beyond `AnimateTransform`'s fixed 3-variant
        `XrdsEaseCurve`, Audio has no action variant *or even imperative
        playback control* to schedule (both confirmed real gaps in the
        Bevy-feasibility assessment, not oversights here).
  - [x] **No playback transport built — a deliberate omission, not a
        missed item.** Confirmed against the runtime (Bevy-feasibility
        doc): `XrdsTimelineAgent::elapsed_secs` only advances forward via
        real `Time` deltas, no seek API exists. A disabled-looking
        play/pause/scrub control that does nothing on click would be
        actively misleading — worse than not having one. Everything else
        in this panel works without it, per that doc's own conclusion.
  - [x] Track/lane list via Phase C's `deriveLanes`, each lane a row with
        its label. `XrdsTimeline` lanes draw real key dots positioned by
        `at_secs / duration`; `XrdsSequence` lanes draw ordered numbered
        chips instead — never a fixed-position block, exactly the
        internal-inconsistency fix the assessment doc called for (a
        Sequence has no fixed start time to place one at).
  - [x] Click-to-select wired to the docked Action Sequence Editor via
        lifted state: `selected`/`onSelectedChange` moved out of
        `TriggerActionEditorOverlay`'s local `useState` into `App.tsx`,
        passed as controlled props to both panels. Aliased the prop to
        the same local name (`onSelectedChange: setSelected`) so every
        existing `setSelected(...)` call site inside the overlay needed
        zero further changes.
  - [x] Mute/Solo/Lock — ephemeral `Set<string>` React state local to
        `Sequencer.tsx`, keyed by lane key, never sent over the bridge.
        Dims this panel's own row rendering only (Solo dims every
        non-soloed lane, standard DAW/NLE convention) — does not affect
        the Action Sequence Editor, does not affect playback, per the
        assessment doc's "Reading 1" (the free one; "Reading 2" needs a
        runtime hook not built in this pass).
  - [x] `tsc`/`vitest`/`vite build`/`cargo check` all clean. Confirmed
        rendering correctly in a live session — both the docked editor
        and the new Sequencer panel showed up together, coexisting with
        no layout regressions, for a real open runnable.
- [x] **Action Sequence Editor panel — docked, ✅ done** (`TriggerActionEditorOverlay.tsx` +
      `App.tsx` + `editor.css`):
  - [x] Converted from a full-viewport modal to a docked column, sitting
        between the viewport and Inspector in `App.tsx`'s `.editor-panels`
        row (mirrors design #1's relative placement — viewport, then this
        panel, then the node-listing panel — adapted to this app's
        hierarchy-on-the-left layout). Removed the `set_viewport_hole`
        toggle entirely: a docked column never covers the viewport, so it
        never needed that mechanism (same as `Inspector`, which never
        touches it). New `.action-sequence-editor-wrap` CSS class
        (420px fixed width for now — resizing can reuse `useResizable`
        later if wanted, not added speculatively).
  - [x] Replaced the old 10-button "add step" row (which doesn't fit a
        420px-wide dock) with a single stateless dropdown.
  - [x] **Scope-to-one-lane was reconsidered and deliberately not done.**
        A `Sequence`'s steps are one globally-ordered array; physically
        filtering the editor to one lane would break `moveSelected`'s
        reorder semantics (moving a step "up" needs to know its real
        index in the full array, not a lane-local one) for a real
        correctness risk, for a benefit ("only see one lane's steps at a
        time") that a future Sequencer-panel-driven scroll-to-selection
        can deliver more safely. Full list stays; the bottom Sequencer
        panel (below) will scroll/highlight into it instead of filtering
        it.
  - [ ] Sequencer-panel click-to-select wiring (depends on the Sequencer
        panel below, not yet built).
  - [ ] Retire note no longer applies — there was never a second modal
        entry point once this became the only presentation.
- [x] **Field editors for the two new actions — done, in this panel (not
      `Inspector.tsx` — corrected mid-build: design #1's "Properties"
      panel this row came from is the *action/key* property editor, which
      in this app is this Action Sequence Editor's field-editor bar, not
      the Node Inspector, which shows the selected node's own static
      properties, a different concept entirely).**
  - [x] `Teleport`/`AnimateTransform` unified under one Mode toggle
        (`Select`: "Teleport (instant)" / "Interpolate"), switching
        preserves position where sensible. Per-field position/rotation/
        scale override (checkbox = `Some`/`None`, matching design #1's
        override-arrow affordance), duration, ease-curve picker
        (`Linear`/`Quad`/`Cubic`), and a warning when `duration_secs <= 0`
        (matches the Phase A diagnostic).
  - [x] `SetMaterial` fields — target picker (reused from `ModifyHealth`),
        base color (4 number fields; not yet a color swatch — plain
        numbers, consistent with every other vector field in this file),
        metallic, roughness, each independently toggleable
        inherit(`None`)/override(`Some`).
- [x] **Hierarchy — Triggers grouping — ✅ done** (`Hierarchy.tsx` +
      Rust/TS bridge):
  - [x] **Second snapshot gap found and fixed, same shape as
        `all_node_bindings`**: watchers had the identical problem —
        `NodeInspector.watchers` only ever carried the *selected* node's
        watchers, so the Watchers sub-row had nothing to derive from for
        any other node. Added `EditorSnapshot.all_node_watchers:
        NodeWatcherSummary[]` (`build_all_node_watchers_dto`, mirrored
        Rust ⇄ TS), same read-only-snapshot-addition category as
        `all_node_bindings`.
  - [x] Per-node "▸ Triggers" pseudo-row (only rendered when a node
        actually has bindings/watchers), expanding to **Bindings**/
        **Watchers** sub-rows (kept separate, not merged — a Watcher only
        fires a Custom name, it never names a runnable itself). Clicking a
        binding with a named runnable calls a new `onOpenRunnable` prop
        (threaded from `App.tsx`, same wiring `TriggerActionLibraryPanel`
        already uses) to jump straight into the docked Action Sequence
        Editor; an inline-sequence binding or a watcher just selects the
        owning node.
  - [x] `tsc`/`vitest`/`vite build`/`cargo check` all clean. Confirmed
        working against a live session, not just compiling — the running
        instance's own `CreateRunnable`/`AddActionStep`/`SetActionStep`
        commands round-tripped correctly through the rebuilt bridge.
- [x] **Diagnostics surfacing — ✅ done, and a real pre-existing gap fixed
      first, not a workaround** (`trigger_action.rs` in `xrds-scene-graph`
      + `TriggerActionEditorOverlay.tsx`):
  - [x] **Found while wiring this up, not before**: `trigger_diagnostics()`
        only ever walked an inline binding's `sequence.steps` — a named
        registry runnable's own body (the primary authoring surface once
        the library panel/docked editor exist) was **never validated at
        all**. Every per-step check (Unknown action, glTF-payload
        mismatch, `Custom`-no-listener, `Run`-unknown-target, dangling
        `ModifyHealth`/`SetMaterial` target, `AnimateTransform`/
        `SetMaterial` no-op) silently skipped every registry Sequence and
        Timeline. Fixed properly: extracted the shared logic into
        `push_step_diagnostics()` (parameterized over `node_id: Option`
        and `node_is_gltf: Option<bool>` — `None` for a registry entry,
        which has no single owning node so the glTF check is skipped
        rather than guessed, not silently wrong), called from both the
        existing inline-binding site and a new registry-body loop.
        Also fixed `emitted_custom` under-counting `FireCustomEvent`
        calls living inside a registry runnable's own body (previously
        only scanned inline sequences), which was producing false-positive
        "no listener" diagnostics for exactly that case.
  - [x] 6 new tests confirming the fix (Unknown-inside-registry-Sequence,
        dangling-target-inside-registry-Sequence, empty-registry-Sequence/
        Timeline, "key" vs "step" wording for Timeline entries, glTF-check
        correctly skipped for a registry runnable, the `emitted_custom`
        false-positive fixed) — `xrds-scene-graph` now 109/109,
        `xrds-runtime` unaffected at 115/115.
  - [x] Frontend: `TriggerActionEditorOverlay.tsx` now shows itemized
        severity/title/detail for whatever it has open — for a `Binding`
        target, reuses `selected_node.trigger_diagnostics` directly
        (already scoped, same selection-can't-change assumption the file
        already relies on); for a `Runnable` target, filters
        `runnable_diagnostics` by exact-quoted name (`` `runnable
        ${JSON.stringify(name)}` `` — anchored to Rust's `Debug` format
        for `String`, which always quotes, so two similarly-named
        runnables can never collide; not a fragile substring guess).
  - [x] `tsc`/`vitest`/`vite build`/`cargo check` all clean.
- [x] Apply the Tailwind/Radix design-token pass (from the earlier
      editor-wide redesign) to all new components as they're built, rather
      than writing new inline styles. Done, and taken further — see the
      visual-language pass below.

### Visual language — adopting the mockup's look, not just its layout

The structural pass reproduced `docs/Sequencer_Editor.dc.html`'s *layout*
but kept the editor's old Catppuccin Mocha palette, 6/8px radii and type
scale, so it still didn't read like the design. Restyled to match:

- [x] **Palette swapped to the mockup's cool slate/blue scheme**, done by
      rewriting the `:root` custom properties rather than touching
      components — `editor.css` *and* `tailwind.config.js` both resolve
      against those tokens, so one block restyles the whole app. Token
      names are deliberately unchanged. New surface tokens added where the
      old palette had no equivalent: `--elevated` (in-panel column
      headers), `--well` (recessed fields), `--sel` (selected row / active
      segment), `--bright` (titles vs `--text` body copy), `--blue-l`.
      `--mauve` now resolves to the light blue — the mockup contains no
      purple, and inventing one would have been worse than remapping.
- [x] **Fields are recessed wells** (`--well` + a visible `--surface0`
      border) instead of mid-grey fills, which is most of why the old
      look felt flat next to the mockup.
- [x] **Radii tightened** 6/8px → 3/5px to match the mockup's controls.
- [x] **`.tb-group` is a real segmented control** — one bordered well with
      1px-divided items and a `--sel`/`--blue-l` active segment, per the
      mockup's Select/Move/Rotate/Scale and Layout groups — rather than
      free-floating pills. Active states generally went from solid blue
      fills to the mockup's subtler tinted treatment.
- [x] **Metrics aligned**: 36px menubar, 44px toolbar, 31px panel header
      strips, 26px flat full-width tree rows (was inset rounded pills),
      mono uppercase captions for section labels.
- [x] **Panel headers unified** — sentence-case semibold titles on their
      own strip, no uppercase + accent-dot treatment. `.hud-library-header`
      was brought onto the same strip style so a docked library panel and
      the Hierarchy above it read as one column.

### Requirement change — Sequencers list panel, and "Action Chain"

Superseding both the mockup and the pass above, on direct instruction:

- [x] **Sequencers are no longer tree nodes.** The "Sequencers" group added
      to `Hierarchy.tsx` is removed — a sequencer isn't a scene node, so
      the tree was the wrong home for it. `Hierarchy` keeps only
      `onOpenRunnable`, used by its per-node Triggers rows.
- [x] **New `SequencerListPanel.tsx`** — the left column in Sequencer
      mode, with two tabs and a per-tab create button, rename
      (double-click), delete, a diagnostics badge, and an `.open` marker
      on whichever entry the workspace currently has loaded.
- [x] **"Action Chain"** replaces "trigger-based sequencer" as the display
      name for `XrdsRunnable::Sequence`. Chosen over *Cue* (jargon,
      misreads as "queue"), *Routine* (code-flavoured) and *Reaction*
      (undersells that it's an ordered multi-step chain). **Display copy
      only — no schema change**; the document type is still
      `XrdsRunnable::Sequence`. The start-mode readout also moved from
      "time-based / trigger-based" to "own clock / when triggered", which
      says what actually differs.
- [x] **The Hierarchy is gone from Sequencer view**, and the node
      Inspector stays (still needed to bind a trigger to a node).
- [x] **The Sequencer docks under the viewport, not across the window.**
      This is the load-bearing part of the fix: with the Sequencer inside
      the centre column, the left sidebar and the Inspector both keep the
      *full* window height, which is what actually stops the Inspector
      scrolling. Removing the Hierarchy from above it wasn't enough on its
      own — at `flex: 0 0 46%` the column was still only ~490px.
- [x] **Fog/Exposure/IBL suppressed in Sequencer view** via a new
      `showEnvironment` prop on `Inspector` (defaults on). They're
      scene-environment settings, not behaviour authoring, so in this
      context they were only noise. The nothing-selected state shows a
      pointer to Triggers instead.
- [x] Step inspector widened 340 → 400px so the Position x/y/z row stops
      wrapping.
- [x] Fixed the `.panel-lock-btn` overlapping panel-header titles, which
      the flush 12px-padded header strips exposed.
- [x] Verified end-to-end in a live session: created a Timeline from the
      new panel, added an `AnimateTransform` key, and confirmed it draws
      as a **bar** spanning its duration against the ruler, the track lane
      appears with M/S/L, every inspector field renders, and the status
      bar goes from "⚠ Empty timeline" to "1 track · 1 key · no
      validation errors". `tsc` / 25 tests / `vite build` clean.

**Not done — typeface.** The mockup specifies IBM Plex Sans/Mono. It isn't
bundled, and the wry webview loads offline from `xrds://`, so the
mockup's Google Fonts `<link>` would simply fail. The stacks now resolve
to the closest locally-available humanist sans / UI mono (Segoe UI
Variable Text, Cascadia Mono). Bundling IBM Plex via `@fontsource` is the
faithful fix and is a self-contained follow-up.

### Phase D revision — one Sequencer workspace, not scattered panels

The first Phase D pass built the sequencer as **two docked panels** (a
right-hand Action Sequence Editor column plus a short bottom lane strip)
bolted onto the existing scene layout, and left the runnable library as a
third separate sidebar panel. Reviewed against
`docs/Sequencer_Editor.dc.html` this was wrong on three counts, all now
fixed:

- [x] **It wasn't one panel.** The mockup is a single cohesive editor —
      transport header, then three columns (track list | ruler + lanes |
      per-item inspector), then a status bar. Merged into
      `SequencerWorkspace.tsx` (+ `SequencerInspector.tsx` for the
      inspector column, split only so neither file becomes unreadable).
      `TriggerActionEditorOverlay.tsx`, `TriggerActionLibraryPanel.tsx`
      and the interim `Sequencer.tsx` are all deleted; the runnable
      library is now a **Sequencers** group inside the Hierarchy tree
      (create/rename/delete/open), which is where the mockup puts it.
- [x] **The ruler was missing entirely.** Key dots were positioned by
      percentage with no time axis to read them against. Added a real
      ruler: `niceStep`/`rulerTicks`/`fmtTime` in `lib/sequencer.ts`
      (major ticks with `m:ss` labels at 1/2/5×10ⁿ intervals, minor ticks
      between), unit-tested — 9 new tests covering interval selection,
      float-drift-free monotonic ticks, endpoint inclusion, degenerate
      durations, and timecode formatting. A trigger-bound `Sequence` gets
      an **index** ruler instead of a time ruler, since it has no fixed
      clock (the internal-inconsistency fix, now visible in the axis
      itself rather than only in the key shapes).
- [x] **It was squeezed into the main layout.** Added a `workspace`
      switch (`"scene" | "sequencer"`) in the Toolbar. Sequencer mode
      reflows the whole window — viewport + Hierarchy/Inspector on top
      (`.editor-panels--seq`, ~46%), the workspace filling the rest,
      status bar at its base — instead of a 340px strip under everything
      else. Deliberately **not** a separate OS window and **not** a
      viewport-covering modal: the mockup keeps a live 3D preview, so the
      Bevy hole just moves and the `viewport_bounds` ResizeObserver
      re-reports it (`workspace` added to that effect's deps, since
      switching layouts can remount `.editor-center` and would otherwise
      leave the observer bound to a detached node).
- [x] Keys now render the mockup's **dot-vs-bar** language: an instant
      action is a dot, `AnimateTransform` stretches into a bar spanning
      its duration, colour-coded per action family. The ruler span also
      accounts for a trailing key's interpolation tail so it isn't
      clipped off the right edge.
- [x] Inspector column restyled to the mockup's labelled-block form
      (`ACTION TYPE`/`MODE`/`DURATION`/`FIRES WHEN`), with `FIRES WHEN`
      as a real read-only readout derived from
      `buildTriggerReverseIndex(all_node_bindings)`.

**Bug found and fixed during this pass** (shipped by the first Phase D
pass, caught from a user report of "nothing but the 3D scene is
clickable"): the interim `Sequencer.tsx` reused `.hud-canvas-no-panel`
for its empty state. That class is `position: absolute; inset: 0` and
belongs to the `position: fixed` `.hud-canvas-overlay` modal shell — used
inside the un-positioned `.sequencer-wrap` it resolved against the
initial containing block and **covered the entire window with an
invisible click-swallowing layer**, on every launch (the empty state
rendered whenever nothing was open). The Bevy viewport still worked
because it's a `SetWindowRgn` hole in a *different* HWND, which made it
look like a Bevy/wry input-routing problem rather than a CSS one. Fixed
with dedicated flow-positioned empty-state classes, plus `position:
relative` on the shell as defence-in-depth (documented inline as
load-bearing so it isn't "tidied" away). The two remaining
`.hud-canvas-no-panel` uses were audited and are correctly contained.

## Phase E — Verification

- [ ] `cargo check -p xrds-editor` clean.
- [ ] `npx tsc --noEmit` and `npm run build` clean (`apps/xrds-editor`).
- [ ] New/updated example under `examples/` exercising
      `AnimateTransform` (all three ease curves) and `SetMaterial`
      together in one scene — per this project's "major feature needs a
      visual example" norm. Screenshot + log check, per the established
      verification pattern for this sandbox (GUI click-automation is
      unreliable here — verify via compile + static screenshot + code
      review, and call out anything that still needs a manual
      interactive check).
- [ ] Manual click-through checklist for you to run (since interactive
      automation isn't reliable in this sandbox):
  - [ ] Add an `AnimateTransform` step, confirm the Mode toggle/duration/
        ease fields all round-trip correctly.
  - [ ] Add a `SetMaterial` step, confirm color/metallic/roughness
        round-trip.
  - [ ] Confirm the trigger-kind picker only offers valid kinds for a
        few different node types (a plain Cube, a WorldPanel button, a
        GltfAsset, an InteractionZone).
  - [ ] Confirm Mute/Solo/Lock reset after closing and reopening the
        editor (proving they're truly ephemeral, not accidentally
        persisted).
  - [ ] Confirm selecting a key in the Sequencer panel focuses the right
        row in the Action Sequence Editor, and vice versa.

## Open follow-ups (explicitly deferred, not blockers)

- [ ] `QuadIn`/`QuadInOut`/`CubicIn`/`CubicInOut` ease variants, if a
      real use case needs them later.
- [ ] Animated `SetMaterial` (color fade / metallic ramp), reusing
      `AnimateTransform`'s interpolator infrastructure.
- [ ] Structured `Emit Event` payloads beyond a plain name — excluded
      per the assessment doc's "no scripting surface" reasoning.
- [ ] Frame-based (fps) display option for the ruler — cosmetic
      conversion over `at_secs`, not required for this redesign.
- [ ] Persisted Mute/Solo/Lock, if/when tracks are ever reified into real
      addressable entities for an unrelated reason.
- [ ] **New diagnostic**: a `trigger_diagnostics()` entry flagging any
      binding with `trigger.kind` in
      `ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange` as an
      `Error` — "this can never fire" — discovered while building
      `validKindsFor` in Phase C (see that entry above for the full
      dispatch-code citation). This is a real, pre-existing silent-failure
      gap independent of the sequencer redesign; worth fixing regardless
      of when the rest of this plan lands.
