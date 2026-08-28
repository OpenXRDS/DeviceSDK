# Animatable properties — making events interpolate

**Phase A built 2026-08-28.** `SetMaterial` now takes a duration and an ease
curve, the same two fields `SetTransform` has always had.

**Phase B built 2026-08-28.** `SetLight` joins them — colour, intensity and range,
across all three light kinds.

## Why this instead of a keyframe curve editor

The question that started this was whether the sequencer needed a curve editor —
a canvas of keyframes with draggable bezier handles, the way Unity's Animation
window or Unreal's Sequencer works. It does not, and the reason is worth writing
down because the alternative looks more capable.

A curve editor is how you author a value that changes *shape* over time:
overshoot, hold, settle. Nothing in this SDK asks for that yet, and every
property an author currently animates is a straight A → B with an ease on it. The
honest version of "detail over time" here is not a curve canvas — it is placing
several events and letting each one interpolate, which the Track model already
expresses.

So the work is not a new editing surface. It is **making the properties
themselves animatable**, one action at a time, reusing one interpolator. That was
already the plan the code itself recorded: `SetMaterial`'s own doc comment said to
"add one later by reusing `SetTransform`'s interpolator infrastructure if a real
case needs it".

## What Phase A shipped

`XrdsAction::SetMaterial` gained `duration_secs` and `ease`, both
`#[serde(default)]`. Zero is instant, which is what every document authored
before this deserialises to — the behaviour did not change under anyone's feet.

`XrdsMaterialTween` holds the in-flight state and `advance_material_tweens`
drives it, mirroring the transform tween: it respects a paused agent's lock, and
it removes itself on completion, because an agent still holding a tween never
finishes and would stall a sequenced Track behind it.

Three decisions worth keeping:

- **A texture swap applies at the start, not the end.** An image has no halfway
  point. Holding it back would mean fading to a colour the new picture never
  wears, and swapping at the end would make a fade-to-a-new-texture snap.
- **`XrdsEaseCurve` grew in-variants rather than changing the old ones.**
  `Quad` and `Cubic` were always ease-*out*; a fade-up usually wants ease-in.
  Redefining the existing names would have quietly changed the motion of every
  scene already authored, so `QuadIn`, `CubicIn`, `QuadInOut` and `CubicInOut`
  were added alongside. An unrecognised name falls back to `Cubic`, so a document
  from a newer editor loads with sensible motion instead of failing.
- **`self_duration_secs` had to learn about it in both languages.** Rust's
  `XrdsAction::self_duration_secs` and the editor's `actionDuration` both fed the
  ruler span and the dot-versus-bar decision, and both hard-coded `SetTransform`.
  Left alone, a three-second fade would have drawn as an instant dot and a Track's
  automatic length would have ended before the fade did — the action working
  while the editor showed it not working, which is this project's most familiar
  failure shape wearing a new hat.

## "In-out" was unreadable, and that was a real defect

First use of the shipped feature turned up a naming failure rather than a code
one. The curves were labelled by their polynomial — "Cubic in-out" — and that was
read as *the value goes out and comes back*. It does not: every curve here is
monotonic, running once from the current value to the target. In / out / in-out
say only **when the change is fast** — at the end, at the start, or in the middle.

Two things followed from it, and both were worth more than the fix:

- **The dropdown became a row of drawn curves.** Names were the problem, so the
  fix was to stop relying on them: each choice is a thumbnail of its own shape
  against a dashed linear reference, labelled only enough to tell neighbours apart
  ("Out", "Out+", "In-out"). Every thumbnail rises from bottom-left to top-right
  and never turns down, which answers "does it come back?" in a glance — the
  explanatory sentence that briefly sat under the control did not, and is gone.
  Modelled on the audio falloff preview, which exists for the same reason: the
  authored numbers do not tell an author what they will get, and the picture does.
  The TS curve is a hand-mirror of the Rust one, so `ease.test.ts` samples the same
  points the Rust test does — a drifted preview teaches the wrong shape and is
  worse than none.

- **The report was "all the options work the same", which is exactly what a
  broken pipeline looks like.** `ease_curve_from_dto` falls back to `Cubic` for an
  unrecognised name; one missing arm would have collapsed every choice to one
  curve while the picker kept working. That was ruled out by test, not by
  reading — `every_ease_curve_round_trips_through_the_dto` and
  `the_ease_curves_are_actually_distinct` now hold both halves down. Neither
  existed before, which is why the symptom could not be answered without going
  and looking.

Colour is also the property where curve shape is hardest to see: the eye tracks
timing far better on a moving object than on a fading one. Worth knowing before
concluding a curve does nothing.

The editor's duration + ease row is now one shared control rather than two
hand-written copies. That is the point rather than a tidy-up: an author who has
learned that a move can be given a time expects a colour change to work the same
way, and two copies would have drifted the moment one gained a curve.

## Phase B — `SetLight`

Colour, intensity and range, interpolating through the same curve. Third action of
the pattern, and the first one that cost almost nothing to add — which was the
point of building the interpolator once.

Four decisions, all of them about the fact that "a light" is three different
things:

- **One action for all three kinds, not three.** A node is a point, spot or
  directional light, and the action drives whichever it lands on. The alternative
  makes an author pick the right variant before knowing which kind they placed.
- **One `intensity` field, and it is two units.** Lumens on a point or spot light,
  lux on a directional one. That is genuinely a unit conflation, and it is what
  Unity and Unreal both do — a single "Intensity" meaning whatever that light type
  means. The editor says `lm · lux if directional` beside the field, because 1000
  is a reasonable point light and a pitch-dark sun, and that is not a discovery to
  make on a headset.
- **`range` follows the light, not the action.** A directional light has none —
  the sun does not fall off — so an authored range is dropped rather than written.
  The tween carries `range: Option<f32>` for the same reason: `None` throughout
  means a directional tween never touches a property that does not exist, instead
  of interpolating a phantom zero.
- **A third tween component, not one generic one.** `XrdsTransformTween`,
  `XrdsMaterialTween` and `XrdsLightTween` are driven by different setters, and
  merging them would mean a light change cancelling an unrelated transform change
  on the same entity. That argument got stronger afterwards rather than weaker:
  each of the three now holds one *channel per property*, so the separation is
  between families that cannot interfere, and within a family only the same
  property replaces. See [`tween-channels-plan.md`](tween-channels-plan.md).

A `SetLight` that sets **no field at all** is diagnosed: it applies successfully
and changes no pixel, so nothing at runtime would ever report it.

### It shipped offerable on any node, which was the whole failure again

`SetLight` could be added to a cube. It saved, previewed, and did nothing —
authorable-but-inert, in the feature added *to* a document about that failure.

The cause was not a missing idea but an unwired one. `lib/sequencer.ts` already
exported `actionUnavailableReasonFor`, the guard for exactly this, and **nothing
in the UI called it** — only its own tests did. The picker used a second,
divergent copy of the action list declared in the component, which had drifted:
the tested list was missing the audio and video actions, so the list under test
was not the list on screen. And `track_diagnostics` already refused glTF actions
on non-glTF nodes and effect actions on non-effect nodes; the light case was
simply never added beside them.

Three gates now, matching what glTF and effects always had:

- **The picker greys it out** with the reason beside it, rather than hiding it —
  `Select` already supported `disabled` + `hint` and its own comment argues for
  showing the reason. The two action lists are now one.
- **The document reports an error**, which is the authoritative gate. An editor
  picker cannot see a hand-edited file or one from another tool — the same lesson
  the video texture slot taught, where the runtime resolver and the picker were
  both fixed and the document validator was missed.
- **The runtime still finishes rather than stalling**, because a Track blocked
  forever on a tween that will never appear is invisible in a way a wrong
  brightness is not.

`AmbientLight` is excluded from all three, and it is the trap worth naming: it
reads like a light and is a *resource* at runtime rather than one of the three
light components, so grouping it by its name would have put back the exact inert
event the gates remove.

The same pass gated glTF, effect and audio actions in the picker. `SetMaterial`
and the video actions stay ungated on purpose — a material is carried by many
payloads and a video is bound to a texture slot, so a node's payload kind cannot
answer either, and greying those out on a guess would refuse valid authoring.

## Overlapping events compose

Two events animating one node at the same time used not to compose — the second
tween replaced the first as a component and the first's animation stopped with no
warning. It was the reason "give each property its own curve" could not be answered
with two events, which is how it should be answered.

All three tween families now hold one channel per property, and an event merges
into them rather than replacing. So colour on one curve and intensity on another is
authorable as two events, with no new UI — which is the answer to the question that
found the bug. See [`tween-channels-plan.md`](tween-channels-plan.md).

## Rotation is not like the other properties

Three things surfaced from first use, and none of them was a bug in the tween.

**A turn always takes the shorter way round.** Rotation interpolates by `slerp`,
which follows the shorter arc between two orientations. Measured: a 0 → 350° turn
moves **−10°**, not +350°. So more than a half turn is not expressible as one
event, by construction — it is two.

**The editor wraps an authored angle into (−180, 180] on commit** rather than
clamping it. The distinction matters: 350° and −10° are the same orientation, so
wrapping keeps what the author meant, while a `max=180` would have silently
pointed the object somewhere else. Seeing 350 become −10 in the box says the
short-path rule up front, instead of leaving it to be discovered at runtime.
Wrapping happens on commit, not while typing, or it would fight an author on their
way to typing "350". A half turn resolves to **+180**, not −180, so a committed
value does not flip sign each time it is re-committed.

**Three sliders sit under the boxes**, one per axis, spanning exactly the
authorable range. That bound is the argument for a slider here and not on position
or scale: those are unbounded, and a slider over an unbounded quantity has to
invent limits. The boxes stay — a slider is for finding an angle by eye, typing is
for saying exactly 90 — and a drag commits once on release rather than per frame,
so it is one undo entry.

**(180, 180, 180) is exactly no rotation.** Three half-turns about perpendicular
axes compose to a full turn; measured at 0.0000° from identity. Every part of it
behaves normally on its own — (180,0,0), (0,180,0), (180,180,0) and (90,90,90) are
all a genuine 180° from identity — so only the combination cancels. An author sets
three large numbers, nothing moves, and there is nothing to read that explains it.
`Rotation target is the current facing` now warns when an authored rotation
resolves to the orientation the node already has, comparing quaternions rather than
the typed numbers so it catches the general case and not just this one. A warning
rather than an error, and only on `Node` rows: the runtime turns from the *current*
orientation, which an earlier event may have changed, so "as authored" is all it
can honestly claim.

## Phase C — the rest of the list

The items this document had been holding as "additive when something asks". Asked
2026-08-28.

### Spot cone angles

`SetLight` gained `inner_angle` and `outer_angle`. The stated reason for leaving
them out — a per-kind field makes the action a different shape depending on where
it lands — did not survive contact with `range`, which is point-and-spot only and
had already set the opposite precedent. Following it is the smaller inconsistency.

- **Radians on the wire, degrees in the box.** The payload and Bevy's `SpotLight`
  both store radians, so the action does too; a second unit for one quantity is the
  mistake avoided with `intensity`. But 0.2 rad is not a number an author thinks
  in, so the editor converts at the one place a human reads it.
- **The tween clamps inner to outer every frame.** Bevy renders *nothing at all*
  when the inner cone is wider than the outer one, so a tween crossing over would
  blink the light out mid-animation rather than show a narrowing cone. Reachable
  without authoring anything strange — widening inner to 0.45 while outer is still
  on its way down from 0.5 puts them the wrong way round for part of the run.
  Verified by deleting the clamp: inner reached 0.4345 against an outer of 0.4330.
- **The node inspector had no rows for them either**, which is how this surfaced:
  asked which lights have a cone, the answer was "spot lights" and then there was
  nowhere to point. `SpotLightSection` read `inner_angle`/`outer_angle` into state
  and sent them on every edit, but rendered only Color, Intensity and Range — so a
  spot light's cone was stuck on its 0.2/0.5 rad defaults for good. Two sliders
  now, in degrees, with the inner one following the outer down rather than letting
  an author invert the pair and black the light out. The `SetSpotLight` command
  clamps as well, because both readers of `pending_spot_light` — the live preview
  and the document commit — take the pair on trust.

### Slider value

`SetElementValue` gained a duration and an ease. **Only a Slider interpolates**: a
Toggle has no halfway between checked and unchecked, so it flips at the start
whatever the duration and reports finished at once — waiting on a tween that was
never inserted would block its agent forever. Same rule a texture swap follows.

The tween clamps to the slider's `min`/`max` **every frame**, not only at the
target, because the start value is read from the live slider and a range edited
mid-flight would otherwise draw the handle off its track for the rest of the run.

### Four actions had no editor at all

Found while building the above, and larger than it: `SetElementText`,
`SetElementValue`, `SetElementEnabled` and `PlayEffect` all carry parameters and
**had no editor block in the sequencer inspector**. They could be added to a row
and then never configured — each stuck for good on whatever
`default_action_for_kind` produced: an empty string, a zero, `false`, no count.

Giving `SetElementValue` a duration would have been one more unreachable field on
an unreachable action, so all four now have editors. The others (`StopEffect`,
`PlayAudio`, `StopAudio`, `StopVideo`) correctly have none, because they carry no
parameters.

### Fog and exposure are not a match arm

This document claimed they were: *"the same shape applies to anything with a
numeric value and a runtime setter — fog visibility, exposure"*. **That was
wrong**, and checking it was the cheapest part of this work.

Every `XrdsActionTarget` is a node or a panel element — `SelfNode`, `Node(id)`,
`TriggerSource`, `Element { panel, name }`. Fog and exposure live on the *scene
environment*, which is global and has no target. An action for them needs a new
target kind, which is a design decision about what a Track can address, not a match
arm and a tween component.

**Decided 2026-08-28: they are not getting an action at all.** A Track addresses
things in the scene and holds asset locks on them; a scene-wide setting has nothing
to address and nothing to lock, so a target for it means either a global lock — one
Track fading the fog blocking every other Track — or an exception to the policy that
keeps Tracks from interfering.

Note that "exposure" covers two things here, and only one was ever in question.
`XrdsSceneExposureEnvironment` is the scene-wide EV100, which is what this
paragraph refuses. `XrdsAnchorExposureOverride` is a component on an anchor, so it
has a node to hang on and needs no new target kind; it stays a backlog candidate.
See `docs/xrds-trigger-action-backlog.md`.

## What would come next, if something asks

Anything with a numeric value, a runtime setter, **and a node or element to hang
it on**. That last clause is the one this document got wrong once; see above.

## What stays out

**A keyframe curve editor.** Reconsider only when an author has a real case for a
value whose shape over time matters — not merely one that changes over time,
which sequenced interpolating events already cover.
