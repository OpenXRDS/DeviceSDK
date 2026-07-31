# Trigger-action sequencing for the scene graph — design track

## Context

Following a review of 3D-engine authoring trends (heavy actor-based
scripting like Unreal Blueprint/Verse being walked back in favor of
lighter, scene-graph-based composition), the goal is: let a scene author
wire "on trigger X, do action Y" and simple ordered sequences of actions
directly into `xrds-scene-graph` data — **without** building a scripting
language, a visual node-graph editor, or a codegen pipeline.

Prior investigation (see conversation, not yet its own doc) established:

- `xrds-scene-graph`'s `XrdsSceneNode`/`XrdsSceneNodePayload` are purely
  declarative data today — no script/behavior/sequence field anywhere.
- `XrdsInteractionZone` (`crates/xrds-components/src/interaction.rs`) +
  `XrZoneEnterEvent`/`XrZoneExitEvent` (emitted by `zone_collision_system`,
  `crates/xrds-runtime/src/xrds_api/zone.rs`) already give a **trigger
  detection** half — nothing currently consumes those events.
- glTF animation playback (`play_gltf_animation` etc. in
  `xrds-runtime/src/xrds_api/api.rs`) is imperative, Rust-caller-driven —
  intended to become one `Action` variant (e.g. `Action::PlayGltfAnimation`)
  once the trigger-action schema exists.
- glTF/glXF are asset-interchange formats only — no extension point for
  authored trigger/action/sequence data, and shoehorning it into `extras`
  would fight the format. This data belongs entirely in
  `xrds-scene-graph`, not in any glTF-derived asset.

**Decision going in:** design the schema (`Trigger`/`Action`/`Sequence`
types) and its authoring/runtime wiring ourselves — this is XRDS-specific
domain data no crate can provide. But evaluate whether the *execution*
mechanics of running an ordered action queue on an entity (progress
tracking, cancel, pause/resume, chaining) can be built on an existing,
compatible Bevy crate instead of hand-rolled, since that part is generic
engine plumbing, not domain-specific.

## Evaluation: `bevy-sequential-actions`

Candidate: [`bevy-sequential-actions`](https://github.com/hikikones/bevy-sequential-actions)
(crates.io name has an underscore variant, `bevy_sequential_actions`, same
crate).

### Bevy-version compatibility

| crate version | Bevy version |
| --- | --- |
| 0.16 | 0.19 |
| 0.15 | 0.18 |
| **0.14** | **0.17** ✅ our workspace pins `bevy = "0.17.2"` |
| 0.13 | 0.16 |
| 0.12 | 0.15 |
| ... | ... (tracks back to Bevy 0.7) |

The crate is actively kept in step with Bevy releases — it already
supports Bevy 0.19 (two majors ahead of what we're on), which is a decent
maintenance signal on its own. Confirms the version pairing mentioned
verbally before this evaluation: **0.14 is the correct pin for Bevy
0.17.2.**

### Maintenance signal (GitHub)

- 53 stars, 0 open issues, 119 commits on `main`.
- Single maintainer (`hikikones`). No CI/release-cadence detail was
  visible from the page fetch alone — worth a closer look (actual commit
  dates, `Cargo.toml` diffs across the last few version bumps) before
  depending on it, but nothing found suggests it's abandoned; it's
  current as of Bevy 0.19.
- Dual-licensed MIT / Apache-2.0 — compatible with this workspace's
  existing `license = "Apache-2.0"` convention.

### API shape

- **`Action` trait** — the core abstraction. Lifecycle methods:
  `is_finished(&self, agent: Entity, world: &World) -> bool`,
  `on_start(&mut self, agent: Entity, world: &mut World) -> bool`,
  `on_stop(&mut self, agent: Option<Entity>, world: &mut World, reason: StopReason)`,
  plus optional `on_add`/`on_remove`/`on_drop`. No derive macro required —
  implement the trait directly.
- **Agent** — "an entity with actions" is the unit sequences run on. Maps
  naturally onto our scene-node entities.
- **`ManageActions`/`actions(agent)`** — `Commands`/`World` extension for
  queue manipulation: `.add(action)`, tuple-based `.add((a, b))` for
  multiple at once, skip/clear/next, pause/resume (surfaces as
  `StopReason::Paused` in `on_stop`).
- **Execution mode: sequential only — confirmed by running code, not just
  docs.** `examples/expert/sequential_actions_spike.rs` queues
  `PrintAction("queue started")`, then a tuple `(TimedAction::new("A",
  2.0), TimedAction::new("B", 2.0))`, then a final print+exit. Actual
  output:

  ```text
  [t=0.00s] queue started
  [t=0.00s] TimedAction(A) started, 2s duration
  [t=2.01s] TimedAction(B) started, 2s duration
  [t=4.01s] after the tuple — t=~2s means concurrent, t=~4s means sequential
  ```

  `B` only starts once `A` finishes, and the final message lands at
  ~4s, not ~2s. **Tuple `.add((a, b))` just queues both one after
  another** — it is not a parallel-execution primitive. Any genuinely
  concurrent trigger effect (e.g. play an animation AND move an object at
  once, both fired by the same event) will need two independent
  queues/agents, not a tuple add on one queue.
- No built-in trigger/condition concept — this crate is purely the
  *action queue* half. The `Trigger` side (zone enter/exit, timers,
  custom events) and the authored `Sequence` schema remain entirely ours
  to design, as expected going in.

### Fit assessment

Good fit for the narrow thing it's meant to replace: an entity-scoped,
ordered action queue with start/stop/finish lifecycle hooks and
pause/resume — exactly the "run these steps in order, track progress,
allow cancel" plumbing that would otherwise need to be hand-rolled. Using
it would mean our own `Action` enum variants become thin adapters that
implement this crate's `Action` trait (e.g. `PlayGltfAnimationAction`
wrapping a call to `play_gltf_animation`), and our system just translates
"trigger fired → look up this node's `Sequence` data → enqueue the
matching actions via `actions(agent).add(...)`."

Two things to verify with actual code (not just docs) before committing:

1. Whether the "parallel" tuple-add pattern gives us anything, or whether
   any genuinely-parallel-action need (e.g. play an animation AND move
   an object at the same time, both trigger by the same event) would
   have to be modeled as two independent action queues/agents instead.
2. Whether `is_finished`/`on_start` running against `&World`/`&mut World`
   directly (rather than a query/system-param based API) has any friction
   integrating with how `XrdsAPI` currently expects to be called (most of
   `XrdsAPI`'s methods take `&mut XrdsAPI<'_>`, not a raw `&mut World`).

### Recommendation

**Adopt `bevy-sequential-actions` 0.14 as the execution substrate**,
pending the two verification items above (a small spike, not a big
effort — a few actions run through it against this workspace's actual
`XrdsAPI` surface). Design our own `Trigger`/`Action`/`Sequence` schema in
`xrds-scene-graph` on top of it, rather than building queue/lifecycle
mechanics from scratch. This keeps us out of the "hand-roll a
mini-scripting-runtime" trap while still avoiding any Blueprint/Verse-like
authoring surface — the schema stays plain, closed-vocabulary data.

## Next steps

- [x] Spike: `examples/expert/sequential_actions_spike.rs`. Confirmed
      `SequentialActionsPlugin` + `Action` coexist cleanly with
      `Runtime`/`RuntimeHandler`, and confirmed (both via stdout
      timestamps and visually — two cubes that only spin while their
      action is active) that tuple `.add((a, b))` is sequential, not
      concurrent.
- [x] Confirmed `is_finished`/`on_start` take `&World`/`&mut World`
      directly — the same signature `play_gltf_animation_in_world` and
      friends already use, so no adapter layer is needed to route a real
      xrds-runtime action through this queue later.
- [x] Drafted the `Trigger`/`Action`/`Sequence` schema — see "Proposed
      schema" below.
- [x] Sketched the consumer system that reads `XrZoneEnterEvent`/
      `XrZoneExitEvent` and enqueues actions per the authored `Sequence`
      data — see "Proposed schema" below.

This design doc's job is done for v1: **see
[`xrds-trigger-action-implementation-plan.md`](xrds-trigger-action-implementation-plan.md)
for the phased build-out**, and
[`xrds-trigger-action-backlog.md`](xrds-trigger-action-backlog.md) for
candidate `Action` variants beyond v1 (audio, materials, physics,
networking, ...), explicitly not scheduled.

## Scope: what the trigger-action system is, and isn't

Stress-tested against a concrete game-like scenario (dynamic scene,
spawned player, "hit button → teleport," "bullet hits player → reduce
HP") to find the real boundary before building anything:

- **In scope:** wiring "when trigger T fires (for some entity), run a
  short, parameterized recipe of `Action`s against that entity/target."
  `Action` stays closed-vocabulary and parameterized (e.g.
  `Action::Teleport { destination }`, `Action::ModifyHealth { target,
  delta }`) — parameters are fine, arbitrary branching logic is not what
  this layer is for.
- **Out of scope, stays as ordinary Bevy components/systems (the expert
  layer):** the gameplay state and simulation itself — health tracking,
  physics movement, projectile spawning/flight, input handling, AI,
  death/respawn logic, anything that needs to branch on live state at
  decision time. If a sequence needs an `if HP < 20% then X else Y`, that
  is the signal to drop to a real Bevy system, not to grow `Action`'s
  vocabulary into a scripting language.
- **`Trigger` must not be a small hardcoded enum of sources.** The
  original sketch (`ZoneEnter`/`ZoneExit`/`Timer`) was too narrow the
  moment a real gameplay case (bullet-hits-player, a dynamic collision via
  `avian3d`, not a static sensor zone) showed up — and more sources will
  keep appearing as the SDK grows (UI widget events, animation-complete,
  custom gameplay/networking events) that we can't fully enumerate today.
  **Design `Trigger` as an open, pluggable mechanism instead of a closed
  list:** any event type that should be able to fire a sequence
  implements a small trait exposing which entity it targets (e.g. `trait
  TriggerEvent { fn target(&self) -> Entity; }`), and one generic
  consumer system (`fn consume_triggers<E: Event + TriggerEvent>(...)`)
  is registered per event type that opts in. `XrZoneEnterEvent`/
  `XrZoneExitEvent` become the first two implementors; a future
  `avian3d` collision-started event becomes a third, added by
  implementing the trait — **no changes to the core `Trigger`/`Sequence`
  data model or consumer logic required** each time a new trigger source
  is added. `Action` stays closed-vocabulary (that's the actual
  Blueprint-avoidance guarantee); only the *trigger source* mechanism
  needs to be open, since new sources are a certainty and hardcoding them
  ahead of time isn't.

## Priority call

Decided: **build the trigger-action system before other new SDK
components**, not as a later add-on. Rationale: it's foundational
plumbing that other planned features (interaction zones, UI widgets,
spawn zones, gltf-driven behavior, physics-driven gameplay) will want to
hook into — building it first means those features get trigger/action
support natively, instead of every one of them needing its own retrofit
later.

## Proposed schema (draft v1 — not yet implemented)

Concrete types, following every decision above: closed-vocabulary
`Action`, open/pluggable `Trigger` source mechanism, authored data
strictly separate from live queue state, `bevy-sequential-actions` as
the execution substrate underneath.

### Authored data (document layer — lives in `xrds-scene-graph`)

```rust
/// One parameterized, closed-vocabulary effect. This enum is the actual
/// Blueprint/Verse-avoidance guarantee — no arbitrary logic, no
/// branching, just named operations with data. Grows by adding variants,
/// same cost model as adding a new `XrdsSceneNodePayload` kind.
pub enum XrdsAction {
    PlayGltfAnimation {
        selector: XrdsGltfAnimationSelector,
        options: XrdsGltfAnimationPlaybackOptions,
    },
    StopGltfAnimation,
    SetVisible(bool),
    Teleport { destination: Vec3 },
    ModifyHealth { target: XrdsActionTarget, delta: XrdsActionValue },
    Wait { seconds: f32 },
    FireCustomEvent { name: String }, // escape hatch into the expert layer
}

/// Which entity an `XrdsAction` applies to.
pub enum XrdsActionTarget {
    SelfNode,               // the node the sequence is authored on
    Node(XrdsSceneNodeId),  // an explicitly-named other node
    TriggerSource,          // whichever entity fired the trigger (e.g. the bullet)
}

/// A value baked in at author time, or pulled from whatever fired the
/// trigger. See "Open question: dynamic parameters" below — this variant
/// is the honest, not-yet-fully-designed part of this draft.
pub enum XrdsActionValue {
    Fixed(f32),
    FromTriggerSource, // e.g. read the bullet's own damage value
}

/// An ordered list of `XrdsAction`s — purely data, no execution state.
/// Runs sequentially (matches `bevy-sequential-actions`' actual, verified
/// behavior — see the spike above).
pub struct XrdsSequence {
    pub steps: Vec<XrdsAction>,
}

/// The recognized trigger kinds an author can pick from today (e.g. in an
/// `xrds-editor` dropdown). Grows by one variant each time a new
/// `XrdsTriggerEvent` implementor is wired in — see below for why that's
/// cheap.
pub enum XrdsTriggerKind {
    ZoneEnter,
    ZoneExit,
    // future: CollisionStart, AnimationComplete, Custom(String), ...
}

/// "When trigger kind K fires for this node, run sequence S." A node can
/// have several bindings (e.g. one for ZoneEnter, a different one for
/// ZoneExit).
pub struct XrdsTriggerBinding {
    pub trigger: XrdsTriggerKind,
    pub sequence: XrdsSequence,
}
```

`Vec<XrdsTriggerBinding>` is the new data this design adds to
`XrdsSceneNode`/`XrdsInteractionZone` — everything above this line is
plain, closed, serializable data, no different in kind from the payload
types that already exist.

### The open/pluggable trigger mechanism (runtime layer)

```rust
/// Any Bevy event that should be able to fire a sequence implements
/// this. Implementing it for a new event type — e.g. a future avian3d
/// collision-started event — is the entire cost of adding a new trigger
/// source. No changes to `consume_triggers` or the data types above.
pub trait XrdsTriggerEvent: Event {
    fn target(&self) -> Entity;
    fn kind(&self) -> XrdsTriggerKind;
}

impl XrdsTriggerEvent for XrZoneEnterEvent {
    fn target(&self) -> Entity { self.zone_entity }  // exact field TBD
    fn kind(&self) -> XrdsTriggerKind { XrdsTriggerKind::ZoneEnter }
}
// XrZoneExitEvent mirrors this with XrdsTriggerKind::ZoneExit.

/// Runtime component holding a node's authored bindings — spawned at
/// scene-document import, per the document/runtime split above. Inert
/// until a matching trigger event actually arrives; nothing is enqueued
/// at import time.
#[derive(Component, Default)]
pub struct XrdsTriggerBindings(pub Vec<XrdsTriggerBinding>);

/// Registered once per `XrdsTriggerEvent` implementor — not once per
/// trigger kind, and not once per gameplay feature. Adding
/// `avian3d`'s collision event later means one more
/// `app.add_systems(Update, consume_triggers::<CollisionStarted>)` call,
/// nothing else changes here.
pub fn consume_triggers<E: XrdsTriggerEvent>(
    mut events: MessageReader<E>,
    bindings: Query<&XrdsTriggerBindings>,
    mut commands: Commands,
) {
    for event in events.read() {
        let target = event.target();
        let Ok(node_bindings) = bindings.get(target) else { continue };
        for binding in node_bindings.0.iter().filter(|b| b.trigger == event.kind()) {
            enqueue_sequence(&mut commands, target, &binding.sequence);
        }
    }
}
```

### Bridging to `bevy-sequential-actions`

`enqueue_sequence` translates each authored `XrdsAction` into the
crate's `Action` trait and pushes it onto the target entity's queue —
this is the one place our closed enum meets the crate's execution
mechanics, confirmed workable by the spike (`&mut World`/`&World` is
exactly what both sides use):

```rust
struct XrdsActionRunner {
    action: XrdsAction,
    deadline_secs: Option<f32>, // only used by Wait, same pattern as the spike's TimedAction
}

impl Action for XrdsActionRunner {
    fn is_finished(&self, agent: Entity, world: &World) -> bool {
        match (&self.action, self.deadline_secs) {
            (XrdsAction::Wait { .. }, Some(deadline)) => {
                world.resource::<Time>().elapsed_secs() >= deadline
            }
            (XrdsAction::PlayGltfAnimation { .. }, _) => {
                // poll via gltf_animation_state_in_world, per api.rs
                todo!()
            }
            _ => true, // most actions (Teleport, SetVisible, ModifyHealth, ...) finish immediately
        }
    }

    fn on_start(&mut self, agent: Entity, world: &mut World) -> bool {
        match &self.action {
            XrdsAction::Teleport { destination } => { /* mutate Transform */ true }
            XrdsAction::ModifyHealth { target, delta } => { /* resolve target + delta, mutate Health */ true }
            XrdsAction::Wait { seconds } => {
                let now = world.resource::<Time>().elapsed_secs();
                self.deadline_secs = Some(now + seconds);
                *seconds <= 0.0
            }
            XrdsAction::PlayGltfAnimation { selector, options } => {
                // play_gltf_animation_in_world(world, handle, selector.clone(), options.clone());
                false
            }
            // ...
        }
    }

    fn on_stop(&mut self, _agent: Option<Entity>, _world: &mut World, _reason: StopReason) {}
}
```

### Worked examples against the stress-test scenario

- **Button → teleport:** `XrdsTriggerBinding { trigger: ZoneEnter, sequence: XrdsSequence { steps: vec![XrdsAction::Teleport { destination }] } }` on the button's `XrdsInteractionZone` node.
- **Bullet → reduce HP:** requires (a) implementing `XrdsTriggerEvent` for a new collision event (not built here — the concrete motivating case for the pluggable-trigger design, not yet needed until physics-driven triggers are built), and (b) `XrdsAction::ModifyHealth { target: XrdsActionTarget::TriggerSource, delta: XrdsActionValue::FromTriggerSource }` — reading the bullet's own damage value rather than a fixed authored number.

### Open question: dynamic parameters (`XrdsActionValue::FromTriggerSource`)

Not resolved by this draft. `ModifyHealth`'s damage amount naturally
wants to come from the bullet entity that fired the trigger, not from a
fixed number authored on the target. This needs `XrdsTriggerEvent` (or a
sibling mechanism) to expose more than just `target()`/`kind()` — some
way to read arbitrary data off the triggering entity/event. Left open
rather than guessed at; worth its own short design pass once a concrete
second `Action` variant needing this shows up (avoids over-designing the
accessor mechanism against a single hypothetical case).

### Resolved since this draft

- **Storage location:** decided — inline, not a separate linked file
  (glTF-style external references are reserved for heavy binary assets;
  this is small structured data, same category as other payload data
  that's already inline). **Corrected once more, though:** the first
  version of this decision put `triggers` inside `XrdsInteractionZone`'s
  payload specifically — that turned out to be wrong, since it would make
  trigger-action data depend on a node having a zone, contradicting the
  open/pluggable-trigger-source decision below (a bullet-hits-player
  binding needs to live on the player, a plain physics body with no zone
  at all). Final answer: `triggers: Vec<XrdsTriggerBinding>` is a
  top-level field on `XrdsSceneNode` itself, alongside the existing
  top-level `grabbable: bool` — not nested in any payload variant. See
  the implementation plan's Phase 2.
- **`FromTriggerSource` accessor mechanism:** decided — Option C, a
  generic `XrdsTriggerValue(f32)` component populated by ordinary
  gameplay code, not a hardcoded field-enum or reflection-based field
  path. This also surfaced that `XrdsTriggerEvent` needs a `source()`
  method in addition to `target()`/`kind()` (the entity that *caused* the
  trigger, vs. whose bindings to check — they can differ). See the
  implementation plan's Phase 3.

### Re-fire semantics and the agent model (decided)

The question "what happens if a trigger fires again while its sequence is
still running" turned out to be the wrong question — it's not a
trigger-layer concern at all. **Trigger-action and sequencing are two
separate systems that collaborate**, and this belongs entirely to the
sequencer half.

Surveyed how mainstream engines handle it, and they all agree: **the
trigger/detection layer never suppresses.**

- **Unity** — `OnTriggerEnter(Collider other)` fires every time, for
  every collider, and hands you `other`. No dedup.
- **Unreal** — `OnComponentBeginOverlap` fires per-overlapping-actor with
  `OtherActor`. Blueprint ships explicit **DoOnce**/**Gate** nodes
  *precisely because* the engine imposes no policy — suppression is
  something the author asks for.
- **Godot** — `body_entered(body)` signal, same shape.

So: always fire, always report the source, and let the consumer decide.
The concrete design consequence is **what the sequencer's agent is**,
since `bevy-sequential-actions` puts the queue on the agent entity:

- ❌ *Agent = the target node* → one queue per node, so two players
  entering the same zone means player 2's sequence waits behind player
  1's. Wrong: a second firing from a **different source** is a valid,
  independent event and must run independently.
- ✅ **Agent = an ephemeral entity spawned per firing**, carrying the
  resolved `(target, source)` pair, despawned once its queue drains.
  Each `(target, source)` event gets its own execution context — exactly
  how Unity/Unreal behave, since each callback invocation is its own
  context.

**Consequences:**

- `ZoneExit` does **not** auto-cancel an in-flight `ZoneEnter` sequence.
  They're independent bindings firing independent agents; implicit
  cross-cancellation would be hidden magic.
- Author-requested suppression (a `once: bool` or similar on the
  binding) is a **later, explicit opt-in**, mirroring DoOnce being a node
  you deliberately place. Not in v1.
- `XrdsTriggerEvent`'s methods return `XrdsId`, not `Entity` — the
  existing `XrZoneEnterEvent`/`XrZoneExitEvent` carry `zone_id`/
  `entity_id` as `XrdsId`, so the consumer system resolves them through
  `XrdsIdIndex`. (The earlier schema sketch above said `Entity`; that
  predated checking the real event shape.)

### Still open

- Error handling: target node missing/despawned when a trigger fires,
  unknown `XrdsTriggerKind` on load (older saved scenes vs. newer engine
  code) — the implementation plan's Phase 1 adds a concrete test for the
  additive-schema-evolution case; the missing/despawned-target case at
  fire-time is not yet designed.
- Editor UI shape for authoring bindings/sequences (list-based, not
  node-graph, per the earlier "no Blueprint-shaped surface" decision) —
  tracked as the implementation plan's Phase 6, deliberately not
  designed until Phases 0-5 land.
