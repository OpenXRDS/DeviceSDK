# Trigger-action sequencing — implementation plan

Companion to [`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md)
(the design/decision doc — read that first for *why* each piece is
shaped the way it is) and [`xrds-trigger-action-backlog.md`](xrds-trigger-action-backlog.md)
(candidate `XrdsAction` variants beyond v1, explicitly not scheduled).
This doc is the *how and in what order* — phased, with checkboxes, in the
same style as `docs/done/xrds-net-release-readiness.md`.

**Status:** Phases 0-5 complete and verified — `xrds-runtime` 85/85,
`xrds-scene-graph` 73/73, `cargo check --workspace` clean, and the Phase 5
example visually confirmed by a human. No v1 gaps remain open. Phase 6
(editor UI) is tracked but deliberately unscheduled.

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
