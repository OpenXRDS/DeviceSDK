# Trigger-action sequencing — remaining plan

What is still **ahead**. The completed work (Phases 0-5, 7, 10) has moved to
[`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md); the
design rationale lives in
[`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md),
and unscheduled action variants in
[`xrds-trigger-action-backlog.md`](xrds-trigger-action-backlog.md).

Phase numbers are stable (code comments reference them), so they are
non-contiguous here — 0-5, 7 and 10 are done and live in the record doc.

**Read the terminology section before Phase 9.** "Sequence" (shipped) and
"timeline" (Phase 9) are different execution models, and conflating them
already caused one real misunderstanding.

## Open items — decisions needed

Both surfaced during the Phase 10 review and are recorded in full in
[`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md).

- [x] **Hand information — resolved.** Added `hand()` to `XrdsTriggerEvent`
  (default `None`), implemented on all 8 events that actually carry a
  controller (`Grabbed`/`Dropped`/`HoverEnter`/`HoverExit`/
  `ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange`), plus an
  optional `hand: Option<XrGrabHand>` filter on `XrdsTriggerBinding`.
  `None` (default) matches any hand — existing bindings are unaffected.
  `XrGrabHand` gained `Serialize`/`Deserialize` (it had neither; it only
  ever lived in runtime events/components before this).
  Applied everywhere a binding is matched — `consume_triggers` **and**
  `fire_trigger_in_world`/`XrdsAPI::fire_trigger` (both gained a `hand`
  parameter) — so an editor preview can't misrepresent what actually fires.
  **Diagnostics catch the resulting footgun:** a hand filter set on a
  trigger kind that never reports one (`ZoneEnter`, `AnimationComplete`,
  `Custom`, …) makes that binding permanently, silently unfireable — flagged
  as an `Error` in `trigger_diagnostics()`, not left to be discovered by a
  scene author wondering why nothing happens.
  Tests: filter matches only the specified hand, no filter matches either
  hand (the compatibility case), `fire_trigger` honors the argument, and
  both diagnostics cases (flagged on a handless kind, allowed on `Grabbed`).
- **Multiplayer authority is unaddressed.** In a networked scene, if a zone
  trigger fires on one client, does the sequence run everywhere? Every client
  simulating the same trigger locally means divergent state. Broader than the
  backlog's `SendNetworkMessage` note, and it interacts with the existing
  `xrds-net` work. Not a v1 blocker, but it should be written down rather
  than discovered later.
- **`XrdsSceneNode::enabled` is decorative today** and honoring it is a
  change affecting every node type, so it was left out of trigger-action
  scope. See Phase 10 in
  [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md) for the
  full reasoning and the template/instance discussion that came with it.

## Priority

Per the priority call recorded in the design doc: this is being built
**before** other new SDK components, as foundational plumbing other
planned features will hook into. Nothing here is blocked on unrelated
work.

## Non-goals (carried over from the design doc)

- No scripting language, no visual node-graph editor, no codegen.
- No general branching/conditional logic inside `Action` — that's the
  expert-layer escape hatch's job (`FireCustomEvent` → real Bevy system).
- No parallel execution via `bevy-sequential-actions` tuple-add — already
  confirmed sequential-only by the spike; genuine concurrency needs
  separate agents/queues, not attempted in v1.
- No editor UI designed here — tracked as Phase 6 below, deliberately not
  specified until the data shape it would edit is stable.

## Terminology: "sequence" vs "timeline"

Worth pinning down, because it caused a genuine misunderstanding mid-build.
What shipped in Phases 0-7 is an **ordered queue**, which these docs call a
"sequence". That is *not* the same thing as a timeline, and the difference is
not cosmetic:

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

## Phase 6 — editor integration (tracked, not designed here)

- [ ] `xrds-editor` property-panel UI for authoring `XrdsTriggerBinding`/
      `XrdsSequence` on interaction zones. **List-based** (add/remove/
      reorder steps, pick trigger kind and action from dropdowns) — not
      a node-graph, per the "no Blueprint-shaped authoring surface"
      decision in the design doc. Deliberately not scoped in detail here.
      Phases 0-5 have landed, so the core data shape is now stable — but
      Phase 9a will change how bindings reference what they run (registry
      names vs inline), so designing the UI before that settles would mean
      designing it twice.

      Two things this UI specifically needs, identified along the way: a
      **picker rather than free text** for `Custom(String)` trigger names
      and (later) runnable names, since string matching means a typo
      silently never fires; and an **instance list** — a view enumerating
      bindings across all nodes, which is what makes the template/instance
      split usable (see Phase 10 in the record doc).

## Phase 8 — threshold watchers (continuous to discrete)

**Status: planned, not started.**

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

### Decisions (settled)

**1. Bindings may be inline or named; `XrdsAction::Run` takes a name only.**

```rust
pub enum XrdsRunnableRef {
    Named(String),
    Inline(Box<XrdsRunnable>),
}
XrdsAction::Run { runnable: String, wait: bool }   // name only — see below
```

Supporting both costs almost nothing (resolution is one function returning
`&XrdsRunnable`, not two code paths) and avoids forcing registry ceremony on
a one-line "on enter, teleport" behavior. Restricting `Run` to a *name* is
the recursion firewall: the only way the data structure could nest is an
action containing a runnable, and a name cannot nest. Bindings can safely be
inline because a binding never sits inside an action.

**2. `Run` blocks via an explicit `wait: bool`, default `true`.**

Honored inside a sequence; **ignored, with a warning, inside a timeline** —
a timeline that pauses has broken the absolute timing that is its entire
purpose. This replaces an earlier proposal to make blocking implicit in the
executor's context: one action silently behaving two ways is the kind of
thing that gets debugged at 2am, so the author states intent instead.

Possible later addition (not first cut): `XrdsTriggerKind::RunnableComplete
{ name }`, mirroring the `AnimationComplete` precedent, so "when this
timeline finishes, do X" is expressible without blocking.

**3. Cycles: static detection for `Run`, runtime detection for the rest,
and a guaranteed escape.** See the next section.

**4. Migration: clean break, no legacy field.** Verified before deciding —
the only saved scene document on disk
(`android/quest/build/assets/scene.json`) has no `triggers` field at all, so
nothing in the wild depends on the current binding shape.

### Runaway loops: detect, break, and always provide an escape

Position: **do not prevent authors from writing loops** — Unity, Unreal and
Godot all permit infinite event loops, and a general prevention mechanism
would restrict legitimate designs. **But when a runaway IS detected, an
escape must always exist.** A mysterious hang is not an acceptable failure
mode.

**Detection: causal chain depth, not a rate limit.**

A rate limit cannot distinguish a loop from legitimately high-frequency
input — `SliderChange` fires every frame while a slider is dragged, which is
correct behavior, not a runaway. The actual discriminator is *causality*: a
loop is self-sustaining, driven by the system's own prior action rather than
by outside input.

So propagate a chain depth through every SDK-mediated causal link:

- `XrdsSequenceAgent` gains `chain_depth: u32`.
- `XrdsAction::Run` spawns its child agent at `depth + 1`.
- `XrdsAction::FireCustomEvent` stamps the emitted `XrdsCustomTriggerEvent`
  with its agent's depth; when that event is consumed as a `Custom` trigger,
  the resulting agent starts at `depth + 1`.
- Exceeding a cap (default 64, overridable via a resource) means the next
  agent is simply not spawned. The loop stops there.

**Known coverage gap, stated rather than papered over:** an app-defined
trigger event cannot carry our depth stamp, so a loop routed through
application code is not detected. That is the same position as other
engines, and it is documented as the author's responsibility.

**Escape, three parts:**

1. **Automatic break.** At the cap the chain stops spawning. Bounded by
   construction, no configuration required.
2. **An escape trigger.** `XrdsRunawayDetectedEvent { node_id, chain_depth,
   runnable }` is emitted, and it implements `XrdsTriggerEvent` as
   `XrdsTriggerKind::RunawayDetected` — so a recovery sequence can be
   *authored*, not just logged. Reuses the pluggable trigger mechanism
   rather than inventing a parallel channel.
3. **A manual kill switch.** `XrdsAPI::stop_sequences_on(node)` and
   `stop_all_sequences()` clear queues and despawn agents. Useful well
   beyond loops — scene transitions, aborting a cutscene on player input.

**Recovery-path protection.** A sequence started from `RunawayDetected`
begins at depth 0 but is flagged: if it exceeds the cap itself, it is
dropped with a hard error log and does **not** emit another
`RunawayDetected`. One level of recovery, so the breaker can never recurse
through its own recovery path.

Also: log a warning when live agent count crosses a threshold, purely as a
diagnostic. It catches the undetectable app-mediated case as a symptom, so
that failure is at least discoverable instead of a silent hang.

### Phase 8 decisions (settled)

- **Crossings re-arm automatically** — edge-triggered on every crossing. A
  `once` flag is deferred until something actually asks for it.
- **`Health` is not an observable.** `XrdsHealth` exists only as a data
  slot; putting it in the observable list starts pulling gameplay semantics
  into the SDK. Ship the transform-derived observables first and let a real
  use case argue for it.

