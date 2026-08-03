# Trigger-action sequencing — v1 implementation record

**Status: done.** The completed record of what shipped, phase by phase, and
why each decision was made. Live at `xrds-runtime` 96/96,
`xrds-scene-graph` 79/79, `cargo check --workspace --all-targets` clean, and
the Phase 5 example visually confirmed by a human.

Companion docs:

- [`../xrds-scenegraph-trigger-action-sequencing.md`](../xrds-scenegraph-trigger-action-sequencing.md)
  — the design rationale (why the system is shaped this way).
- [`../xrds-trigger-action-implementation-plan.md`](../xrds-trigger-action-implementation-plan.md)
  — what is still *ahead* (Phases 6, 8, 9, 9a) plus open questions.
- [`../xrds-trigger-action-backlog.md`](../xrds-trigger-action-backlog.md)
  — candidate `XrdsAction` variants, unscheduled.

Phase numbers are kept stable because code comments reference them. They are
ordered here by phase number, which is also the order they were built —
except Phase 10, a robustness pass done after Phase 7 and deliberately
before Phase 8.

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
- [ ] **Hand information is discarded — still open, needs a decision.**
      `XrGrabEvent`, `XrWorldButtonPressEvent` and friends all carry
      `hand: XrGrabHand`, but the `XrdsTriggerEvent` impls drop it — only
      target/source/kind survive. So "grabbed with the left hand" is
      inexpressible even though the data is right there, which is a notable
      gap for an XR SDK. Proposal: an optional `hand()` on the trait
      (defaulting to `None`) plus an optional `hand` filter on
      `XrdsTriggerBinding`, so a binding can require a specific hand. Not
      implemented — it changes the authored schema, so it wants an explicit
      call.
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

