# Spatial audio backend — evaluation spike

**Status:** desk research complete, **device verification not done**. The
recommendation below is actionable but two of its three remaining unknowns need a
Quest 3. Written 2026-08-19 on branch `road-to-v1.0`.

**Why this exists:** `XrdsSceneAudioClip` carries five spatial fields that nothing
reads (see `docs/small-phases-plan.md` S1). Before implementing them on the current
stack, we checked whether the current stack is the one we want — because building
attenuation on a backend we are about to replace is wasted work.

## Gates, in priority order

Set deliberately, and the ordering is the point:

1. **Platform support — hard gate.** Windows, Linux, and **Quest 3 / Android
   (`aarch64-linux-android`)**. A missing feature is work; an unsupported platform
   is a dead end. This question goes first, not last — asking it last is how
   `bevy_hanabi` got adopted and then rejected when it rendered nothing on Adreno.
2. **Integration cost — hard gate.** What else does adopting it drag in, and what
   does it break that currently works?
3. **Feature completeness — soft.** If HRTF or distance models are missing but the
   first two gates pass, we can build them.

## Candidates

### fyrox-sound — passes gate 1, **fails gate 2**

Feature-complete on paper: HRTF with IRCAM-derived HRIR spheres, reverb, streaming.
Android via `tinyaudio` → **AAudio, API 26+**, comfortably under our API 32 baseline;
Fyrox's own account is that `tinyaudio` builds cleanly on Android where `cpal`
struggles.

It fails on integration, for three independent reasons — any one would be
disqualifying:

- **It depends on `fyrox-core` and `fyrox-resource`.** That pulls Fyrox's *own
  resource-management system* into an app that already has Bevy's asset system. Two
  resource systems, two loaders, for one audio path.
- **It owns its own output stream** via `tinyaudio`. Adopting it alongside
  `bevy_audio` means two audio devices open at once — "two audio stacks competing for
  one output device", which is the stated reason `xrds-audio` was deleted in
  `2149192`, one level up in cost. Avoiding that means disabling `bevy_audio`
  entirely and re-solving asset loading plus the decoder-panic guard
  (`pre_validate_audio_decoders_system` and its `catch_unwind`, which exists because
  rodio panics on unrecognised formats).
- **Format narrowing:** WAV + OGG/Vorbis, against the `wav`/`ogg`/`flac`/`mp3` the
  current loader accepts (`bevy_audio-0.17.2/src/audio_source.rs:11`).

Noted for the record in case this is ever revisited: `tinyaudio` requires the audio
device be created **only after the app gains focus** (`GainedFocus` in
`android-activity`), or device creation fails. That constraint lands squarely on our
patched `bevy_winit` and the open, unreproduced Android window-lifecycle crash —
i.e. on the least stable part of the Android stack.

### kira — passes both gates, but solves nothing

`cpal`-backed, so its Android story is the one already shipping. `bevy_kira_audio`
is an established integration, so gate 2 is cheap. But its spatial support is
**volume and panning only** — the same ceiling as rodio. Adopting it would be a
large change that leaves us exactly where we started.

### The `hrtf` crate alone — the option that was not on the list

`fyrox-sound`'s HRTF is not Fyrox-specific: it re-exports [`hrtf`](https://docs.rs/hrtf),
a **standalone crate** with three pure-Rust dependencies (`byteorder`, `rubato`,
`rustfft`). No output device, no resource system, no engine. It is a buffer
processor — feed it samples plus an HRIR sphere (IRCAM database), it convolves via
overlap-save in the frequency domain.

So the one capability worth having can be obtained **without adopting Fyrox at
all**. Best possible answer on gate 2, and low risk on gate 1 since there is no
platform-specific code to fail.

Two caveats from its own documentation, both real:

- **"HRTF is heavy"** — FFT plus convolution plus memory traffic, per sound. On
  Quest's mobile CPU this is a budget question to be measured, not assumed.
- Known audible artifacts on rapidly moving sources.

## The gating question, answered: yes, Bevy is extensible here

The whole `hrtf`-on-Bevy option rested on one unknown — *can a custom processing
stage be injected into `bevy_audio`'s pipeline without forking it?* It can. This is
a supported public extension point, not a workaround:

- `pub trait Decodable` (`bevy_audio-0.17.2/src/audio_source.rs:82`) — implement it
  on our own asset type. Its associated `Decoder` need only be a `rodio::Source`,
  which is any iterator of samples. An HRTF-convolving wrapper around
  `rodio::Decoder` satisfies it.
- `pub trait AddAudioSource` with `app.add_audio_source::<T>()`
  (`audio_source.rs:106`) registers it.
- Bevy uses this path itself for a second built-in source: `lib.rs:99` registers
  `AudioSource`, `lib.rs:103` registers `Pitch`. Bevy also ships an example for it
  (`examples/audio/decodable.rs`).

**A second finding that matters more than it first looks.** Spatialization is chosen
per-sink at `audio_output.rs:126`:

```rust
if settings.spatial {  // SpatialSink::try_new(...)  else  Sink::try_new(...)
```

So `spatial: false` yields a plain sink with **no rodio spatialization at all**. A
custom `Decodable` therefore gets a clean signal path, and the
"our-gain-multiplies-with-rodio's-hardcoded-`1/d²`" wrinkle recorded in
`small-phases-plan.md` disappears — we would own the entire gain path rather than
fighting one we cannot configure.

**The real engineering problem on this path** is not the extension point but the
thread boundary: a `rodio::Source` is pulled by the audio thread and knows nothing
about entity transforms, while emitter and listener positions live in the ECS and
change every frame. Bridging that needs shared lock-free state written by a Bevy
system and read by the audio thread. Standard, but it is the part to design
carefully, and it is where the CPU-budget question also lives.

## Consequence for S1 — an earlier conclusion reversed

It was previously agreed in conversation that S1's attenuation work would be
throwaway if a backend swap were coming. **On this evidence no backend swap is
coming:** the only candidate with the features fails the integration gate, and the
only one that passes it has no better spatial audio than we already have.

So S1 is back on the critical path. The path forward is: keep `bevy_audio`, write
our own attenuation, and add `hrtf` as a processing stage **only if** binaural
proves necessary and its cost on Quest proves acceptable. That also matches the gate
ordering — the features are ours to develop, the platform was never negotiable.

## Front/back and elevation — what is reachable, in order of cost

Rodio emits one gain per ear. A source 30° front-left and one 30° back-left produce
**identical** ear gains — the cone of confusion — and a source directly overhead is
equidistant from both ears, so it lands dead centre. No amount of work on our side
changes that; the information is not present.

**Rung 1 — head tracking. Free, and already wired.** Front/back ambiguity is largely
resolved by *movement*: turn your head and a front source drifts one way in the
image while a rear source drifts the other. Listeners do this unconsciously. It is
already live — Bevy builds ear positions with
`transform.transform_point(settings.left_ear_offset)`, the **full** transform
including rotation (`audio_output.rs:73`), and `update_listener_positions` re-runs on
`Changed<GlobalTransform>`. Every XRDS camera is the listener, so head rotation
already repans every source. **Untested on device**, and invisible on desktop, where
the check example drives a camera that never rotates. Listen on a Quest before
concluding anything about front/back. Does little for elevation.

**Rung 2 — spectral fakery. Skip it.** A low-pass on rear sources reads as "behind"
(the head shadows highs); rodio has `low_pass` in `blt.rs`. But it needs the custom
`Decodable` path anyway — the same plumbing rung 3 needs — and buys an impression
rather than placement. If the plumbing is being built, build the real thing.

**Rung 3 — HRTF.** Direction-dependent pinna filtering is the only true elevation
cue, and the only real front/back cue that survives a stationary head. Gated on the
affordability question below.

### Trap: rodio's panning depends on an unrealistic ear gap

It works only because Bevy defaults to a **4.0 world-unit** ear gap. With a
realistic head width (~0.18 m), a hard-left source at 2 m computes to left 0.139
and right 0.227 — **the panning inverts**, because the weak, backwards
`diff_modifier` overpowers the now-negligible per-ear distance difference. Anyone
"correcting" the ear gap to human scale will silently break stereo.

## Affordability must be measured before HRTF is planned

Audio carries a hard real-time deadline that rendering does not. A missed frame is
jank; a missed audio buffer is an audible click, every time, unrecoverable. At
48 kHz with 512-sample buffers that is **~10.6 ms for every source combined**, on a
CPU already running the render thread. `hrtf`'s own documentation says "HRTF is
heavy" — FFT plus overlap-save convolution plus memory traffic, *per source*.

So cost belongs inside gate 1, not after it: "runs on Quest" has to mean "runs on
Quest inside the audio deadline". This is the same lesson as `bevy_hanabi`, which
was adopted and then rejected once it met Adreno.

**It is cheap to answer, and answerable before any integration.** The `hrtf` crate
is standalone — no Bevy, no `Decodable`, no runtime changes. A small binary that
convolves N sources in a loop, cross-compiled to `aarch64-linux-android` and run
over `adb shell`, settles both open unknowns at once. Roughly an hour, and it
requires committing to nothing.

**Expect a budget, not a yes/no.** The likely outcome is "affordable for N
simultaneous sources, not for 3N", which is a design input rather than a verdict —
and it is exactly how commercial spatializers behave: HRTF the nearest N voices,
amplitude-pan the rest. That N is the number to measure for.

## Remaining unknowns

| # | Question | Needs |
| --- | --- | --- |
| 1 | ~~Can a custom stage be injected into `bevy_audio`?~~ | **Answered: yes**, `Decodable` + `add_audio_source`. |
| 2 | How many HRTF sources fit in the audio deadline on Quest? | Standalone bench over `adb`; see above |
| 3 | `hrtf` cross-compiles and runs on `aarch64-linux-android` | Same bench answers this; expected fine (pure Rust) |
| 4 | Does head tracking alone give usable front/back? | Quest 3, a listener, no code — rung 1 above |

Unknown 4 is free and should be checked first: it may close the front/back gap
without HRTF entering the picture at all. Unknowns 2 and 3 gate any HRTF *plan* —
they are not implementation details to discover afterwards. None of the four blocks
S1, which is done.

## What rodio's spatialization actually does (measured 2026-08-19)

Established while implementing S1, by reading `rodio-0.20.1/src/source/spatial.rs`
and then listening. None of it is in rodio's documentation, and it bounds what can
be promised about direction without a backend change.

- **`dist_modifier` — the per-ear `(1.0 / dist_sq).min(1.0)` — is the pan law.**
  There is no separate one. The nearer ear is closer, so it gets more gain.
- **`diff_modifier` is weak and inverted.** It spans `0.5..=1.0` (6 dB) and gives
  the ear *nearer* the source the *smaller* value. It opposes the panning and is
  normally overpowered by `dist_modifier`. Anything that flattens `dist_modifier`
  leaves this term in charge and the image collapses — or worse, inverts.
- **Panning strength collapses with distance.** A source hard left at 3 m gives
  roughly 22 dB between the ears; the same source at 10 m gives about 1 dB, because
  `dist_modifier`'s ratio tends to 1 while the inverted `diff_modifier` does not.
  Rodio pans convincingly up close and barely at all far away.

Consequence for the decision below: rodio is adequate for *distance* and for a
near-field *left/right* image, both confirmed by a listener. It is not adequate for
placement, for far-field direction, or for elevation and front/back — and no amount
of work on our side changes that, because there is nothing to configure.

## Recommendation

1. **Do not swap the audio backend.** Neither candidate is worth its integration
   cost.
2. **Proceed with S1 as planned** on `bevy_audio` — implement the four distance
   fields, delete `hrtf` from the payload. Deleting the field does not delete the
   option: this document records how binaural would be added, which is more than
   the field ever conveyed.
3. **If binaural becomes a requirement**, the path is the `hrtf` crate behind a
   custom `Decodable` with `spatial: false`, and unknowns 2 and 3 must be answered
   on device *before* any of it is written.

## Sources

- <https://docs.rs/fyrox-sound> · <https://lib.rs/crates/fyrox-sound>
- <https://docs.rs/hrtf>
- <https://github.com/mrDIMAS/tinyaudio> · <https://fyrox.rs/blog/post/twif18/>
- <https://docs.rs/kira/latest/kira/> · <https://crates.io/crates/bevy_kira_audio>
- Local sources read directly: `bevy_audio-0.17.2/src/{audio_source,audio_output,lib}.rs`,
  `rodio-0.20.1/src/source/spatial.rs`
