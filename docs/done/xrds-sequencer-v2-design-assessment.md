# Sequencer redesign (design #2) — feasibility & pre-work assessment

> **Revision note:** the sections below marked **DECIDED** reflect answers
> given after the first pass of this doc. Sections without that marker are
> still the original assessment and stand as written. All open questions
> are now resolved — see
> [`xrds-sequencer-v2-implementation-plan.md`](xrds-sequencer-v2-implementation-plan.md)
> for the checklist-form implementation plan built from these decisions.

Assessing `Sequencer Editor.dc.html` ("design #2" — AXIS-style, IBM Plex,
graphite/blue palette) as the target visual/UX direction for
`xrds-editor`'s trigger-action panel, against what
`xrds-scene-graph`/`xrds-runtime` actually implement today. Design #1
(`Sequencer Editor v1.dc.html`) is not the target; it's referenced only
where it clarifies what changed between the two.

Ground truth for "what we have today" is cited directly from
`crates/xrds-scene-graph/src/scene/trigger_action.rs` and
`crates/xrds-scene-graph/src/scene/timeline.rs`.

## Verdict

Design #2's *chrome* (menubar, viewport, panel layout, IBM Plex type,
graphite palette, track list with M/S/L, dual ruler) is straightforward to
adopt — it's presentation, and none of it depends on data we don't have.

Design #2's *authoring model* is only partly ours. Three of its ideas are
things our current architecture already does, just inverted in framing —
cheap to adopt once relabeled. One idea (continuous interpolated
transforms) is a real capability we don't have and would need new runtime
work, not just new UI, if kept. The rest of the specific actions/fields
shown (`Set Node State`, structured `Emit Event` payloads) are the ones
worth pruning, per your note that we don't need every action/state the
mockup happens to show.

## Action Sequence Editor is back in scope — DECIDED

Revised requirement: keep #1's dedicated "Action Sequence Editor" panel.
It is not redundant with #2's bottom Sequencer/track view once the two are
given distinct, complementary jobs instead of both showing the same list:

- **Sequencer (bottom panel, #2-style)** — the spatial/temporal overview.
  Multiple tracks/lanes at once, keys drawn on a ruler, playhead scrubbing,
  zoom. Answers "what happens, and roughly when, across the whole
  Sequence/Timeline."
- **Action Sequence Editor (docked side panel, #1-style, positioned like
  #1's middle panel between viewport and hierarchy)** — the focused list
  editor for whichever track/binding currently has focus (selected from
  the hierarchy, or by clicking a key on the Sequencer below). Answers
  "what exactly are this one track's steps, in order, with all their
  fields." This is the same job `TriggerActionEditorOverlay.tsx` already
  does today — the change is docking it as a persistent panel instead of
  a full-viewport modal, and scoping it to one track's steps rather than
  one Runnable's entire body when several tracks share a Runnable.

Selecting a key in the Sequencer scrolls/highlights the matching row in
the Action Sequence Editor; adding/reordering/removing a step there
updates the Sequencer's drawing immediately. Two views, one underlying
list — not three ways to edit the same thing (the redundancy flagged in
the original pass of this doc, between the middle panel, the Properties
panel's alternate "Actions" tab, and the timeline keys themselves, is
avoided by *not* replicating this list a third time in an Inspector tab).

## What #2 assumes that we already have (just framed backwards)

**"Trigger Node" as a standalone hierarchy entity a Sequencer "binds" to.**
We don't have a separate Trigger Node type, and don't need one:
`XrdsInteractionZone` (`crates/xrds-components/src/interaction.rs:115`) is
already a real component living on whichever node has the zone volume, and
`ZoneEnter`/`ZoneExit` already fire from that node
(`XrdsTriggerKind::ZoneEnter`/`ZoneExit`,
`crates/xrds-scene-graph/src/scene/trigger_action.rs:162-163`). The
direction is just reversed: #2 has a Sequencer reference a Trigger Node;
we have the zone node hold a `XrdsTriggerBinding` that names a runnable
(`XrdsTriggerBinding::runnable: Option<String>`,
`trigger_action.rs:251-264`). Same relationship, opposite arrow.

**Recommendation:** don't add a new node kind. Build the "Triggers" grouping
and the Sequencer's "bound to trigger" chip as a *derived, editor-side
view* — a reverse index over existing `node.triggers` bindings and the
`runnables` registry, computed at render time. Zero schema/runtime change.

**Per-node/registry diagnostics, richer than #2's status bar.**
#2 only shows a blanket "no validation errors" string. We already have
structured, itemized diagnostics —
`XrdsSceneDocument::trigger_diagnostics()` (`trigger_action.rs:347`)
returns `Vec<XrdsSceneTriggerDiagnostic>` with `node_id: Option<NodeId>`,
`severity: Info|Warning|Error`, `title`, `detail`, covering: unknown
trigger kind, hand filter on a handless kind, dangling `runnable`/`Run`
reference (both binding-level and registry-level), `Run`-cycle detection,
empty sequence, unrecognized action, glTF action on a non-glTF node,
`Custom` event with no emitter/listener in the document, dangling
`ModifyHealth`/`DistanceTo` node targets, negative hysteresis. This is
strictly better than what #2 shows — keep our version, just give it a
per-item surface in the new layout instead of collapsing it to one line.

## AnimateTransform — DECIDED: build it

Confirmed via search: there is no interpolation/tweening/easing code
anywhere in the trigger-action system today (`XrdsAction::Teleport` is
instant — `trigger_action.rs:36-38`). Building it.

**Proposed shape**, consistent with the existing closed-vocabulary
philosophy (`trigger_action.rs:13-24`) and with `Teleport` staying exactly
as it is today (the "instant jump" mode) rather than being replaced:

```rust
AnimateTransform {
    // Each field independently optional — None means "leave this
    // component alone," which is exactly the per-field override arrows
    // (yellow = overridden, gray = not) design #1's STATE OVERRIDES
    // section showed for Position Y vs Rotation Z.
    position: Option<[f32; 3]>,
    rotation: Option<[f32; 3]>,   // Euler degrees, matching XrdsAxis's convention elsewhere
    scale:    Option<[f32; 3]>,
    duration_secs: f32,
    #[serde(default)]
    ease: XrdsEaseCurve,
}

#[derive(Default)]
enum XrdsEaseCurve {
    Linear,
    Quad,
    #[default]
    Cubic,
}
```

In the redesigned Inspector, `Teleport` and `AnimateTransform` present as
the two **Mode** options on one "Transform" property row — exactly #2's
Teleport/Interpolate toggle — rather than as two unrelated action-type
picker entries.

**DECIDED: Out-only for v1.** `XrdsEaseCurve` ships as exactly
`Linear`/`Quad`/`Cubic` (each implicitly an "ease-out" curve, matching
#2's own "Cubic Out" default). `QuadIn`/`QuadInOut`/`CubicIn`/`CubicInOut`
are cheap to add later as new variants if a real case needs them — closed
enum, same cost model as any other action addition.

**Runtime side:** needs an interpolator system alongside the existing
`XrdsTimelineAgent`/sequential-actions dispatch — tracks start
transform → target transform, advances by `duration_secs`, applies the
ease curve, completes (or blocks the queue, if inside a `Sequence` with
default `wait` semantics matching `Run`'s existing pattern).

## "Node state" reframed — transform / visible / material / animation

Clarified scope: "node state" means a small, closed set of concrete
properties — not an open-ended named-state machine (the "Extending" style
string state I flagged as a scripting-surface risk in the first pass).
That changes the recommendation from *exclude* to *build as separate
typed actions*, mapped onto what already exists:

| Property          | Action                                                       | Status                                                  |
| ----------------- | ------------------------------------------------------------ | ------------------------------------------------------- |
| Transform         | `Teleport` (instant) / `AnimateTransform` (interpolated) | Teleport exists; AnimateTransform is new, decided above |
| Visibility        | `SetVisible(bool)`                                         | Already exists, no change needed                        |
| Playing animation | `PlayGltfAnimation` / `StopGltfAnimation`                | Already exist, no change needed                         |
| Material          | *(none yet)*                                               | **New — needs a decision**                       |

**Material is the one gap.** Nothing in the trigger-action system can
change base color / metallic / roughness at trigger time today (the
`SetMaterial`/`CommitMaterial` commands that exist are editor-authoring
IPC commands, not a runtime `XrdsAction` variant — they let you set a
material once while authoring, not "flip a light's color to red when a
zone is entered").

**Proposed shape**, instant-only (no interpolation) for v1, mirroring
`PrimitiveSection`'s existing `MaterialParams`:

```rust
SetMaterial {
    target: XrdsActionTarget,   // reuse the existing enum — SelfNode/Node/TriggerSource
    base_color: Option<[f32; 4]>,
    metallic:   Option<f32>,
    roughness:  Option<f32>,
}
```

**DECIDED: instant-only for v1.** Animated material (color fade, metallic
ramp) can reuse `AnimateTransform`'s interpolator infrastructure later —
noted here as a cheap follow-up, not built now.

## What #2 shows that's still worth pruning

**Structured `Emit Event` payload** (`channel`, `payload: { level: 2 }`)
— `XrdsAction::FireCustomEvent` carries only `name: String`
(`trigger_action.rs:49-51`), by design (`XrdsActionValue::FromTriggerSource`
is the existing escape hatch for passing a value out — see
`trigger_action.rs:120-125`). Making the payload arbitrary/structured
reopens the "no scripting surface" guarantee the whole enum design is
built to preserve. **Recommendation: exclude** — keep it a plain name.

## What #2 is missing that we must not drop

Neither mockup shows these, but they're real fields on
`XrdsTriggerBinding`/`XrdsThresholdWatcher` today and need a home in the
new layout:

- **Hand filter** (`XrdsTriggerBinding::hand: Option<XrGrabHand>`,
  `trigger_action.rs:296`) — restricts a binding to Left/Right controller.
- **`disabled`** (`trigger_action.rs:281`) — park a binding without
  deleting it.
- **Threshold Watchers** (`XrdsThresholdWatcher`, `trigger_action.rs:814`)
  — continuous value (height, rotation, distance, scale) crossing a
  threshold fires a `Custom` trigger. Location **DECIDED**: stays in the
  Triggers hierarchy grouping (see below), not a separate tab.

## One internal inconsistency in #2 worth resolving before building it

The Master Timeline draws the "Sequence 1" block at a fixed horizontal
position/duration, tagged "⚡ trigger", right under a "start: ⦿
trigger-based" toggle. But a trigger-fired runnable's start time is
whenever the trigger fires at runtime — it has no fixed position on an
authoring clock. Only a real `XrdsTimeline` (`timeline.rs:23`, which does
have an absolute `at_secs` clock) has a legitimate fixed-position block.

**Recommendation:** reserve the absolute-ruler/fixed-block visualization
for actual Timelines. Render trigger-bound Sequences in an unpositioned
list/lane (ordered, not placed on the clock) so the UI doesn't imply a
timestamp that doesn't exist.

## Track/lane grouping — needed, but no schema change required

#2 renders one lane per (asset, action-type) pair (e.g. "Asset 1 ·
Transform", "Asset 2 · State"). We have no explicit track/lane field on
`XrdsSequence`/`XrdsTimeline` — but we don't need one: every action that
matters for visual placement already carries enough to derive a lane key
(the binding's own node for zone-scoped bindings, `ModifyHealth::target`,
`Run::runnable`) combined with a coarse category of `action.kind`. Actions
with no natural target (`Wait`, `FireCustomEvent`) fall into an implicit
"Flow" lane, same as #1's "FLOW" tag on its `Wait` row.

**Recommendation:** implement lane grouping as a pure editor-side
derivation over existing data. Only add an explicit manual lane-assignment
field later if users actually want to override the automatic grouping.

## Terminology — DECIDED, one term still open

- **"Sequencer" = the panel** (the whole editor surface, analogous to
  "Inspector" naming a panel, not a data type). **Decided.**
- **"Sequence" = the chained/ordered-action data** — maps directly to
  `XrdsSequence`. **Decided.**
- **Timeline events — recommend "Key" (short) / "Keyframe" (long form in
  tooltips/headers).** This is the one term I was asked to suggest.
  Reasoning:
  - It matches the actual type name 1:1 — `XrdsTimelineKey`
    (`timeline.rs:12`) — so UI copy and code never drift apart.
  - It's the industry-standard term (Unreal Sequencer, Unity Timeline,
    After Effects all call these "keys"/"keyframes"), so it costs nothing
    to learn for anyone who's touched an NLE/DAW/animation tool.
  - **Avoid "Event"** — despite it being the natural-sounding word for
    "a thing that happens on the timeline," it collides with vocabulary
    we already use for a *different* concept: `XrdsTriggerEvent` (the
    Rust trait app code implements to fire a trigger *in*),
    `XrZoneEnterEvent`/`XrZoneExitEvent` (the actual Bevy events), and
    "Custom event" (`FireCustomEvent`, `Custom(String)`). Calling a
    timeline entry an "Event" too would make "this sequence fires an
    Emit Event action containing... an event" a real sentence a user
    could type, which is exactly the kind of ambiguity worth avoiding
    up front.
  - Sequence steps keep the existing "step" wording (`XrdsSequence::steps`
    — already what `TriggerActionEditorOverlay.tsx`'s copy uses today);
    "Key" is Timeline-only. No change needed there.

## Triggers hierarchy grouping — DECIDED to stay, plus contextual filtering

Threshold Watchers stay grouped under a node's "Triggers" entry rather
than getting a separate tab — they already function as a trigger source
(a Watcher's whole job is to fire a `Custom` trigger), so splitting them
into a different part of the UI than `XrdsTriggerBinding`s would separate
two things that are conceptually one category. Recommend rendering them
as two labeled sub-rows under the same "Triggers" node entry — **Bindings**
(kind → runs a Sequence/Timeline directly) and **Watchers** (continuous
value → fires a named `Custom` event that some binding *elsewhere* may or
may not listen for) — rather than one flat merged list, since a Watcher
doesn't itself name a runnable and conflating the two would obscure that.

**Better approach for "filter out the invalid triggers":** rather than
only catching an invalid combination after the fact (which
`trigger_diagnostics()` already does — e.g. the hand-filter-on-handless-
kind Error), filter the trigger-*kind* picker's option list contextually,
based on what the selected node can actually support, so invalid options
are never offered in the first place. Diagnostics remain the safety net
for whatever filtering can't catch statically (a `Custom` name typo, for
instance — no amount of filtering catches that).

Filter table, verified against `xrds-runtime`'s actual dispatch code (not
just the scene-graph schema this doc started from):

| Trigger kind                                                                         | Only offered when                                       |
| ------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| `ZoneEnter` / `ZoneExit`                                                             | node's payload is `InteractionZone`                       |
| `Grabbed` / `Dropped`                                                                | node's `grabbable` flag is `true`                          |
| `HoverEnter`/`HoverExit`/`ButtonPress`/`ButtonRelease`/`SliderChange`/`ToggleChange`   | node is a `WorldPanel` widget (button/slider/toggle)       |
| `AnimationComplete`                                                                   | node's payload is `GltfAsset`                              |
| `Custom`                                                                              | always offered (name-matched, no node-type constraint)    |
| `RunawayDetected`                                                                     | **always offered — verified, see below**                  |

### What `RunawayDetected` actually is

The escape hatch for infinite `Run` loops. Every `Run` action tracks how
many `Run` hops deep the current chain is (`chain_depth`, capped at
`MAX_RUN_CHAIN_DEPTH = 64` —
`crates/xrds-runtime/src/xrds_api/trigger_action.rs:134`). It's a *depth*
cap, not a rate limit, deliberately: a rate limit can't tell "this is
looping forever" apart from legitimately-fast input (`SliderChange`
firing every frame while dragging), but a depth counter can — genuine
high-frequency input doesn't nest `Run` calls 64 levels deep in one causal
chain.

When the cap trips, the runtime doesn't just kill the chain silently — it
fires `XrdsTriggerKind::RunawayDetected` on the exact node the runaway
`Run` was executing on at that moment
(`fire_runaway_detected_in_world(world, node)`, `trigger_action.rs:557`,
called with `self.target` from `XrdsActionRunner`, `trigger_action.rs:995`),
same as any other trigger kind. If that node has a binding listening for
it, its Sequence/Timeline runs as an authored recovery (log it, reset
state, whatever fits) instead of the chain just hanging with no
diagnostic. The recovery chain is flagged `is_recovery: true`
(propagated to everything it spawns), and the runtime explicitly refuses
to ever fire `RunawayDetected` again from inside a recovery chain
(`trigger_action.rs:975-987`) — hard-erroring and dropping instead — so a
recovery handler can never become the new infinite loop itself.

**Concrete use case:** Sequence A does `Run("B")`, Sequence B does
`Run("A")` — a copy-paste mistake, or an intentional back-and-forth that
was supposed to terminate on some game-state condition that never
actually flips. `trigger_diagnostics()` flags this as a cycle but doesn't
block it (see the Run-cycle diagnostic — intentional event loops are
allowed). Without `RunawayDetected`, a mistaken cycle just spins forever
with zero diagnostic, burning CPU while the game appears to hang. With
it, at hop 64 the chain stops and — if authored for — you get a
controlled, visible response instead of a silent freeze.

**Resolved:** fires on whichever node happens to be executing the runaway
chain at the moment the cap trips — not tied to any component/payload
type. Confirmed universal, no filtering needed in the picker.

The hand-filter field itself should follow the same rule already encoded
in `trigger_diagnostics()` (`trigger_action.rs:434-445`): only show the
Left/Right picker when the currently-selected trigger kind is one of
`Grabbed`/`Dropped`/`Hover*`/`Button*`/`SliderChange`/`ToggleChange` —
hide it entirely otherwise, rather than showing it and relying on the
diagnostic to catch the mismatch after the fact.

## Mute / Solo / Lock — why ephemeral-only, explained

Real DAW/NLE tools (Premiere, Unreal Sequencer, Unity Timeline) *do*
persist mute/solo/lock into the saved asset — so "just make it ephemeral"
isn't the obviously-correct default, and deserves the explanation asked
for rather than a bare recommendation.

**What the feature is actually for, in either case:** isolating your
attention while authoring. Muting Sequence 2 while tuning Sequence 1's
timing stops its effects from firing and cluttering the viewport/log
while you scrub; Solo is the inverse (silence everything *except* the one
you're working on); Lock guards against an accidental drag/edit on a
track you're not touching right now. None of the three change what a
*player* ever experiences — a shipped scene has no "editor session," so a
mute/solo/lock flag has zero runtime meaning by construction, in either
design.

**Why persisted-anyway can still make sense (the NLE precedent):** those
tools persist it because the *project file* is worked on across many
sessions and handed between artists on a team, and losing your working
setup (which tracks you'd isolated to focus on) every time you reopen the
file is a real, repeated cost on a long-running production.

**Why *this* codebase should still default to ephemeral, for now — two
independent reasons, not just philosophy:**

1. **Technical blocker: there is nothing to attach the flag to yet.**
   Mute/Solo/Lock are per-*track* in every mockup, and per the track/lane
   section above, a "track" is a *derived* grouping (target + action-kind)
   with no stable identity in the document — recomputed fresh from
   `XrdsSequence::steps`/`XrdsTimeline::keys` every time the editor opens
   it. There is no field to persist a boolean *onto* without first giving
   tracks a real, addressable identity in the schema — which is a bigger
   change than the three booleans themselves, and not something this pass
   needs for any other reason.
2. **It would duplicate a mechanism that already exists and already
   means something specific.** `XrdsTriggerBinding::disabled`
   (`trigger_action.rs:281`) is already a persisted, deliberate "parked,
   but authored" flag — and its own doc comment explicitly rejects adding
   a second on/off switch with different semantics right next to it
   ("a second `enabled` field... would be actively confusing,"
   `trigger_action.rs:270-277`). A persisted `muted` would face the exact
   same problem: what does muted-but-not-disabled mean, at runtime, on a
   shipped scene? The honest answer is "nothing should be shipped in that
   state," which is a strong sign the flag doesn't belong in the document.

**Recommendation stands: ephemeral/session-only for this pass** — same
bucket as which node is selected, which panel tab is open, or the editor
camera's position: real, useful, resets on reopen by design. **Revisit
persisting it once tracks are reified into real addressable entities** (if
that ever happens for an unrelated reason, e.g. wanting manual track
reordering) — at that point the NLE precedent becomes the right call to
follow, not before.

## Priority list for pre-frontend work

**Decided, ready to scope into tickets:**

1. Action Sequence Editor panel restored as a docked side panel (not a
   modal), scoped to one track's steps, alongside the #2-style Sequencer
   overview panel.
2. `AnimateTransform` action + `XrdsEaseCurve` (Linear/Quad/Cubic, all
   ease-out only, default Cubic) + runtime interpolator system.
3. `SetMaterial` action, instant-only (`base_color`/`metallic`/`roughness`,
   reusing `XrdsActionTarget`).
4. Terminology: Sequencer (panel) / Sequence (data) / Key (timeline entry)
   / step (sequence entry).
5. Threshold Watchers rendered as a sub-grouping under each node's
   Triggers entry (Bindings vs Watchers), not a separate tab.
6. Contextual trigger-kind + hand-filter picker filtering (table above) —
   verified against `xrds-runtime`'s dispatch code, including
   `RunawayDetected`'s universal scope.

**P0 — needed for #2's layout to mean anything, no schema change:**
7. Editor-side track/lane derivation (target + action-kind → lane key).
8. Editor-side "Triggers" hierarchy grouping + reverse lookup (which
   node's binding points at runnable R; which runnables a node's bindings
   name) — powers the "bound to trigger" chip and hierarchy category.
9. Per-item diagnostics surfacing in the new layout (data already exists
   via `trigger_diagnostics()`; this is a frontend rendering task only).

**Confirmed ephemeral, no schema change:**
10. Mute/Solo/Lock — editor/session state only (see explanation above).

**Excluded from this pass (backlog candidates, not blockers):**
11. Structured `Emit Event` payloads beyond a plain name.
12. Frame-based (fps) display — cosmetic conversion over `at_secs`, fine
    to add later, not required for the redesign to work.

## Status: all open questions resolved

Every decision point raised in this doc is now settled — see the
**DECIDED** markers throughout, and the priority list above for the
consolidated ticket-ready scope. Nothing is blocking the start of
implementation work.
