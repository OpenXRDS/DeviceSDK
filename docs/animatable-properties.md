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
  on the same entity.

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

## What would come next, if something asks

The same shape applies to anything with a numeric value and a runtime setter — fog
visibility, exposure, a panel element's value. Each is a match arm and a tween
component, not a new system.

A spot light's cone angles (`inner_angle`/`outer_angle`) are animatable in
principle and were deliberately left out: they apply to one of the three kinds, and
including them would make the action a different shape depending on where it
landed. Additive when something asks.

## What stays out

**A keyframe curve editor.** Reconsider only when an author has a real case for a
value whose shape over time matters — not merely one that changes over time,
which sequenced interpolating events already cover.
