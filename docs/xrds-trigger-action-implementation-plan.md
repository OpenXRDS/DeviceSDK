# Trigger-action sequencing — implementation plan

Companion to [`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md)
(the design/decision doc — read that first for *why* each piece is
shaped the way it is) and [`xrds-trigger-action-backlog.md`](xrds-trigger-action-backlog.md)
(candidate `XrdsAction` variants beyond v1, explicitly not scheduled).
This doc is the *how and in what order* — phased, with checkboxes, in the
same style as `docs/done/xrds-net-release-readiness.md`.

**Status:** Phases 0-5 and 7 complete and verified — `xrds-runtime` 89/89,
`xrds-scene-graph` 73/73, `cargo check --workspace --all-targets` clean, and
the Phase 5 example visually confirmed by a human. No v1 gaps remain open.

Not started: **Phase 6** (editor UI, tracked but unscheduled),
**Phase 8** (threshold watchers — turning continuous values into discrete
triggers), **Phase 9** (timeline-based composition, a *different model* from
the ordered queue shipped in Phases 0-7 — read the terminology section
first), and **Phase 9a** (interoperability between the two models, which is
a requirement and shapes Phase 9's schema, so read it before starting 9).

## Priority

Per the priority call recorded in the design doc: this is being built
**before** other new SDK components, as foundational plumbing other
planned features will hook into. Nothing here is blocked on unrelated
work.

## Scope for v1

The seven `XrdsAction` variants already drafted (`PlayGltfAnimation`,
`StopGltfAnimation`, `SetVisible`, `Teleport`, `ModifyHealth`, `Wait`,
`FireCustomEvent`), three `XrdsTriggerKind`s (`ZoneEnter`, `ZoneExit`, and
`AnimationComplete` — added during Phase 4, see there for why), and
Option C for dynamic parameters (`XrdsTriggerValue(f32)` component, read
via `FromTriggerSource`). Everything in the backlog doc is explicitly
out of scope for v1 — pull items in only when a real use case needs them.

## Non-goals (carried over from the design doc)

- No scripting language, no visual node-graph editor, no codegen.
- No general branching/conditional logic inside `Action` — that's the
  expert-layer escape hatch's job (`FireCustomEvent` → real Bevy system).
- No parallel execution via `bevy-sequential-actions` tuple-add — already
  confirmed sequential-only by the spike; genuine concurrency needs
  separate agents/queues, not attempted in v1.
- No editor UI in this plan's scope (Phase 5 flags it as a distinct,
  likely-later milestone, not designed here).

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

## Phase 6 — editor integration (tracked, not designed here)

- [ ] `xrds-editor` property-panel UI for authoring `XrdsTriggerBinding`/
      `XrdsSequence` on interaction zones. **List-based** (add/remove/
      reorder steps, pick trigger kind and action from dropdowns) — not
      a node-graph, per the "no Blueprint-shaped authoring surface"
      decision in the design doc. Deliberately not scoped in detail
      here; revisit once Phases 0-5 land and the actual data shape is
      stable enough to design a UI against.

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
- Storage: inline on `XrdsInteractionZone`, not a separate linked file
  (glTF-style external references are reserved for heavy binary assets,
  not small structured data like this).
- Priority: build this before other new SDK components.

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

## Terminology: "sequence" vs "timeline"

Worth pinning down, because it caused a genuine misunderstanding mid-build.
What shipped in Phases 0-7 is an **ordered queue**, which this doc has been
calling a "sequence". That is *not* the same thing as a timeline, and the
difference is not cosmetic:

| | `XrdsSequence` (shipped) | `XrdsTimeline` (Phase 9, not built) |
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
coroutine-style sequencing for the same reason. An ordered queue is the
right model for "play this animation, then react when it actually ends"
(duration unknown at author time); a timeline is the right model for
choreography ("at 0.5s the door starts opening and the light dims").

**Nothing from Phases 0-7 is wasted by adding a timeline:** the trigger
layer and the entire `XrdsAction` vocabulary are shared. Only the
*execution strategy* differs.

## Phase 8 — threshold watchers (continuous to discrete)

**Status: planned, not started.**

Phase 7 deliberately excluded continuous values (rotation angle, position,
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

- [ ] Add the types above to `xrds-scene-graph`, serde-default, stored as a
      node field alongside `triggers`.
- [ ] One runtime system: read observable, compare against threshold with
      hysteresis, fire `Custom` on crossing. Previous-state storage lives in
      a runtime component, never in the authored document — same
      document/runtime split rule as everything else here.
- [ ] Tests: crossing fires exactly once per crossing (not per frame);
      hysteresis suppresses chatter; `Above`/`Below`/`Either` each behave;
      `DistanceTo` an unresolvable node degrades quietly.
- [ ] **Explicitly out of scope:** arbitrary expressions (`a > b && c < d`),
      property paths, math over observables. Anything needing those drops to
      gameplay code and fires its own `Custom` trigger — which already works
      and remains the escape hatch. This watcher is a convenience for the
      common case, not the only way in.

### Open decisions for Phase 8

- Does a crossing re-arm automatically? Leaning yes (edge-triggered on every
  crossing), with a `once` flag deferred until something asks for it.
- `Health` as an observable: deliberately omitted above. `XrdsHealth` exists
  only as a data slot, and adding it here starts pulling gameplay semantics
  into the SDK. Ship the transform-derived ones first; let a real use case
  argue for it.

## Phase 9 — timeline-based composition

**Status: planned, not started. Distinct from Phases 0-7, not a replacement
for them** — see the terminology section above.

Goal: author choreography against absolute time, with concurrency — "at
0.0s the door animation starts, at 0.5s a sound plays and the light dims, at
2.0s the zone re-enables."

Proposed shape:

```rust
pub struct XrdsTimelineKey {
    pub at_secs: f32,
    pub action: XrdsAction,
}

pub struct XrdsTimeline {
    /// Sorted by `at_secs`. Two keys may share a timestamp — that IS the
    /// concurrency mechanism.
    pub keys: Vec<XrdsTimelineKey>,
    /// Defaults to the last key's time when absent.
    pub duration_secs: Option<f32>,
    pub looping: bool,
}
```

**A flat key list, not explicit tracks.** Tracks are an *editor
organization* concept; at runtime, two keys sharing a timestamp already
express concurrency, so runtime tracks would be redundant structure. If the
editor wants lanes later, it can group by an editor-only tag without the
runtime knowing about it.

Execution is a scheduler, not a queue: a runtime component holds elapsed
time; each frame it advances and fires every key whose `at_secs` was crossed
during that step. **This largely bypasses `bevy-sequential-actions`** —
correctly so. In a timeline, actions are fire-and-forget at their timestamp;
duration is expressed by where the next key sits, not by an action blocking.

- [ ] Types in `xrds-scene-graph`, serde-default.
- [ ] Runtime scheduler component + system. Must fire keys crossed *within*
      a frame step, not keys exactly equal to `elapsed` — at 60fps a naive
      equality check would silently drop nearly every key.
- [ ] Reuse `XrdsActionRunner`'s per-action logic for the actual effects, so
      there is one implementation of what each `XrdsAction` does, shared
      between queue and timeline.
- [ ] Tests: keys fire once each, in order; two keys at the same timestamp
      both fire; a low frame rate does not drop keys; looping re-fires;
      stopping mid-timeline fires nothing further.

### Open decisions for Phase 9

- **How does a trigger start a timeline?** `XrdsTriggerBinding` currently
  holds `sequence: XrdsSequence`. Options: add
  `timeline: Option<XrdsTimeline>` alongside it (non-breaking for saved
  documents, but allows a nonsensical both-set state), or replace the field
  with an enum (cleaner, but a breaking schema change for any document
  already saved with `sequence`). Not yet decided — this is the main
  structural question for the phase.
- **`XrdsAction::Wait` inside a timeline is meaningless** — delay is
  expressed by placing the next key later. Reject at author time, warn at
  runtime, or silently ignore? Leaning warn-and-ignore.
- **Seeking/scrubbing.** Not needed for runtime playback; likely wanted for
  editor preview. Deliberately deferred, but the scheduler should not be
  written in a way that forecloses it — keep firing a function of `elapsed`,
  and avoid hidden incremental state beyond already-fired bookkeeping.

## Phase 9a — interoperability between the two models

**A requirement, not an extra.**

Interoperability was initially filed as "probably useful, definitely not
v1". That was wrong: it is a stated requirement. A timeline key must be able
to start an ordered sequence, and a sequence step must be able to start a
timeline. Treating it as a requirement rather than an afterthought changes
the schema, so it belongs in the design from the start.

**Mechanism: a document-level registry of named runnables, referenced by
name.** Not inline nesting.

```rust
pub enum XrdsRunnable {
    Sequence(XrdsSequence),
    Timeline(XrdsTimeline),
}

/// Lives on XrdsSceneDocument, not on a node.
pub struct XrdsNamedRunnable {
    pub name: String,
    pub runnable: XrdsRunnable,
}

/// New action variant — how either model starts the other.
XrdsAction::Run { runnable: String }
```

Why a registry rather than inline nesting:

- **It avoids a recursive data structure.** Inline nesting means an
  `XrdsAction` containing an `XrdsRunnable` containing `XrdsAction`s. That
  needs boxing, serializes into deeply nested JSON, and is unpleasant to
  author or diff.
- **It gives reuse for free**, which answers an earlier open question in
  the design doc about sharing one sequence across many nodes. The
  conclusion there was "inline unless reused" — a registry makes reuse the
  natural case without needing a separate external-file mechanism.
- **It is the editor-friendly shape**: a library of named sequences and
  timelines, with bindings and `Run` actions picking from it.

### Semantics to settle

- **Does a sequence step wait for what it starts?** The two models differ
  naturally and the asymmetry should be deliberate, not accidental:
  a *sequence* is completion-chained, so `Run` inside a sequence should
  probably block until the started runnable finishes. A *timeline* is
  fire-and-forget at each timestamp, so a `Run` key should start the
  runnable and immediately move on. Leaning toward exactly that — but it
  means `XrdsAction::Run` behaves differently depending on which executor
  it is in, which needs to be documented loudly or it will surprise people.
- **Cycles.** A registry makes `A runs B runs A` expressible. Needs either
  cycle detection at load time or a runtime depth cap. Not optional — this
  is the one way this design can hang the runtime, so it must be handled
  before `Run` ships.
- **Inline vs reference for bindings.** With a registry, does
  `XrdsTriggerBinding` still hold an inline `XrdsSequence`, or only a name?
  Supporting both is friendlier for one-offs but means two code paths.
  Leaning: registry reference as the primary model, keep inline as sugar
  for the single-use case, and make the runtime resolve both through one
  path.
- **Migration.** `XrdsTriggerBinding.sequence` already ships and may exist
  in saved documents. Whatever shape is chosen must either keep that field
  working or provide a load-time migration — this is pre-release, so a
  breaking change is permissible, but it should be a decision rather than
  an accident.
