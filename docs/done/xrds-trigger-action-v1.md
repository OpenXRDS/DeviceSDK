# Trigger-action sequencing — v1 implementation record

**Status: done. Nothing left planned.** The completed record of what
shipped, phase by phase, and why each decision was made. Live at
`xrds-runtime` 112/112, `xrds-scene-graph` 95/95,
`cargo check --workspace --all-targets` clean, the Phase 5 example visually
confirmed by a human, and the Phase 6 editor UI confirmed against a real
scene file (fired a binding, watched its timeline actually move a node).

Companion docs:

- [`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md)
  — the design rationale (why the system is shaped this way).
- [`xrds-trigger-action-editor-plan.md`](xrds-trigger-action-editor-plan.md)
  — the Phase 6 editor-integration implementation record in full detail
  (architecture decisions, per-stage build notes, and every follow-up bug
  found while testing it).
- [`../xrds-trigger-action-backlog.md`](../xrds-trigger-action-backlog.md)
  — candidate `XrdsAction` variants, unscheduled.

Phase numbers are kept stable because code comments reference them. They are
ordered here by phase number, which is also the order they were built —
except Phase 10, a robustness pass done after Phase 7 and deliberately
before Phase 8; Phase 9/9a (timelines and interop), built after Phase 10;
and Phase 6 (editor integration), built last of all, once Phase 9a had
settled the data shape it needed to edit.

## Scope for v1

The seven `XrdsAction` variants already drafted (`PlayGltfAnimation`,
`StopGltfAnimation`, `SetVisible`, `Teleport`, `ModifyHealth`, `Wait`,
`FireCustomEvent`), and Option C for dynamic parameters
(`XrdsTriggerValue(f32)` component, read via `FromTriggerSource`).
Everything in the backlog doc is explicitly out of scope for v1 — pull items
in only when a real use case needs them.

Trigger kinds started at two (`ZoneEnter`/`ZoneExit`), gained
`AnimationComplete` in Phase 4, and reached **twelve** in Phase 7
(grab/drop, hover, button, slider, toggle, plus `Custom(String)`). See those
phases for why each was added.

## Phase 0 — dependency

- [x] Promoted `bevy-sequential-actions` to the workspace's
      `[workspace.dependencies]` and added it as a real dependency of
      `xrds-runtime` (`crates/xrds-runtime/Cargo.toml`); the root
      package's `[dev-dependencies]` entry (used by
      `examples/expert/sequential_actions_spike.rs`) now points at the
      same workspace version instead of a separate pin.
- [x] Registered `SequentialActionsPlugin` in
      `crates/xrds-runtime/src/xrds_api/install.rs`, alongside the other
      plugins already installed there. `cargo check -p xrds-runtime`
      clean.

## Phase 1 — core authored-data types

- [x] Added `XrdsAction`, `XrdsActionTarget`, `XrdsActionValue`,
      `XrdsSequence`, `XrdsTriggerKind`, `XrdsTriggerBinding` to
      `crates/xrds-scene-graph/src/scene/trigger_action.rs`, wired into
      `scene/mod.rs` alongside the other payload modules.
      **Schema change from the design doc's sketch:** `PlayGltfAnimation`
      uses the crate's existing `XrdsSceneGltfPlayback` (already bundles
      selector + repeat + speed + start_paused for glTF node authoring,
      `scene/assets/gltf.rs`) as a single `playback` field, instead of
      inventing separate mirror types — the design doc's draft predated
      discovering this type already existed; no new
      `XrdsSceneAnimationSelector`/`...PlaybackOptions` types were added.
- [x] Serde derives match existing conventions (`#[serde(default)]` on
      every field for additive-schema safety, `Default` impls following
      the established `#[default]`-variant-marker style except where a
      data-carrying variant requires a manual `impl Default`
      — `XrdsActionValue::Fixed(0.0)`, since `#[default]` on a
      derive only supports unit variants).
      Round-trip test covering all 7 v1 `XrdsAction` variants in one
      `XrdsTriggerBinding`: `crates/xrds-scene-graph/src/tests/trigger_action.rs`.
- [x] Additive-schema check: `trigger_binding_minimal_json_uses_defaults_and_deserializes_from_empty_object`
      deserializes `XrdsTriggerBinding` from a bare `"{}"` and confirms
      every field falls back to its default — the "older saved scene has
      no `triggers` data" case is covered here, not left as an assumption.
      All 4 new tests pass; full `xrds-scene-graph` suite (73 tests) and
      `cargo check --workspace` both still clean.

## Phase 2 — storage on the scene graph

**Corrected from the original plan** — the first draft of this phase put
`triggers` inside `XrdsInteractionZone`'s payload. That's wrong: it would
make trigger-action data depend on a node having a zone, which directly
contradicts the "Trigger is an open/pluggable source" decision — the
bullet-hits-player case needs the *player* entity (a plain physics body,
no zone at all) to carry trigger bindings. Caught before any code was
written against the old shape.

- [x] Added `pub triggers: Vec<XrdsTriggerBinding>` as a top-level field on
      `XrdsSceneNode` (`crates/xrds-scene-graph/src/scene/node.rs`), next to
      `grabbable`, with `#[serde(default, skip_serializing_if =
      "Vec::is_empty")]`. `XrdsInteractionZone` itself is untouched.
      **Blast radius:** this required adding the field to 80 existing
      `XrdsSceneNode { .. }` struct literals across 10 files (test fixtures
      mostly) — done via a scripted pass that used the compiler's own
      E0063 error spans to insert `triggers: Vec::new(),` at the exact
      right point in each literal, then iterated `cargo check` to
      convergence. Verified zero behavior change: all pre-existing
      `xrds-scene-graph` tests (73) still pass unmodified in meaning.
- [x] `XrdsTriggerBindings` runtime component added
      (`crates/xrds-runtime/src/xrds_api/trigger_action.rs`) — holds
      `Vec<xrds_scene_graph::XrdsTriggerBinding>` directly (no mirror type
      needed; `xrds-runtime` already depends on `xrds-scene-graph`).
- [x] Import wiring: `tag_trigger_binding_entities`
      (`crates/xrds-runtime/src/xrds_api/reimport.rs`), mirroring
      `tag_grabbable_entities` exactly — inserts/removes
      `XrdsTriggerBindings` per node's `triggers` field, called from
      **both** existing import call sites
      (`reimport_scene_in_world` and `XrdsAPI::import_scene_document`) —
      this codebase has hit the "only wired one of the two import paths"
      bug before (Player/PlayerAnchor tagging), so both are covered here
      from the start. **Spawns the definition only —
      nothing is enqueued onto a `bevy-sequential-actions` queue here**,
      per the document/runtime separation rule.
- [x] Export wiring (not in the original plan, but required for the
      round-trip test to mean anything): `export_scene_node_in_world`'s
      caller in `helper.rs` now reads `XrdsTriggerBindings` back off the
      entity into `node.triggers`, mirroring exactly how `XrGrabbable` is
      read back into `node.grabbable` a few lines above it.
- [x] Test: `trigger_bindings_survive_import_export_round_trip`
      (`crates/xrds-runtime/src/tests/document_roundtrip.rs`) — imports a
      node with a `ZoneEnter` → `Teleport` binding, asserts
      `XrdsTriggerBindings` exists on the live entity post-import, then
      asserts the exported document's `triggers` field matches exactly.
      **Passing.** Uses an `Empty` payload deliberately (triggers are
      payload-agnostic by design), which also sidesteps a separate
      pre-existing gap: `export_scene_node_in_world` in `helper.rs` has
      no case for `XrdsInteractionZone`, so InteractionZone nodes aren't
      round-trippable through export at all. **Logged as a real
      pre-existing bug, not fixed here** (unrelated to trigger-action).
- [x] **Fixed two pre-existing headless-test-harness gaps** found while
      verifying the above. These were breaking 57 of 73 `xrds-runtime`
      tests before any of this work — confirmed via `git stash` that they
      reproduce identically on the untouched baseline:
      1. `bevy_mod_outline::OutlinePlugin::build()` calls
         `.sub_app_mut(RenderApp)`, which requires Bevy's render sub-app
         (normally created by `bevy_render::RenderPlugin`). The minimal
         headless harness never adds it, so every test calling
         `XrdsAPI::attach` panicked. Fixed by gating the plugin behind
         `#[cfg(not(test))]` in `install.rs` — the same pattern already
         used there for `Text3dPlugin`/`FontMeshPlugin`, and safe because
         real apps go through `Runtime::build_bevy_app` with full
         `DefaultPlugins`.
      2. `avian3d`'s `init_collider_constructor_hierarchies` requires
         `Res<SceneSpawner>` (from `bevy_scene::ScenePlugin`), which
         `xrds_test_app()` was missing — while the two *other* test-app
         builders in the same file already added it for this exact
         reason. Fixed by adding `ScenePlugin` to `xrds_test_app()`.
      Result: **`xrds-runtime` 74/74 and `xrds-scene-graph` 73/73 pass**,
      up from 16/73 in `xrds-runtime` before this phase.

## Phase 3 — the pluggable trigger mechanism

All in `crates/xrds-runtime/src/xrds_api/trigger_action.rs`.

- [x] `XrdsTriggerEvent` trait, with `target()` / `source()` / `kind()`.
      `source()` defaults to `target()` so trigger sources with no
      separate cause don't have to think about it.
      **Deviation from the schema sketch:** methods return `XrdsId`, not
      `Entity` — the real `XrZoneEnterEvent`/`XrZoneExitEvent` carry
      `zone_id`/`entity_id` as `XrdsId`, so `consume_triggers` resolves
      them through `XrdsIdIndex`. The doc sketch predated checking the
      actual event shape.
- [x] Implemented for `XrZoneEnterEvent` (target = zone, source = whoever
      entered) and `XrZoneExitEvent`.
- [x] `XrdsTriggerValue(pub f32)` — the Option C payload slot, with a
      doc-comment stating plainly that gameplay code owns populating it
      and this layer only reads it.
- [x] Generic `consume_triggers<E>`, registered once per implementor in
      `install.rs`, explicitly `.after(zone_collision_system)` so a zone
      entered on frame N fires on frame N rather than relying on event
      double-buffering to hide a missing ordering constraint.
- [x] **Re-fire policy: decided and implemented — ephemeral agent per
      firing.** Surveyed Unity/Unreal/Godot: all three never suppress at
      the detection layer, and Blueprint's DoOnce/Gate exist precisely
      because suppression is opt-in. So each firing spawns its own
      short-lived agent entity carrying `(target, source)`, reaped by
      `despawn_finished_sequence_agents` once its queue drains. Two
      different sources firing the same trigger therefore run
      independently instead of one queueing behind the other — the case
      that ruled out the "ignore while running" default I'd originally
      proposed. Full rationale in the design doc.
- [x] **Opposite-trigger cancellation: decided — no.** `ZoneExit` does
      not cancel an in-flight `ZoneEnter` sequence; they're independent
      bindings on independent agents. Author-requested suppression stays
      a future explicit opt-in, not implicit behavior.

## Phase 4 — action execution

- [x] `XrdsActionRunner` implements `bevy-sequential-actions`' `Action`
      with one arm per v1 variant. Actions apply to the recorded
      `target`/`source` entities, never to the agent entity the queue
      lives on.
- [x] `spawn_sequence_agent` translates an `XrdsSequence` into
      `XrdsActionRunner`s and pushes them in one `.add(Vec<BoxedAction>)`
      call (sequential, per the spike's verified tuple/collection
      semantics). `XrdsActionTarget::Node(id)` resolves through the
      existing `XrdsIdIndex` — no second lookup built.
- [x] `ModifyHealth`'s `FromTriggerSource` reads `XrdsTriggerValue` off
      the source, degrading to `0.0` with a `warn!` when absent rather
      than panicking.
      **Scope note:** this needed somewhere to *put* health, so
      `XrdsHealth(pub f32)` was added as a plain data slot alongside
      `XrdsTriggerValue`. Consistent with the scope boundary — the SDK
      provides the slot and an action that changes it; reacting to it
      (death, respawn, UI) stays gameplay code's job.
- [x] Tests (`crates/xrds-runtime/src/tests/trigger_action.rs`, 6
      passing) — these are behavior tests, not serialization tests: they
      write a real trigger message and assert the effect on the world.
      Covers end-to-end `ZoneEnter` → `Teleport`; a `ZoneExit` binding
      correctly *not* firing on enter; `ModifyHealth` reading the
      source's `XrdsTriggerValue`; **two distinct sources each firing
      their own concurrent agent** (the re-fire decision, verified rather
      than assumed); ephemeral agents actually being reaped (a leak
      check); and `FireCustomEvent` emitting with the right target/source.
- [x] **"Play an animation, then do X" — solved, but via a third trigger
      source rather than a blocking action.** The investigation found the
      obvious approach unusable, and the intended signal itself broken:
      - `XrdsGltfAnimationState.playing` is **not** a live signal. Every
        writer of `ActiveGltfAnimationStates` was an imperative API call
        (play/stop/pause/resume); nothing observed the engine, so `playing`
        stayed `true` forever after a `Once` clip ended. That made
        `XrdsAPI::gltf_animation_state()` a **pre-existing public-API bug**
        — reporting finished animations as still playing. Nothing consumed
        it before, so nobody had hit it.
      - The trustworthy signal is Bevy's `ActiveAnimation::is_finished()`
        (reads a real `completions` counter).
      - `RepeatAnimation::Forever` makes `is_finished()` **always false**
        (`bevy_animation/src/lib.rs:530-536`), and XRDS's `Loop` maps to
        exactly that — so a blocking wait-for-completion on a looping clip
        would hang forever, *silently* (agent never drains, never reaped).
      - `AnimationPlayer::all_finished()` is `.all()` over the active set,
        and `.all()` on an **empty** set is `true` — so a still-loading
        asset would report "finished" instantly. The check must also
        require at least one active animation.

      Implemented: `sync_completed_gltf_animations_in_world` (`helper.rs`)
      is now the only writer driven by what the engine is actually doing —
      it corrects the cached `playing` flag (fixing the public-API bug) and
      reports completed roots. `sync_completed_gltf_animation_triggers`
      (`trigger_action.rs`) turns those into a new
      `XrdsGltfAnimationCompleteEvent`, which implements `XrdsTriggerEvent`
      as `XrdsTriggerKind::AnimationComplete`. Both run in `Last`; the
      trigger is consumed on the next frame's `Update` (one-frame latency,
      the only cost).

      **So no `wait_for_completion` field was added.** The follow-up goes
      in a second binding keyed on `AnimationComplete` — more idiomatic for
      an event-driven system, no schema field needed, and it makes the
      looping case a non-issue by construction (a `Loop` clip simply never
      fires the trigger) instead of something to guard against. This design
      fell out of the investigation rather than being the one originally
      planned; recorded here rather than silently substituted.

      **First real validation of the pluggable trigger design:** adding a
      third, entirely different trigger source cost one trait impl and one
      `consume_triggers::<E>` registration — no change to the data model,
      the consumer, or the sequencer.
      Tests: `completed_animation_clears_the_playing_flag` (the bug fix),
      `looping_animation_never_reports_completion` (the hang case, verified
      rather than reasoned about), `animation_complete_fires_as_an_authored_trigger`
      (end-to-end).
- [x] Made `xrds_api::trigger_action` a **public** module with key types
      re-exported. Found while wiring the above: it was `pub(crate)`, which
      made `XrdsAction::FireCustomEvent` **useless** — apps had no way to
      read the message it emits, so the expert-layer escape hatch didn't
      actually reach the expert layer. Same for gameplay code needing to
      insert `XrdsTriggerValue`/`XrdsHealth`.
- [x] **Despawn safety: verified, not assumed.** Two tests added:
      `trigger_targeting_an_already_despawned_entity_is_ignored` (stale
      `XrdsIdIndex` mapping for a dead entity → event skipped, no agent
      spawned) and `target_despawned_mid_sequence_does_not_panic` (target
      killed while the sequence sits in a `Wait`, then every remaining
      target-touching action — `SetVisible`, `Teleport`, `ModifyHealth`,
      `StopGltfAnimation` — runs against the dead entity). Both pass; the
      agent is still correctly reaped in the second case, so a despawned
      target doesn't leak its agent.

## Phase 5 — example + docs

- [x] `examples/xrds_first/trigger_action_sequence.rs` — **written against
      the default `XrdsApp`/`XrdsAPI` layer**, confirming a non-expert can
      author trigger-action sequences without touching `RuntimeHandler`.
      (`SequentialActionsPlugin` is registered inside `install_xrds`, so
      the example needed no plugin wiring at all — even easier than the
      `XrdsApp::configure` escape hatch this plan had anticipated.)
      One node, two bindings: `ZoneEnter` runs a four-step sequence
      (`SetVisible(false)` → `Wait 0.35s` → `SetVisible(true)` →
      `Teleport`) and `ZoneExit` runs a separate one-step return.
      **Visually confirmed by a human** running it: the cube blinks then
      moves right, and returns on the next tick — i.e. ordered
      multi-step sequencing and two independent bindings both working
      on-screen, not just in assertions.
      One honest caveat, documented in the example's own header: it
      synthesizes the zone messages from a timer system rather than
      driving them from a physics body entering a sensor volume, to keep
      what you're watching deterministic and physics-setup-free. Every
      other link in the chain (`consume_triggers`, the sequencer, the
      actions) is the real production path; the physics→event half
      (`zone_collision_system`) is pre-existing and untouched here.
- [x] Documented in `ARCHITECTURE.md` (new "Trigger-Action Sequencing"
      section) rather than `MANUAL.md` — there is no root `MANUAL.md`;
      `ARCHITECTURE.md` is where the document/authoring layer and the
      scene-node model are already described. Covers the two-system
      split, the top-level `triggers` field and why it is not nested in
      `XrdsInteractionZone`, an authoring snippet, the closed-vocabulary
      rule, the scope boundary, and `FromTriggerSource`.
- [ ] Move this plan and the design doc to `docs/done/` once Phases 0-5
      are complete and verified — Phase 6 (editor) is intentionally
      excluded from that gate, tracked separately. Phases 0-5 are now all
      complete, so this is ready to move once Phase 6 is either scheduled
      or explicitly deferred.

## Phase 7 — completing the trigger-source surface

Added after the initial commit (`8e02bce`). Prompted by the question of how
to handle the many entity states one might want to watch — grabbed,
visible, rotated-by-how-much — without the trigger vocabulary exploding.

**The resolution: split discrete from continuous.**

- *Discrete* state changes ("this happened at this moment") fit the
  existing model, and eight such events already existed in the codebase,
  registered but unconsumed. Wiring each cost ~5 lines plus one
  registration.
- *Continuous* values (rotation angle, position, scale) have no natural
  moment and no SDK-knowable threshold — 45° matters for a valve puzzle and
  is meaningless for a spinning fan. No mainstream engine models these
  declaratively either. **Deliberately not modeled as trigger kinds.**

- [x] Wired all eight remaining existing events as trigger sources:
      `Grabbed`/`Dropped` (the canonical XR interaction pair),
      `HoverEnter`/`HoverExit`, `ButtonPress`/`ButtonRelease`,
      `SliderChange`, `ToggleChange`. Wiring all eight rather than a
      need-driven subset: leaving most of an SDK's interaction surface
      unbindable would be an arbitrary gap ("why can I trigger on grab but
      not hover?"), and inconsistency in a pick-from-a-list vocabulary is
      its own cost.
- [x] **Required a trait refactor, discovered while implementing:** the
      world-UI button/slider/toggle events carry a raw Bevy `Entity`, while
      zone/grab/hover events carry an `XrdsId`. `XrdsTriggerEvent`'s
      methods returned `XrdsId`, so **three of the eight could not
      implement the trait at all.** Introduced `XrdsTriggerRef` (`Id` |
      `Entity`) with a `resolve(&XrdsIdIndex)` method; events report
      whichever they have and `consume_triggers` normalizes. Chosen over
      changing the event types themselves, which already have other
      consumers.
- [x] Added `XrdsTriggerKind::Custom(String)` — the inbound counterpart to
      `XrdsAction::FireCustomEvent`. Without it, app code could implement
      `XrdsTriggerEvent` but had to return an existing SDK variant from
      `kind()`, so **no new trigger kind could be introduced without
      editing the SDK.** Sequences could call out to app code but app code
      couldn't call in. This is also the mechanism for continuous state:
      gameplay watches the value and fires a `Custom` trigger when its own
      threshold is crossed.
      Trade-off accepted knowingly: string-matched, so a typo silently
      never fires. It stays a *name*, not a query path or expression, so it
      cannot grow into a scripting surface. Worth an editor picker rather
      than free text in Phase 6.
      Note: this drops `Copy` from `XrdsTriggerKind` (String payload);
      nothing depended on it.
- [x] Tests: `grab_event_fires_its_authored_sequence` (the `Id` ref path),
      `button_press_fires_via_the_entity_ref_path` (the `Entity` ref path —
      the reason `XrdsTriggerRef` exists),
      `app_defined_custom_trigger_fires_without_any_sdk_change` (a message
      type defined outside the SDK driving a sequence end to end), and
      `custom_trigger_with_a_different_name_does_not_fire` (the flip side
      of string matching).
- [ ] **Known limitation:** `SliderChange`/`ToggleChange` fire correctly,
      but the event's *value* (`slider.value`, `toggle.checked`) is not
      reachable from a sequence — `XrdsActionValue::FromTriggerSource`
      reads the `XrdsTriggerValue` component off the source entity, not the
      event payload. Deliberately not "fixed" by having
      `consume_triggers` write `XrdsTriggerValue`: that slot is documented
      as gameplay-owned, so clobbering it as a side effect would break
      that contract. Needs its own small design pass if a real use case
      appears.

**Deliberately not built:** any generic property-watcher, reflection-based
path, or threshold-expression mechanism for continuous state. That is the
Blueprint slide, and it is the one thing this whole design exists to avoid.

---

## Phase 8 — threshold watchers (continuous to discrete)

**Status: done.** `xrds-runtime` 105/105, `xrds-scene-graph` 88/88,
`cargo check --workspace --all-targets` clean.

Phase 7 (see [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md))
deliberately excluded continuous values (rotation angle, position,
scale) from the trigger vocabulary, on the grounds that they have no natural
"moment" and no SDK-knowable threshold. That reasoning still holds for
*general* property watching — but it over-corrected. A **closed set of
observables** with a threshold is a much narrower thing than the
reflection-path-plus-expression-predicate mechanism that was being ruled
out.

Precedent: animation state machines already do exactly this. Unity's
Animator transitions compare **declared parameters** against thresholds
(`Speed > 0.5`); Unreal's AnimBP is similar. Those parameters are a
declared, typed list — not arbitrary property paths. That is the shape that
works.

**Why this is worth centralizing — it is not about reading the value.** The
value read is trivial. The fiddly parts are:

1. **Edge detection.** A threshold must fire on *crossing*, not
   while-above, or it fires every frame. That needs stored previous state
   per watcher.
2. **Hysteresis.** A value hovering at the threshold will chatter without a
   deadband.

Both are easy to get subtly wrong, and every project would otherwise
reimplement them. Writing this as **one** system rather than one per
quantity means that logic exists in exactly one place.

Proposed shape — closed enum plus one `match`, the same design language as
`XrdsAction`:

```rust
pub enum XrdsAxis { X, Y, Z }

pub enum XrdsObservable {
    RotationDegrees { axis: XrdsAxis },
    DistanceTo { node: XrdsSceneNodeId },
    Height,            // translation.y
    ScaleMagnitude,
}

pub enum XrdsCrossing { Above, Below, Either }

pub struct XrdsThresholdWatcher {
    pub observable: XrdsObservable,
    pub crossing: XrdsCrossing,
    pub value: f32,
    /// Deadband, to stop chatter at the boundary.
    pub hysteresis: f32,
    /// Fired as `XrdsTriggerKind::Custom(fires)` on each crossing.
    pub fires: String,
}
```

The full loop stays data-driven with no code: watcher crosses, fires
`Custom("valve_opened")`, and an existing `XrdsTriggerBinding` on
`Custom("valve_opened")` runs. Keeping that two-step indirection (rather
than letting a watcher point at a sequence directly) avoids duplicating the
binding mechanism, and lets one watcher drive several bindings.

- [x] Added the types (`XrdsAxis`, `XrdsObservable`, `XrdsCrossing`,
      `XrdsThresholdWatcher`) to `xrds-scene-graph`, serde-default, as a
      `watchers: Vec<XrdsThresholdWatcher>` field on `XrdsSceneNode`
      alongside `triggers`. Adjacently-tagged like `XrdsAction`/
      `XrdsTriggerKind` were made in Phase 10, for the same forward-compat
      reason — not needed by anything in `XrdsObservable` today, but
      consistent rather than reintroducing the externally-tagged trap.
      **Deviation from the sketch:** all observables read world space (via
      `GlobalTransform`), not local — "has this rotated past 90°" almost
      always means in the world, not relative to a possibly-rotating
      parent. `RotationDegrees` uses Euler XYZ decomposition (documented
      gimbal-lock caveat); `ScaleMagnitude` is `scale.length()` rather than
      per-axis, so "grown past 2x" has one unambiguous number.
- [x] One runtime system, `evaluate_threshold_watchers` — reads the
      observable, compares against threshold with a sticky hysteresis band
      (not just a wider single check: once `Above`, must fall below
      `value - hysteresis` to become `Below`, and vice versa — this is what
      actually stops chatter at the boundary rather than just moving it),
      fires `XrdsThresholdCrossedEvent` on a qualifying crossing.
      Previous-state lives in `XrdsThresholdWatcherState`, a runtime-only
      component never touched by import/export/reimport.
      `XrdsThresholdCrossedEvent` implements `XrdsTriggerEvent` as
      `Custom(fires)`, so a crossing reuses the exact same `consume_triggers`
      path as any other trigger source — no special-casing.
      Runs in `Last`, after `TransformSystems::Propagate`, alongside the
      existing glTF-animation-trigger sync — same one-frame-latency
      trade-off, for the same reason.
- [x] **A real, load-bearing test-harness gap found and fixed while writing
      tests, not guessed at:** a probe test proved `GlobalTransform` was not
      propagating in `xrds_test_app()` at all (stayed at identity after
      moving an entity via `Transform`) — the harness was missing
      `TransformPlugin`, the same class of gap as the `OutlinePlugin`/
      `ScenePlugin` fixes in Phase 10 (a real app always has this via
      `DefaultPlugins`; this minimal harness did not). Fixed by adding it,
      guarded by `is_plugin_added`. The alternative — reading local
      `Transform` instead of `GlobalTransform` to dodge the gap — was
      rejected because it would have silently broken correctness under any
      parenting, exactly what reading world space was chosen to avoid.
- [x] Tests (`crates/xrds-runtime/src/tests/trigger_action.rs`, 6 new):
      a crossing fires the bound `Custom` sequence; an `Above`-only watcher
      stays silent on the downward crossing; wobbling inside the hysteresis
      band fires nothing, clearing it fires exactly once; `Either` re-arms
      and fires on both directions; `DistanceTo` correctly uses world-space
      positions between two different nodes; a `disabled` watcher never
      evaluates regardless of how far the value moves.
- [x] Diagnostics (`crates/xrds-scene-graph/src/scene/trigger_action.rs`):
      a `DistanceTo` targeting a node that does not exist is an `Error`
      (mirrors the existing `ModifyHealth` dangling-target check); negative
      hysteresis is a `Warning` (silently clamped to `0.0` at runtime, so
      authoring it is never dangerous, just pointless); a watcher's `fires`
      name with no listener is `Info`. A watcher's `fires` name also now
      counts as a legitimate `Custom` emitter in the existing "nothing
      fires this" check — a binding listening for what only a watcher (not
      a `FireCustomEvent` action) emits must not be flagged as dangling.
      All of the above correctly stay silent while a watcher is `disabled`.
- [x] **Explicitly out of scope, unchanged from the plan:** arbitrary
      expressions (`a > b && c < d`), property paths, math over observables.
      Anything needing those drops to gameplay code and fires its own
      `Custom` trigger — already works, remains the escape hatch.

### Phase 8 decisions (settled during implementation)

- Crossings re-arm automatically — edge-triggered on every crossing, as
  planned. A `once` flag remains deferred until something asks for it.
- `Health` stays out of the observable list, as planned — still just a data
  slot, still not pulling gameplay semantics into the SDK.
- Hysteresis is `.max(0.0)` at read time rather than rejected at author
  time, so a negative value degrades to "no hysteresis" instead of being a
  hard error — consistent with this project's general stance that
  malformed/nonsensical authored data should degrade, not crash or block
  loading. The diagnostic (`Warning`, not `Error`) is what actually
  surfaces it to an author.

## Phase 10 — robustness pass (done, before Phase 8)

A review of the shipped system surfaced seven gaps. Fixed here, ranked by
likelihood of biting.

- [x] **Forward compatibility — was a whole-document data-loss risk.**
      `XrdsAction`/`XrdsTriggerKind` were externally-tagged serde enums with
      no unknown-variant handling, so a scene written by a newer editor
      (e.g. containing a backlog `PlayAudio` action) would fail to
      deserialize on an older runtime — and because actions are nested
      inside the document, **the entire scene** would fail to load, not just
      that step. Realistic here: scenes get pushed to a Quest APK that can
      lag the editor.
      Fixed with an `Unknown` fallback on both enums. This required
      switching them to **adjacent tagging** (`{"kind": …, "data": …}`),
      because `#[serde(other)]` is only allowed on internally- or
      adjacently-tagged enums, and internal tagging cannot represent the
      newtype variant `SetVisible(bool)`. An unrecognized action is now a
      logged no-op; the rest of the sequence and the whole document survive.
      **Known limitation, deliberate:** lossy. `#[serde(other)]` requires a
      unit variant, so the original payload is not retained — an older build
      that loads and re-exports a document drops what it did not understand.
      Total parse failure is the worse harm. Lossless needs a hand-written
      `Deserialize`; tracked, not done.
      Test: `unknown_action_variant_does_not_destroy_the_whole_document`
      builds a real document, renames one action tag, and asserts the node
      and the *following* action both survive.
- [x] **Authoring diagnostics — `XrdsSceneDocument::trigger_diagnostics()`.**
      Catches the failure modes that are otherwise **silent at runtime**.
      The worst is a `Custom` trigger whose name nothing emits: "never
      fires" is indistinguishable from "not triggered yet", so there is
      nothing to debug against. Also flags dangling
      `XrdsActionTarget::Node(id)` (Error — genuinely unworkable), glTF
      animation actions on non-glTF nodes, `FireCustomEvent` with no
      listener, unrecognized actions/kinds, and empty sequences.
      Shaped like the existing `XrdsSceneAssetDiagnosticEntry`
      (subject/severity/title/detail) so an editor can render asset and
      trigger diagnostics in one list. **This is also the intended home for
      Phase 9a's named-runnable reference checks.**
- [x] **`XrdsAPI::fire_trigger(node, kind)`** — runs a node's matching
      bindings without waiting for the real event, returning how many
      sequences started so a caller can distinguish "nothing bound" from
      "ran". For an editor preview button and for application tests, where
      staging a real zone collision or button press is impractical.
- [x] **`stop_sequences_on(node)` / `stop_all_sequences()`** — the manual
      half of the runaway escape hatch, and independently useful for
      aborting a cutscene or tearing down before a scene transition.
      Verified to cancel mid-`Wait` without running the following step and
      without leaking the agent.
- [x] **Pause/time-scale behavior — verified, previously only assumed.**
      `Wait` reads `Res<Time>`, which is `Time<Virtual>`, so pausing the app
      should pause a wait mid-sequence. `wait_respects_paused_virtual_time`
      now proves it: the wait does not elapse while paused, and the sequence
      resumes on unpause. Exactly the kind of thing that stays silently
      broken until someone pauses during a cutscene.
- [x] **Hand information — resolved.** Added `hand()` to `XrdsTriggerEvent`
      (default `None`), implemented on all 8 events that actually carry a
      controller (`Grabbed`/`Dropped`/`HoverEnter`/`HoverExit`/
      `ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange`), plus an
      optional `hand: Option<XrGrabHand>` filter on `XrdsTriggerBinding`.
      `None` (default) matches any hand — existing bindings are unaffected.
      `XrGrabHand` gained `Serialize`/`Deserialize` (it had neither before).
      Applied everywhere a binding is matched — `consume_triggers` **and**
      `fire_trigger_in_world`/`XrdsAPI::fire_trigger` — so an editor preview
      can't misrepresent what actually fires. **Diagnostics catch the
      resulting footgun:** a hand filter on a trigger kind that never
      reports one (`ZoneEnter`, `AnimationComplete`, `Custom`, …) makes that
      binding permanently, silently unfireable — flagged as an `Error` in
      `trigger_diagnostics()`.
- [ ] **Multiplayer authority — recorded, deferred.** In a networked scene,
      if a zone trigger fires on one client, does the sequence run
      everywhere? Every client simulating the same trigger locally means
      divergent state. This is broader than the backlog's
      `SendNetworkMessage` note and interacts with the existing `xrds-net`
      work. Not a v1 blocker, but it should be a written-down open question
      rather than a later surprise.

### `enabled` semantics and the template/instance distinction

Raised while reviewing whether a disabled node should still fire triggers.
Checked first: **`XrdsSceneNode.enabled` has essentially no runtime consumer
today** — the only hit anywhere is an unrelated world-UI surface flag. It is
currently decorative, so trigger-action would be its first real consumer and
there is no precedent to follow.

Clarified intent: `enabled` is meant to express **whether the node is
instantiated into the scene at all**, not a runtime active/inactive toggle.
The term is doing a poor job of conveying that and should be made clearer.

That reframing resolves the trigger question without any special-casing: if
`enabled: false` means the node is never spawned, it has no
`XrdsTriggerBindings` component, so its triggers cannot fire. Correct by
construction. **But honoring `enabled` is a change affecting every node
type, not just triggers, so it is out of trigger-action scope** and wants
its own pass — including whether a separate runtime active/inactive concept
is also needed (Unity distinguishes these; XRDS currently has neither).

**Done within trigger-action scope:** a per-binding flag so an author can
switch one rule off without deleting it — for isolating which of several
bindings misbehaves, or parking a rule before it is ready.

Named `disabled: bool` (default `false`), **not** `enabled`, for two
reasons. First, `XrdsSceneNode::enabled` already exists and means something
different (instantiation), so a second `enabled` with different semantics
nested inside it would compound exactly the confusion this section is
about. Second, the negative form makes plain `#[serde(default)]` correct:
serde's bool default is `false`, so an `enabled` field would have silently
switched off every binding in every existing document on load. Follows the
same serde shape as `XrdsSceneNode::grabbable`
(`skip_serializing_if = "std::ops::Not::not"`).

Applied in three places, not one: `consume_triggers` skips disabled
bindings, `fire_trigger_in_world` skips them too (an editor preview that
ran parked rules would misrepresent runtime), and `trigger_diagnostics`
skips them — a deliberately inert binding should not generate warnings the
author has to dismiss on every pass, and anything genuinely wrong
resurfaces the moment it is re-enabled. The diagnostics pre-pass that
collects custom-event names also skips them, so a *parked* emitter cannot
suppress the "nothing fires this" warning on a live listener.

**Template vs instance.** The trigger-action data is a *template* — "under
{X} do {Y}" — and the instantiated ones should be listable for modification.
The Phase 9a registry design already provides exactly this split, which is
worth making explicit:

- a **named runnable** in the document-level registry is the *template*
  ("do Y"),
- a **binding on a node** is an *instance* ("under X, on this node, do Y"),
- so an editor view that enumerates bindings across all nodes **is** the
  instance list.

Consequences that fall out for free: editing a template affects every
instance, while editing an instance (its trigger kind, its `enabled` flag,
which template it points at) affects only that one. No extra schema is
needed for this — it is a Phase 6 editor view over Phase 9a data.

## Terminology: "sequence" vs "timeline"

Worth pinning down, because it caused a genuine misunderstanding mid-build.
What shipped in Phases 0-7 is an **ordered queue**, which these docs call a
"sequence". That is *not* the same thing as a timeline (Phase 9), and the
difference is not cosmetic:

| | `XrdsSequence` | `XrdsTimeline` |
| --- | --- | --- |
| When does step N+1 run? | when step N reports finished | at its own authored timestamp |
| Timing model | relative, implicit | absolute, explicit |
| Concurrency | none — one action at a time per agent | yes — two keys can share a timestamp |
| Duration comes from | each action blocking | where the next key is placed |
| Substrate | `bevy-sequential-actions` queue | a clock + scheduler |

The practical consequence: in a queue, `[A, Wait 0.5, B]` places B at 0.5s
**only if A takes no time**. If A's duration changes, everything after it
drifts. On a timeline, `t=0.5` is `t=0.5` regardless of what else happens.

Both are legitimate and complementary — Unity ships Timeline *and*
coroutine-style sequencing for the same reason.

## Phase 9 — timeline-based composition

- [x] `XrdsTimelineKey { at_secs: f32, action: XrdsAction }`,
      `XrdsTimeline { keys, duration_secs: Option<f32>, looping: bool }` in
      `crates/xrds-scene-graph/src/scene/timeline.rs`, serde-default
      throughout. **A flat key list, not explicit tracks** — tracks are an
      editor-organization concept; two keys sharing a timestamp already
      expresses concurrency at runtime, so runtime tracks would be
      redundant structure.
- [x] Runtime scheduler: `XrdsTimelineAgent` component +
      `advance_timelines` system (`crates/xrds-runtime/src/xrds_api/trigger_action.rs`).
      Fires every key crossed within a frame step via a `while` loop, not an
      `if` — a long frame, or a `duration_secs` shorter than one frame,
      cannot silently drop a key. `duration_secs <= 0.0` fires every key
      immediately instead of hot-spinning at one key per frame forever.
      Looping wraps `elapsed_secs` and re-fires any key at/before the
      wrapped time immediately, so a key at `t=0` doesn't wait a full lap.
- [x] **Reuses `spawn_sequence_agent_with_depth` for the actual effects**,
      rather than a second action-execution path: each fired key becomes its
      own one-step `XrdsSequence` agent. One implementation of what each
      `XrdsAction` does, shared between queue and timeline, as planned.
      `XrdsAction::Wait` inside a key is meaningless (a key already carries
      its own `at_secs`) — skipped with a warning rather than silently
      stalling.
- [x] Tests (`crates/xrds-runtime/src/tests/trigger_action.rs`): a timeline
      started via `Run` fires its keys at their times and despawns once
      non-looping duration elapses; `stop_all_sequences`/`stop_sequences_on`
      also cancel in-flight timelines (extended to query `XrdsTimelineAgent`
      alongside `XrdsSequenceAgent`).
- [ ] **Not done, tracked as follow-up:** seeking/scrubbing (deliberately
      deferred — not needed for runtime playback, likely wanted for editor
      preview; the scheduler still fires purely as a function of `elapsed`,
      so this isn't foreclosed) and an exhaustive multi-frame-drop stress
      test beyond the coverage above.

## Phase 9a — interoperability, the runnable registry, and Run

**Mechanism, as designed: a document-level registry of named runnables,
referenced by name, not inline nesting** (avoids a recursive data structure —
an `XrdsAction` containing an `XrdsRunnable` containing `XrdsAction`s would
need boxing and serialize into deeply nested JSON).

- [x] `XrdsRunnable { Sequence(XrdsSequence) | Timeline(XrdsTimeline) }` and
      `XrdsNamedRunnable { name: String, runnable: XrdsRunnable }` in
      `timeline.rs`; `XrdsSceneDocument::runnables: Vec<XrdsNamedRunnable>`
      (document-level, not per-node).
- [x] `XrdsTriggerBinding` gained `runnable: Option<String>`, **additive**
      alongside the existing inline `sequence` field rather than retyping it
      into a `Named | Inline` enum — deliberately, to avoid an expensive
      rewrite of ~60 existing test literals for marginal type-safety gain.
      `Some(name)` resolves through the registry and takes priority;
      `None` falls back to the binding's own `sequence`.
- [x] `XrdsAction::Run { runnable: String, wait: bool }` (`wait` defaults
      `true`). Takes a **bare name only** — the recursion firewall, since a
      name cannot nest the way an inline runnable could.
      `wait: true` blocks the enclosing sequence until the started runnable
      finishes (natural for a sequence, since it's already
      completion-chained); `wait: true` on a *timeline* target is ignored
      with a warning instead (a timeline that paused would break the
      absolute timing that is its entire purpose) and it always runs
      fire-and-forget.
- [x] Runtime resolution: `XrdsRunnableRegistry` resource
      (`HashMap<String, XrdsRunnable>`), replaced wholesale on every full
      document import (`reimport::sync_runnable_registry`, called from both
      `XrdsAPI::import_scene_document` and the editor's
      `reimport_scene_in_world`) — matching how the rest of import treats
      the document as complete, authoritative state rather than something to
      merge into. `consume_triggers` and `fire_trigger_in_world` both
      resolve a binding's `runnable`/`sequence` through the same
      `spawn_binding_runnable` helper, so the two paths can't drift.
      An unresolvable name (unknown runnable, or a binding naming one that
      isn't registered) logs a warning and fires nothing, rather than
      panicking or stalling the rest of a sequence — same forward-compat
      posture as `XrdsAction::Unknown`.
- [x] **Runaway loops: causal chain depth, not a rate limit, with a
      guaranteed escape** — the user's explicit requirement going in: cycles
      may be *authored* (other engines permit intentional event loops too),
      but an escape must always exist so a runaway loop is never a
      mysterious hang.
      - `XrdsSequenceAgent`/`XrdsTimelineAgent`/`XrdsActionRunner` all carry
        `chain_depth: u32`. `Run` spawns its child at `chain_depth + 1`.
        A rate limit was explicitly rejected: it can't distinguish a real
        loop from legitimately high-frequency input (`SliderChange` fires
        every frame while dragging — correct behavior, not a runaway).
      - At the cap (`MAX_RUN_CHAIN_DEPTH = 64`), the `Run` action stops
        spawning and fires `XrdsTriggerKind::RunawayDetected` on the node —
        an *authorable* escape trigger (via `fire_runaway_detected_in_world`),
        not just a log line, so a recovery sequence can run.
      - **Recovery-path protection, verified, not just documented:** agents
        spawned from a `RunawayDetected` firing carry `is_recovery: true`,
        propagated through every descendant `Run`. If a recovery chain
        itself hits the depth cap, it is dropped with a hard `log::error!`
        and does **not** re-fire `RunawayDetected` — the breaker can never
        recurse through its own recovery path. Without this flag, a
        `RunawayDetected` binding that itself contained a looping `Run`
        would cycle forever in bursts of 64, exactly the "mysterious hang"
        the design commits to ruling out; caught and fixed before shipping,
        not left as a known gap.
      - `XrdsAPI::stop_sequences_on`/`stop_all_sequences` (already shipped
        in Phase 10) are the manual kill switch, extended in this phase to
        also cancel in-flight `XrdsTimelineAgent`s, not just
        `XrdsSequenceAgent`s.
      - **Known, stated coverage gap** (matches the design doc, not
        silently dropped): an app-defined trigger event cannot carry the
        depth stamp, so a loop routed entirely through application code is
        undetectable by this mechanism. Same position as other engines.
      - Tests: a self-referencing `Run` chain hits the cap and fires
        `RunawayDetected` (observed via a bound recovery action) instead of
        hanging the test process.
- [x] Migration: clean break, no legacy field — verified before deciding
      that no saved scene document on disk depended on the pre-registry
      binding shape.
- [x] **Static `Run` diagnostics — resolved.** Extended
      `XrdsSceneDocument::trigger_diagnostics()` (Phase 10) to catch these
      at author time rather than only degrading safely at runtime:
      - A binding's `runnable: Some(name)` naming an entry
        `XrdsSceneDocument::runnables` doesn't have — `Error`, attributed to
        that binding's node.
      - A binding with **both** `runnable: Some(_)` and a non-empty inline
        `sequence` — `Info`, since the named runnable silently wins at
        runtime and the inline steps are dead data (the "nonsensical
        both-set state" flagged as a risk when the additive field was
        designed).
      - An inline `Run` step (when `runnable` is `None`, so it actually
        executes) naming an unregistered runnable — `Error`.
      - **Registry-level, node-less** (`node_id: None` — a registry entry
        may be referenced by many nodes or none, so a problem in it isn't
        any one node's fault): a `Run` inside a registry entry itself
        targeting an unregistered name, and a static cycle in the
        registry's own `Run`-graph (`A runs B runs A`), found via DFS over
        every entry's `Run` targets. A cycle is **flagged, not rejected** —
        matches the runtime's "escape, don't prevent" stance — every member
        of the cycle gets its own diagnostic (not just one), and the detail
        message spells out the path (`"a" -> "b" -> "a"`).
      Tests: unknown-runnable-name binding (Error, attributed to its node),
      both-set binding (Info, and confirmed *not* also flagged as unknown
      since the name IS valid), a healthy named-runnable binding is quiet,
      an inline `Run` step naming an unknown runnable (Error), a two-entry
      registry cycle (both members reported, node-less), an unknown `Run`
      target inside a registry entry (node-less), and a healthy
      non-cyclic `Run` chain is quiet.
      **Schema note:** `XrdsSceneTriggerDiagnostic::node_id` changed from
      `XrdsSceneNodeId` to `Option<XrdsSceneNodeId>` to allow this — no
      external consumers existed at the time (Phase 6 hadn't landed yet),
      so this was a free change; all existing construction sites updated
      to `Some(node.id)`.

## Phase 6 — editor integration

Full implementation record (architecture decisions, per-stage build notes,
every follow-up bug found while testing it) lives in
[`xrds-trigger-action-editor-plan.md`](xrds-trigger-action-editor-plan.md) —
this is the condensed version.

- [x] **A property-panel + full-viewport-overlay UI in `xrds-editor`** for
      authoring the whole system: the document-level runnable registry
      (`TriggerActionLibraryPanel` sidebar list + `TriggerActionEditorOverlay`
      step/timeline-key editor, reusing the exact HUD-library and
      world-panel-widget patterns already in this codebase), and per-node
      `TriggersSection`/`WatchersSection` in the Inspector. List-based, not
      a node-graph, per the original "no Blueprint-shaped authoring
      surface" decision.
      **Architecture finding worth keeping:** this editor has no precedent
      for a second WebView or OS window, and a prior version of the editor
      that *did* run two windows was deliberately abandoned for the
      complexity and latency it cost. The "dedicated editing surface" need
      was met instead by the same full-viewport same-WebView-overlay
      pattern (`set_viewport_hole`) three other editor features already
      use — zero new architectural risk.
      **`TriggerActionEditorOverlay` serves both a registry runnable and a
      binding's inline sequence** through one `StepTarget` prop
      (`Runnable{name}` | `Binding{node_id, binding_index}`), resolving the
      question the original plan had deliberately left open.
- [x] **The two pickers Phase 10's review said this UI would need** — both
      built as planned: a `<select>` (not free text) for a binding's
      `runnable` reference, with the same dangling-reference warning
      `PlayerAnchorSection`'s HUD-template picker already establishes; and
      the per-node `TriggersSection` list *is* the instance list the
      template/instance split was designed around.
- [x] **Trigger preview — `XrdsUpdateContext::fire_trigger()`.** Found
      while testing: authoring a binding in the editor had no way to
      actually fire it — desktop editing generates no real
      `ZoneEnter`/`Grabbed`/etc event, and `XrdsAPI::fire_trigger()` (built
      in Phase 10 for exactly this) had never been wired into any UI.
      Added the `update()`-time counterpart plus a "▶ Fire" button on every
      binding row.
- [x] **Default trigger kind on a new binding is an explicit "none",
      not a silent `ZoneEnter`.** `XrdsTriggerKind` has no real None
      variant, so the editor (not the domain type's own `Default`) seeds a
      freshly added binding with `Unknown` instead — already means "never
      fires" at runtime, doubling as an inert placeholder. The `Unknown`
      diagnostic message was reworded to cover both real causes (not yet
      picked in the editor, or a kind from a newer editor build).
- [x] **Two pre-existing bugs found and fixed while testing this feature,
      neither introduced by it:**
      - Selecting a step immediately after adding it showed the empty-state
        hint instead of its field editor — `AddActionStep`/`AddTimelineKey`
        round-trip through Rust before the array actually grows, and the
        sync effect's dependency array didn't account for that.
      - Viewport click-selection picked the ground plane instead of the
        clicked object, but only during Play mode — the editor camera used
        for raycasting is deactivated (not removed) when Play mode starts,
        so its stale `Transform` no longer matched what the player-pawn
        camera was actually rendering. Fixed with the same
        `if state.is_playing { return; }` guard `orbit_camera_system`
        already uses for the identical reason.
      Also added: rename support directly in the overlay's title bar (the
      sidebar list had it; the overlay — where authoring actually happens —
      didn't).
- [ ] **Not done, tracked as a real gap, not silently dropped:** an
      editor-side "preview this sequence without a real trigger" mode
      exists (`fire_trigger`), but there is still no way to *watch* a
      runnable's timeline advance frame-by-frame in the overlay itself —
      firing it drives the live 3D viewport, which is the actual point,
      but the overlay's own step list does not highlight "the key that
      just fired." Minor; not blocking.

## Decision log

Carried over from the design doc's decisions, for a single place to see
what's settled vs. still open:

- Execution substrate: `bevy-sequential-actions` 0.14, confirmed by a
  working spike (not just docs).
- `Trigger` is an open/pluggable mechanism (a trait + generic consumer
  system per event type), not a hardcoded enum of sources.
- `Action` stays closed-vocabulary — that's the actual Blueprint/Verse
  avoidance guarantee.
- Dynamic parameters: Option C (a generic `XrdsTriggerValue(f32)` slot,
  populated by ordinary gameplay code) over hardcoded field-enums or
  reflection-based field paths.
- Storage: inline data, not a separate linked file (glTF-style external
  references are reserved for heavy binary assets, not small structured
  data like this) — **corrected once, during Phase 2:** the first version
  of this decision put `triggers` inside `XrdsInteractionZone`'s payload
  specifically; that was wrong, since it made trigger-action data depend
  on a node having a zone, contradicting the open/pluggable-trigger-source
  decision (a bullet-hits-player binding needs to live on the player, a
  plain physics body with no zone at all). Final answer: `triggers: Vec<XrdsTriggerBinding>`
  is a top-level field on `XrdsSceneNode` itself, alongside `grabbable: bool`.
- Priority: build this before other new SDK components.
- Non-goals, held for the whole system, not just v1: no scripting language,
  no visual node-graph editor, no codegen; no general branching/conditional
  logic inside `Action` (the expert-layer escape hatch, `FireCustomEvent`,
  is the answer to "I need an if"); no parallel execution via
  `bevy-sequential-actions` tuple-add (confirmed sequential-only by the
  spike — genuine concurrency is what the Phase 9 timeline scheduler is,
  built outside that crate for exactly this reason).
