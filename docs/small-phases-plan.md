# Small phases — the 1-phase tier on the road to 1.0

**Status:** planned, none started. Written 2026-08-19.

Scope: the five **Small** items from the blueprint in `OVERALL_PROGRESS.md`
("Road to 1.0"). Each is meant to be one landable change with its own tests.
They are grouped here because they share a property worth stating plainly:

> Four of the five close a gap where something is *authorable but inert* — the
> author can set a value, it validates, it serializes, and nothing consumes it.

That is the same failure mode `docs/done/player-body-collider-plan.md` was
written about, and it is worse than a missing feature, because a missing feature
announces itself. This tier is mostly about not shipping 1.0 with any of those
left.

Suggested order: **S2 → S1 → S3 → S4 → S5**. S2 is the cheapest and makes every
subsequent device check more informative. S5 is last because it is source-breaking
and should not be interleaved with behavioural work.

---

## S1 — Spatial-audio parameters: honour them or delete them

**Status: DONE — implemented and verified by ear, 2026-08-19.** Desktop + AirPods,
via `cargo run --example spatial_audio_falloff_check`. Both axes confirmed by a
listener: distance attenuation follows the authored curve, and left/right placement
works. Not yet heard on a Quest; nothing in the change is Android-specific, but that
is an assumption, not a result.

**The listening pass was not a formality — it caught a bug the tests could not.**
See "What listening caught" below before trusting a green suite on anything audible.

**Crates:** `xrds-components`, `xrds-scene-graph`, `xrds-runtime` — one more than
planned. `XrdsAudioClip`, the *runtime* component, turned out not to carry the
falloff fields at all, which is the deeper reason both conversions dropped them.
`XrdsAudioDistanceModel` therefore moved to `xrds-components` (re-exported from
`xrds-scene-graph`), following the `XrdsGrabType` precedent, so the gain curve is
evaluable without the document layer and unit-testable without Bevy.

### The actual state

`XrdsSceneAudioClip` carries six spatial fields
(`crates/xrds-scene-graph/src/scene/payload.rs:1169-1187`):

| Field | Default | Consumed by anything? |
| --- | --- | --- |
| `spatial: bool` | `true` | **Yes** — `spawn.rs:926` → `PlaybackSettings.spatial` |
| `distance_model` | `Inverse` | No |
| `min_distance` | `1.0` | No |
| `max_distance` | `50.0` | No |
| `rolloff_factor` | `1.0` | No |
| `hrtf` | `false` | No |

`spawn_audio_clip_descriptor` (`crates/xrds-runtime/src/xrds_api/spawn.rs:914`)
builds `PlaybackSettings` from `looped`, `volume`, `autoplay` and `spatial`, and
reads nothing else. A `grep` for the other five names across `crates/` returns
only the payload definition, the deprecated `xrds-audio` crate, and documentation.

They have real doc comments (`/// Gain decreases linearly from min_distance to
max_distance`) which makes them read as supported. They are not.

`OVERALL_PROGRESS.md` described this as an editor-only gap until 2026-08-19. That
was wrong and is corrected there now.

### Why it got worse, not better

`xrds-audio` — the crate that would plausibly have honoured these — was assessed
against Bevy and deprecated on 2026-08-18 (`2149192`), and is `exclude`d from the
workspace in the root `Cargo.toml`. Whatever happens here has to happen on
`bevy_audio`, or not at all.

### The decision to make

Bevy's spatial audio is rodio's spatial sink: emitter/listener positions with
stereo panning and distance amplitude. It exposes no distance-model selection, no
min/max clamp, no rolloff exponent, and no HRTF. So the six fields split three
ways:

1. **`spatial`** — already honoured. Nothing to do.
2. **`distance_model`, `min_distance`, `max_distance`, `rolloff_factor`** —
   *implementable by us*, without patching Bevy: a system that each frame computes
   listener→emitter distance and sets the sink's volume from the authored curve.
   This is a real feature and not a large one.
3. **`hrtf`** — **not implementable** on this backend. It needs an HRTF-convolving
   audio path. There is no honest way to honour this field short of replacing the
   audio backend. (An earlier draft of this file said `hrtf` was "precisely the kind
   of thing `xrds-audio` was deprecated for attempting." **That was wrong** —
   `xrds-audio` never attempted HRTF; see "What the audio stack actually contains"
   below.)

**Recommendation:** implement (2), delete (3). A `hrtf: bool` that silently does
nothing is worse than no field, and "we will support it later" is what the current
state already is.

Deleting `hrtf` is a document-schema change. It is `#[serde(default)]`, so old
documents still load once the field is removed — the value is simply dropped. That
is acceptable *because nothing ever consumed it*; no scene can regress in
behaviour, only in a field that never did anything.

### What the audio stack actually contains (verified 2026-08-19)

Read the sources rather than trusting the summaries, because two of the summaries
in this repository were wrong.

**rodio 0.20.1** — `bevy_audio` is a thin ECS layer over it, so rodio is the real
ceiling. Its own manifest calls it an *"Audio playback library."* Its entire notion
of 3D is `src/source/spatial.rs`, ~80 lines, whose doc comment states the whole
algorithm: downmix to mono, then two per-channel gains from per-ear distance. The
gain is hardcoded:

```rust
let left_dist_modifier = (1.0 / left_dist_sq).min(1.0);
```

A fixed inverse-square law, clamped at 1.0 — no exponent, no max distance, no model
choice. `grep -niE 'hrtf|binaural|convolv|reverb|doppler|occlusion'` across all of
rodio's `src/` returns **nothing**. Biquad `low_pass`/`high_pass` exist (`blt.rs`),
as do `amplify`, `delay`, `crossfade`, `speed`, `agc` — stream effects, not a world
model. Bevy adds exactly one lever on top: `SpatialScale`
(`bevy_audio-0.17.2/src/audio.rs:205`), which scales world units before rodio sees
them.

**`xrds-audio` never implemented any of this.** At deprecation: 233 lines in
`audio.rs`, dependencies `cpal` and `log` only, and the `cpal` was for *output-device
enumeration* — the sound-card layer, not DSP. The version trimmed away in `2149192`
was a rodio `SpatialSink` wrapper whose every method was a one-line passthrough
(`set_emitter_position` → `self.sink.set_emitter_position`). No attenuation maths,
no HRTF, nothing lost in the trim. So the capability was never present in *either*
stack — nobody deleted 3D audio, it was never written.

**A doc that misleads, still in the tree.** `crates/xrds-audio/src/lib.rs:36-39`
tells the reader that `XrdsSceneAudioClip` "carries `spatial`, `distance_model`,
`min_distance`, `max_distance`, `rolloff_factor` and `hrtf`. **Use that.**" One of
the six works. Twelve lines earlier the same file warns against "the same
authorable-but-inert failure that made zone triggers look broken for two device
sessions." Fix or delete that paragraph as part of S1.

### Implementation wrinkle: our gain multiplies with rodio's

Because rodio's `1/d²` is unconditional, a volume we set from the authored curve
lands as `our_curve × (1/d²)` — so an authored `Linear` model would not be linear.
Either divide rodio's contribution back out, or use `SpatialScale` to push its
falloff far enough out to be negligible. Decide this before writing the system; it
is cheap to handle up front and expensive to discover halfway.

Incidental: that `.min(1.0)` clamp gives rodio an effective `min_distance` of exactly
1.0 world unit, which happens to equal our payload default.

### Scope note: attenuation is in 1.0, HRTF is not

Judged against the parity bar in `OVERALL_PROGRESS.md`:

- **Distance attenuation is a real parity gap.** Godot ships attenuation models on
  `AudioStreamPlayer3D`, Unreal ships attenuation curves, Unity ships rolloff
  curves — all core engine, no plugin. We have the fields and none of the
  behaviour. In scope.
- **HRTF is not.** In Unity and Unreal, binaural rendering is *plugin* territory
  (Steam Audio, Meta XR Audio SDK, Wwise); the engines themselves ship panning. Out
  of scope for 1.0.

Binaural was evaluated properly rather than left as a guess — see
**`docs/spatial-audio-backend-spike.md`**. Outcome, in short:

- **No backend swap.** `fyrox-sound` has the features but drags in `fyrox-core` +
  `fyrox-resource` and a second output stream; `kira` integrates cleanly but its
  spatial support is volume-and-panning only, i.e. no better than today.
- **If binaural is ever needed**, the path is the standalone [`hrtf`](https://docs.rs/hrtf)
  crate behind a custom `Decodable` — a supported Bevy extension point, verified.
- **Therefore S1 is not throwaway work.** An earlier draft of this plan assumed a
  backend swap might make it so. It will not.

One finding from the spike changes the wrinkle above: spatialization is selected
per-sink at `bevy_audio-0.17.2/src/audio_output.rs:126`, so `spatial: false` gives a
plain sink with **no rodio spatialization at all**. If S1's attenuation is ever
moved into a custom `Decodable`, the multiply-on-top problem disappears entirely
rather than needing to be divided out. Not required for S1 as scoped — noted so the
simpler fix is not mistaken for the only one.

### What listening caught

Two problems survived a green test suite and a clean-looking log. Both are the
reason `examples/xrds_first/spatial_audio_falloff_check.rs` exists and should be
kept.

**1. A "fix" that flattened the panner while making the numbers perfect.**
Rodio's per-ear `dist_modifier` — `(1.0 / dist_sq).min(1.0)` — *is* its panning:
the nearer ear is closer, so it gets more gain. Its other term, `diff_modifier`,
spans only `0.5..=1.0` and is **inverted** (the nearer ear gets the *smaller*
value); it fights the panning and is normally simply overpowered.

An intermediate revision set a per-clip `SpatialScale` to pin `dist_modifier` at
its clamp, believing the *other* term was the panner. That inverted the design: it
flattened the panner and left only the weak backwards term, and the stereo image
collapsed to centre. The falloff diagnostics during that revision were
**flawless** — `sink_volume` equalled the authored gain to three decimals, better
looking than the correct implementation's — so nothing in the log or the tests
indicated a problem. A listener said "it plays on both ears" and that was the only
signal. There is now a `rodios_near_ear_stays_louder_than_the_far_ear` regression
test.

**2. A check that could not test what it claimed to.** The first version of the
example only pulled straight back from the sources. That destroys direction two
ways at once: the sources' angular separation shrinks toward zero as you retreat,
*and* rodio's panning independently collapses with distance — roughly 22 dB between
the ears at 3 m, about 1 dB at 10 m. It was asking a listener to judge the stereo
image precisely where it cannot exist. The example now alternates a DISTANCE phase
(pull back and return) with a DIRECTION phase (a lateral pass at 2 m).

**Also worth keeping:** the first run used `transportation_1.wav`, and the listener
reported it "sounds like just playing a sample file". Every clip in `assets/sound/`
is stereo and ~20 s long; rodio downmixes a spatial source to mono, so a recording
with its own baked-in stereo movement — that one is a train passing, which pans by
itself — actively fights the cue under test. `assets/sound/wav/spatial_test_ping.wav`
was generated for this: mono, 1 s loop, four broadband bursts with sharp onsets,
because onsets are what the ear localises with.

### The accuracy limit that remains

The correction divides rodio's law out using the distance to the listener's
**centre**, while rodio works per **ear**, and Bevy's default ear gap is 4.0 world
units. So the level is monotone and hits silence exactly at `max_distance` — what
the authored fields promise — but is not a calibrated absolute. Panning is
unaffected: the sink volume is one scalar across both channels and cannot alter
their ratio. Closing this properly means abandoning rodio's spatialization for a
panner of our own; see `docs/spatial-audio-backend-spike.md`.

### Watch for this

`payload.rs:1678-1684` converts a legacy audio shape and ends with
`..Default::default()`, which means it resets `distance_model`, `min_distance`,
`max_distance`, `rolloff_factor` and `hrtf` to defaults rather than carrying
authored values. Harmless today (nothing reads them). The moment S1 lands it
becomes a real bug: audio through that path would silently ignore its authored
falloff. **Fix the conversion in the same change.**

### Steps

1. Decide and record: implement the four distance fields, delete `hrtf`.
2. Add a runtime attenuation system in `xrds-runtime` driving sink volume from
   `distance_model` / `min_distance` / `max_distance` / `rolloff_factor`, applied
   only when `spatial` is true.
3. Fix the `..Default::default()` conversion at `payload.rs:1678`.
4. Remove `hrtf` from the payload, its default fn, and its `Default` impl. Fix or
   delete the misleading paragraph at `crates/xrds-audio/src/lib.rs:36-39` in the
   same change, and record binaural as a backend decision with named candidates so
   the removal does not read as "nobody thought about it".
5. Tests: unit-test the gain curve for `Linear` and `Inverse` at
   `d < min`, `d == min`, mid-range, `d == max`, `d > max`. Extend the audio
   round-trip test in `crates/xrds-runtime/src/tests/document_roundtrip.rs` (which
   currently asserts only `spatial`) to cover the four surviving fields.
6. Only then the inspector UI, which is editor work and not part of this phase.

### Done when

An authored `min_distance`/`max_distance`/`rolloff_factor`/`distance_model`
measurably changes what a listener hears, and no audio field exists that nothing
reads.

---

## S2 — Zone-event logging

**Status: done 2026-08-19.** Not yet exercised on a device — the next zone-related
device pass is what confirms it, and nothing else needs to happen first.

**Crate:** `xrds-runtime`

### Changed from the plan: `info!`, not `debug!`

This item was written as "`debug!` logging". That would have been useless for its
own stated purpose. The deployed runtime configures `Level::INFO`
(`runtime.rs:178`), so a `debug!` line never reaches a headset's logcat — and the
headset is the only place this matters. Discovered during the S1 device pass, where
the falloff diagnostics were written at `debug!` and were duly invisible.

Zone enter/exit are discrete and infrequent, so `info!` costs nothing and matches
the existing `GRABBED` precedent. The dropped-collision path logs `info!` **once**
and `debug!` thereafter: unregistered entities overlapping a zone is legitimate, so
warning on each would train people to ignore it, but a single breadcrumb is what
turns "nothing happened" into "the id was missing".

### Why this one is worth a phase at all

`zone_collision_system`
(`crates/xrds-runtime/src/xrds_api/zone.rs:10`) writes a zone event only inside
an `if let` over **two** id lookups:

```rust
if let (Some(zone_id), Some(entity_id)) = (id_index.id_of(e1), id_index.id_of(e2)) {
    enter.write(XrZoneEnterEvent { zone_id, entity_id });
}
```

When either lookup returns `None`, the collision is dropped **with no trace at
all**. That is exactly the shape of the bug that cost a Quest 3 device pass: the
collision was happening, the id was missing, and nothing anywhere said so. The
fix (giving the player an `XrdsId`) landed, but the *silence* did not — the same
class of failure will be equally invisible next time, e.g. for any entity spawned
by host-app code rather than the authored-node path.

### Steps

1. `debug!` on each successful enter/exit, with zone id and entity id.
2. `debug!` — **not** `warn!` — on the dropped-collision branch, naming which side
   failed to resolve and its `Entity`. Unregistered entities colliding with zones
   is legitimate (scenery, debris), so this is not a warning; it is the breadcrumb
   that turns "nothing happened" into "the id was missing".
3. Confirm the target filter used by the Quest recipe surfaces it — see
   `docs/quest-device-test-recipe.md`.

### Done when

A device pass can distinguish "no collision occurred" from "collision occurred,
id unresolved" from the log alone, without a rebuild.

---

## S3 — Head-locked interactive-template diagnostic

**Status: done 2026-08-19.** Reported as an **Error**, not a Warning: the scene
loads, the panel renders and its buttons work, and the wearer simply cannot point
at anything else — a failure with no visible cause. Editor enforcement (greying the
template out in the picker) is still open and now has one definition to enforce.

**Crate:** `xrds-scene-graph` · SDK half of `OVERALL_PROGRESS.md` §4.

### As built

- `XrdsSceneDocument::is_head_locked(node_id)` in `document/core.rs` — walks
  **ancestors**, since head-locking is expressed by parenting rather than a flag.
  Cycle-guarded with a visited set: diagnostics run on documents just loaded from
  disk, and a malformed parent chain must terminate rather than hang the editor.
- `XrdsPanelTemplate::has_interactive_element()` in `scene/panel.rs`, beside the
  existing `XrdsPanelElement::is_interactive()`.
- The check itself in `panel_diagnostics.rs`, naming the offending elements so the
  author knows which ones to move.

Five tests, including the two that would otherwise rot: detection **through
intermediate grouping nodes** (the arrangement an author is most likely to build,
and the one a parent-only check waves through) and a parent cycle that must return
rather than spin.

### The policy, already decided

A panel's pointer capture is per-panel, not per-element — settled as policy, not a
bug, matching how visionOS and Quest treat a window. The accepted consequence: a
template with an interactive element, head-locked to a `PlayerAnchor`, captures the
pointer wherever it sits in the wearer's view, **permanently**. A world panel only
captures when approached; a HUD never stops.

Decided policy: a head-locked `Panel` should link only to info-only templates
(`Label` / `Image`). Today an author can head-lock an interactive template with no
warning of any kind.

### Where it goes

`crates/xrds-scene-graph/src/document/panel_diagnostics.rs` already exists for
exactly this class of check, already reports `XrdsSceneTriggerDiagnostic` (a shape
the editor renders today), and already provides `all_trigger_bindings()` which
walks both a node's own triggers and each Panel node's `element_triggers`. Use it
— do not add a parallel walk.

"Head-locked" is determinable from the document: a `Panel` node whose ancestor
chain reaches a `PlayerAnchor` payload (`payload.rs:24`). Note it is *ancestor*,
not *parent* — a Panel nested under an intermediate node under a `PlayerAnchor` is
still head-locked, and a check that only looked at the immediate parent would pass
the exact scene an author is most likely to build.

### Steps

1. Helper: does this node's ancestor chain reach a `PlayerAnchor`?
2. Diagnostic: head-locked `Panel` whose template contains any interactive element
   (`Button` / `Slider` / `Toggle`).
3. Tests: head-locked + interactive → diagnostic; head-locked + Label/Image only →
   none; world panel + interactive → none; **nested under an intermediate node**
   → diagnostic (the case a parent-only check would miss).

Editor enforcement — greying out and refusing such templates in the picker behind
`SetPanelInstanceTemplate` (`apps/xrds-editor/src-tauri/src/panel_library.rs:431`)
— is **separate editor work** and follows this. The diagnostic is what makes it
enforceable; ship the diagnostic first so the rule has one definition rather than
two.

### Done when

Loading a document that head-locks an interactive template produces a diagnostic
the editor can already display.

---

## S4 — Passthrough blend-mode toggle

**Editor only.** Listed here for completeness of the tier; it is the one Small
item with no SDK half remaining.

`XrdsXrBlendMode` (`Opaque` / `AlphaBlend`) exists at
`crates/xrds-scene-graph/src/scene/node.rs:20`, is serialized with a
`skip_serializing_if` default, and the runtime already handles `EnvironmentBlendMode`
plus the `fb_passthrough` extension. Only the editor control is missing.

### Steps

1. Inspector control on the scene/root settings surface, bound to `xr_blend_mode`.
2. Editor command + undo entry, following an existing scene-level setting.
3. Device check on Quest 3 — `AlphaBlend` should show passthrough behind the scene.
   Desktop cannot verify this; do not mark it done on a desktop run.

### Done when

An author can switch a scene to passthrough from the editor and see it on device.

---

## S5 — Naming polish

**Crate:** `xrds-scene-graph` · **Source-breaking. Land alone, last.**

From `OVERALL_PROGRESS.md` §3, both long-standing:

- `TransformParams::rotation_quat_xyzw` and `rotation_euler_xyz_deg` — a dual
  field whose precedence has been *clarified in prose but not resolved
  structurally*. Two ways to say the same thing, with a rule you must read docs to
  learn, is precisely what the non-expert-first principle exists to prevent. The
  structural fix is an enum with one variant per representation, so the type makes
  the rule unstatable-wrongly.
- `*Patch` types (`NamePatch`, `ParentPatch`, …) — ECS jargon on a surface aimed at
  non-experts. Currently hidden behind typed helpers, which is a mitigation, not a
  fix.

### Why last

This is mechanical but touches call sites broadly, including the editor bridge. Run
it against a clean tree so a compile break is unambiguously from the rename and not
from S1–S4. Nothing else in this tier depends on it, and it changes no behaviour —
so it is also the safest item to drop if the tier needs to be cut short.

### Steps

1. Replace the dual rotation fields with a single representation enum; migrate
   `serde` so existing documents still load.
2. Round-trip test: a document authored with euler, and one authored with quat,
   both load and re-serialize unchanged.
3. Rename `*Patch` on the public surface; keep the ECS-facing names internal.

---

## S6 — Audio authoring in the editor

**Status: not started. Added 2026-08-19, after S1 shipped and the device pass
showed what authoring these values actually requires.**

**Size: 2–3 phases — Medium, not Small.** It sits in this document because every
finding it depends on is here, but it does not belong to the 1-phase tier and
should not be estimated as though it did. Sized honestly in
`OVERALL_PROGRESS.md`'s blueprint.

### The gap is total, not partial

`Inspector.tsx` has **no `AudioClip` section**, and `src-tauri/src/bridge.rs` has
**no audio commands at all**. An author can place an Audio Clip node from the
palette and then change nothing about it — not `distance_model`, `min_distance`,
`max_distance` or `rolloff_factor`, and not `volume`, `looped`, `spatial` or
`autoplay` either. Everything verified on device during S1 was authored in Rust,
through `gen_device_check_scene.rs`.

So this is not "add rows to the audio panel". There is no audio panel. It is a new
inspector section plus the bridge commands underneath it.

**Why it matters enough to be tracked rather than assumed:** spatial audio is now
genuinely working on device — and it is reachable only from code. For a
non-expert-first SDK that is the same authorable-but-inert failure S1 existed to
fix, moved up one layer: the capability is real and the author cannot get at it.

### What to build, in order

**1. Inspector section + bridge commands.** Model dropdown and three numeric
fields, plus the four basic fields that are equally unreachable today. Unglamorous
and a hard prerequisite — nothing below works without the commands.

**2. Radius gizmos in the viewport.** Two wireframe spheres on the selected audio
node, at `min_distance` and `max_distance`, draggable. This is the part that makes
the feature authorable rather than guessable, and it is what every comparable
engine does: Unity draws min/max distance spheres on an AudioSource, Unreal draws
the attenuation shape, Godot the emission sphere. These are *spatial* quantities;
typing `15` into a box says nothing about whether that reaches the far wall.

Worth scheduling alongside whatever eventually draws `InteractionZone` bounds —
same machinery, and zones have the same "invisible volume" problem (recipe Trap 6).

**3. Curve preview in the inspector — and plot it in dB, not amplitude.**

This one earned its place during the S1 device pass rather than being a nice-to-have.
Two things cost real rebuild cycles that a graph would have made obvious:

- `min_distance` is the **reference radius**, not merely a near clamp. Raising it
  flattens the whole curve, not just the part below it. It is the first knob to
  reach for when a sound dies too fast — and nothing about the name says so.
- **Linear-in-amplitude collapses in loudness.** `Linear` from 0.2 to 0 over the
  final metre is a smooth line on paper and a fall from −14 dB to silence in the
  ear. Plotted in amplitude it looks fine; that is precisely why the first two
  attempts sounded, in the listener's words, "a bit dumb" and then "too hard".

A preview drawn in amplitude would reproduce the same illusion that misled the
implementation. Draw decibels.

**4. Audition in the editor.** If play mode moves the listener and the falloff can
be heard in the viewport, the loop closes without an APK build. During S1 the only
way to judge a curve was a four-rebuild device cycle — roughly an hour per
adjustment for a change that is one number.

### Done when

An author can place an audio clip, see its reach in the viewport, adjust the curve
against a preview that reflects what the ear will do, and hear the result without
building an APK.

## What this tier does not include

- **The documentation pass.** Deliberately held until after this tier — S1, S3 and
  S4 all add or remove inspector surface, and the GUI manual would document an
  editor that is about to change. See the blueprint's "Deliberately not in the
  blueprint" section.
- **Editor enforcement for S3** and **the S1 inspector UI** — both are the editor
  halves that follow their SDK phases.
- Anything from the Medium or Large tiers.
