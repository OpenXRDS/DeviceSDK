# Per-property tween channels — plan

**Done 2026-08-28.** All three phases. Written the same day, after the question "should colour, intensity
and range each get their own curve?" turned up a defect underneath it.

## The question, and the answer that is not the interesting part

Should `SetLight` carry three durations — one for colour, one for intensity, one
for range — so each property can have its own curve?

**No.** One duration per event is the conventional shape for an event model: an
event is one change, at one time, over one duration. Three duration+ease pairs
inside one event makes the inspector noisy for a rare case, and it is the keyframe
curve editor this project deliberately declined, arriving through a side door.

The right way to give two properties different curves is **two events** — one
setting only colour over 2s, another setting only intensity over 5s. Fields are
already independently optional, so that should need no new code at all.

## The defect: overlapping events silently cancel each other

It does not work, and the failure is invisible.

A tween lives on the target entity as a component, and a second event **inserts
over it**. Bevy replaces the component; the first tween is gone, along with
whatever it was animating. Measured, not reasoned about — two overlapping events
fired at the same timestamp on one node:

| | Tweens on the entity | First event's property | Second event's property |
| --- | --- | --- | --- |
| Two `SetLight` (colour 30s, intensity 30s) | **1** | green stuck at 1.0 — never moved | intensity 1000 → 999.34, running |
| Two `SetMaterial` (colour 30s, roughness 30s) | **1** | green stuck at 1.0 — never moved | roughness 0.5 → 0.4996, running |

`XrdsTransformTween` has the same shape — it stores a whole `start`/`target`
`Transform` — so two overlapping `SetTransform` events behave the same way. Not
separately measured; the mechanism is identical and the struct is on the page.

Nothing reports any of this. The Track completes normally, both agents finish, and
one of the two fades simply never happened. It is the project's recurring failure
in its purest form: authored, saved, previewed, silent.

Worth noting that `XrdsTransformTween`'s own doc comment says it lives on the
target "so overlapping/back-to-back tweens on **different targets** never
collide". It is precise about the case it handles and silent about the one it does
not — the same target — which is exactly the case that breaks.

Two keys at the same timestamp genuinely do run at once: the track agent fires
keys by comparing `at_secs` against elapsed time, not by waiting for the previous
key to finish, and each key gets its own ephemeral agent. So this is reachable
authoring, not a theoretical arrangement.

## The fix: channels, not components

Give each animatable property its own channel inside the tween — start, target,
elapsed, duration and ease per property, rather than per component. Inserting a
new tween then **merges**: it overwrites only the channels its event names, and
leaves the rest running.

That buys three things at once:

- **Overlapping events compose.** A colour fade continues while an intensity fade
  starts, which is what every tween library does — DOTween's `DOColor` and
  `DOIntensity` are independent tweens on one object, not one tween that clobbers
  the other.
- **Per-property curves become authorable with no new UI.** The answer to the
  original question turns out to be a bug fix, not a feature.
- **A second event on the same property replaces just that channel**, which is the
  correct last-writer-wins, and is what an author expects when they retime a fade.

### The wrinkle worth getting right

`is_finished` currently polls for component *absence*:

```rust
XrdsAction::SetLight { .. } => world.get::<XrdsLightTween>(self.target).is_none(),
```

With merged channels that becomes wrong in a way that is easy to miss: a colour
event finishing at 2s would stay "unfinished" until an unrelated intensity channel
finished at 5s, holding its row lock for three seconds it does not need. Long
enough to reject another Track under the reject-the-newcomer policy.

So each ephemeral agent has to wait on **its own** channels, not on the component.
That means the agent needs to know which channels it started — a small token
(a generation counter per channel, checked by the agent) rather than a presence
check. This is the part of the work that is design rather than typing, and it is
where a shortcut would produce a Track that stalls or a lock released early.

## Phases

| Phase | Scope | State |
| --- | --- | --- |
| 1 | Channel model + merge-on-insert for `XrdsLightTween`, with per-channel completion and the agent generation | ✅ built |
| 2 | The same for `XrdsMaterialTween` and `XrdsTransformTween` | ✅ built |
| 3 | Editor: nothing, if this is right | ✅ nothing was needed |

### Phase 1 — lights

`XrdsLightTween` holds three independent channels — colour, intensity, range —
each with its own start, target, elapsed, duration, ease and generation. An event
**merges**: it overwrites only the channels it names.

Four things worth keeping:

- **The generation is load-bearing, and that was proven rather than assumed.**
  Reverting `is_finished` to the old component-absence check and re-running the
  test leaves **2** agents waiting instead of 1 — the short colour event's agent
  held its row lock for the full thirty seconds of an unrelated intensity fade,
  long enough to reject another Track under the reject-the-newcomer policy.
- **A tween writes only the properties it animates.** The first version reused the
  full-reading setter, which would have let an intensity channel re-assert a stale
  colour every frame and undo a colour channel beside it — the same composition bug
  one layer down. `LightWrite` exists to make "leave this alone" expressible.
- **Instant events write only their own fields too**, so a duration-0 event next to
  a running fade no longer freezes it.
- **The component is removed only when every channel is spent.** Removing it when
  the first finished would drop the others mid-flight: the original bug rebuilt.

### Phase 2 — materials and transforms

Mechanical after phase 1, with two exceptions that were not:

- **Rotation needs its own channel type**, because it must **slerp**. Lerping two
  quaternions component-wise cuts through the inside of the sphere and speeds up
  through the middle of the turn. The generic numeric channel would have compiled
  and rotated wrongly.
- **A texture-only `SetMaterial` now inserts no channels**, since a texture never
  interpolates. Before, such an event always got a tween; now it must report
  finished immediately or its agent waits forever on one that was never created —
  a failure that looks like a hang rather than a wrong colour. There is a test.

Both composition tests were checked against phase-1 code first and **fail there**
(`green = 1` — the colour cancelled), which is what makes them worth having.

### Phase 3 — the editor

Nothing. That was the stated test of whether the design was right: two overlapping
events compose with no inspector change at all.

### The diagnostic is gone

`Overlapping events cancel an animation` was built while this plan was deferred, to
make the silent failure visible. All three families compose now, so it would report
an error on authoring that works — worse than the silence it was raised to fix. It
is deleted, along with `tween_family` and `animated_properties`, which existed only
to serve it. What replaced it are the runtime tests that assert the composition
directly.

## What stays out

- **Three durations inside one event.** The question that started this. Answered
  by composition instead, and adding both would give two ways to express one
  thing.
- **Blending two events on the *same* property** (a colour fade to red and one to
  blue, at once). Last-writer-wins per channel is correct and is what engines do;
  averaging two intentions produces a colour neither event asked for.
- **A keyframe curve editor**, still — see
  [`animatable-properties.md`](animatable-properties.md).

## Why this was worth doing rather than documenting

The cheap alternative was to write down "overlapping events on one node replace
rather than compose" and move on. The reason that was wrong is that the limitation
was **silent**: no error, no warning, a Track reporting success, and the thing lost
was the animation just authored.

It was in fact deferred once, with a diagnostic built instead. Asked why, the
honest answer was that one of the three reasons did not hold — "nothing has asked
for per-property curves" was wrong, because the question that found the bug *was*
the ask, read narrowly. And the diagnostic had converted an invisible failure into
an enforced restriction: better, but it was forbidding the very thing this plan
argues is the right way to author two curves. The work followed the same day and
came to less than the estimate, because phase 1 carried the design and phases 2–3
mostly carried it out.
