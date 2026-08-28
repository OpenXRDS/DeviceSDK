# Per-property tween channels — plan

**Deferred — see the last section.** Written 2026-08-28 after the question "should colour, intensity
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

| Phase | Scope |
| --- | --- |
| 1 | Channel model + merge-on-insert for **one** tween (`XrdsLightTween`), including per-channel completion and the agent token. Prove the shape on the smallest of the three. |
| 2 | Apply to `XrdsMaterialTween` and `XrdsTransformTween`. Mechanical once phase 1 settles the design — or it reveals the design does not fit `Transform`, which is the point of doing lights first. |
| 3 | Editor: nothing, if this is right. The test of the design is that two overlapping events compose with no inspector change at all. If phase 3 turns out to need UI, phases 1–2 chose wrong. |

Each phase lands with a test that **fails before it** — specifically the table
above, re-run as an assertion: two overlapping events, two tweens, both properties
moving.

## What stays out

- **Three durations inside one event.** The question that started this. Answered
  by composition instead, and adding both would give two ways to express one
  thing.
- **Blending two events on the *same* property** (a colour fade to red and one to
  blue, at once). Last-writer-wins per channel is correct and is what engines do;
  averaging two intentions produces a colour neither event asked for.
- **A keyframe curve editor**, still — see
  [`animatable-properties.md`](animatable-properties.md).

## Why this is worth doing rather than documenting

The cheap alternative is to write down "overlapping events on one node replace
rather than compose" and move on. That is wrong here for one reason: the
limitation is **silent**. An author gets no error, no warning, and a Track that
reports success — and the thing they lose is the animation they just authored. A
documented limitation an author can see is a trade; an undocumented one they
cannot is the failure this project keeps paying for, and it has now been shipped
three times in the same shape.

**Deferred 2026-08-28, with the diagnostic built instead.** The reasoning: the
harm here is the silence, not the limitation. Sequencing two events instead of
overlapping them is a modest workaround; losing a fade you authored, with no error
and a Track reporting success, is not. Nothing has asked for per-property curves —
the question that started this asked whether the shared curve was *right*, which
is a different thing — and the diagnostic cost an hour against a redesign of all
three tween types.

`Overlapping events cancel an animation` is now an **error** on any document where
it happens, naming the properties that are lost and the earliest time the second
event could start without losing them.

One correction to the paragraph this replaced, which had the rule wrong: it is
**not** "two events overlapping and touching the same property set". Overlapping
two events that animate the *same* property is correct authoring — retiming a fade
so it overtakes the previous one is last-writer-wins, and nothing is lost. The
rule that ships is narrower and is the accurate one:

> Report only the properties the earlier event animates and the later one does
> **not**. Those are the ones that stop part-way.

It is also scoped per tween family, because a light tween and a material tween are
separate components on the entity and genuinely do run side by side. And an
instant (`duration 0`) event inserts no tween, so it cancels nothing.

A diagnostic that fires on correct authoring is how you teach people to ignore
diagnostics — the same argument that kept a zero-duration warning out of this file
when `Teleport` was deleted.

**This plan stays open.** The diagnostic tells an author what they cannot do; the
channels work would let them do it. Pull this out when someone wants two
properties on two curves and being told "no" is the wrong answer.
