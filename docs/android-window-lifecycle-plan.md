# Android window-lifecycle crash — investigation notes and plan

**Status:** not fixed, and **not reproduced under controlled conditions**. This file
exists so the next attempt starts from evidence rather than from the guess that was
in my head.

## The observed failure

```
uniform_buffer.rs:312   write_buffer_with(..).unwrap()
→ queue submission timeout
→ panic in a destructor
```

Seen repeatedly during the VFX device pass, around donning the headset after
launching it unworn. Reproduces with no particle crate present, so it is not
VFX-related.

**That trigger description is unconfirmed** — two staged attempts at it did not
reproduce the crash. See "Two controlled trials" below before treating it as the
repro.

## Correction: it is not simply "renders into a dead surface on resume"

I previously described this as the app rendering into a destroyed surface across a
doff→resume cycle. **A controlled test does not support that.**

Driving a full window destroy/recreate over adb — `input keyevent KEYCODE_HOME`
(`onPause`, native window goes null) then `am start` again (`onResume`, surface
recreated):

| Checkpoint | Result |
|---|---|
| PID before pause | 31421 |
| PID after pause | 31421 |
| PID after resume | 31421 |
| `panicked at` count | **0** |
| Surface after resume | `wgpu_core: configuring surface ... 1600x900` — clean reconfigure |

So the plain pause/resume path is healthy, and wgpu *does* correctly rebuild the
surface. Whatever breaks on donning the headset is more specific than "the surface
was recreated".

## `lifecycle=Suspended` is the normal steady state — not the bug

Worth recording, because it looks alarming and is not:

```
17:03:16.898  onActivityResumed
17:03:16.907  onActivityPaused        ← 9 ms later
17:03:17+     lifecycle=Suspended  should_update=true   (forever)
```

The VR compositor owns the display, so the 2D Activity resumes and immediately
pauses. A Quest app therefore runs permanently `Suspended`, and rendering while
suspended is exactly what the `patches/bevy_winit` changes exist to allow —
`ControlFlow::Poll` plus the Android-only `suspend_blocks_redraw = false` shadow
(`state.rs:732`), so `xrWaitFrame` keeps pacing frames. Do not "fix" this.

The unused-variable warning on `suspend_blocks_redraw` is a consequence of that
deliberate shadowing, not a half-applied patch.

## What still needs explaining

1. **What differs between HOME-resume (clean) and don-after-launch (crash)?**
   Donning also changes XR session state and restores immersive focus, and it happens
   while the app is in the ~1 frame/10s throttled state
   (`DOFF_FROM_GUARDIAN`, `bevy_time::virt ... skipping ~9.8s`). The long frame delta
   is a candidate the surface theory doesn't cover at all.
2. **Is the panic actually about the surface?** `uniform_buffer.rs:312` is a
   `write_buffer_with(..).unwrap()` on a *uniform buffer*, not a swapchain
   acquisition. A queue-submission timeout preceding it suggests the device/queue was
   already wedged. The surface may be a red herring.
3. **This error appears at every startup and nobody has explained it:**
   ```
   winit .../platform_impl: Cannot get the native window, it's null and will always
   be null before Event::Resumed and after Event::Suspended.
   ```
   Harmless-looking, but it proves something asks for the native window outside the
   valid window. Worth finding the caller before theorising further.

## Two controlled trials, both clean — the trigger is NOT understood

Ran on 2026-08-12 with a human donning the headset on cue.

**Trial 1 — don→doff while the device was already awake.** Survived. PID stable,
0 panics. The surface was destroyed and rebuilt at a *different resolution*, which
worked:

```
configuring surface ... 1600x900     ← window surface, on resume
configuring surface ... 4128x2208    ← VR surface, on don
```

**Trial 2 — the documented condition**: `mWakefulness=Asleep` confirmed before
launch, app started genuinely unworn, then donned on cue. **Survived.** PID 32168
stable, 0 panics, three surface reconfigures (1600x900, then 4128x2208 twice).
Lifecycle moved `Idle → Running → Suspended → Running` — note it does reach
`Running` here, unlike the adb HOME path where it stayed `Suspended` forever.

**Conclusion: "launch unworn, then don" is not a reliable trigger.** That was my
description of the repro and two attempts failed to confirm it. Combined with the adb
pause/resume result, three separate window destroy/recreate paths are now known to be
handled correctly.

The crash itself is real — observed several times during the VFX device pass — so what
remains is that it is **intermittent and timing-dependent**, with no known
deterministic trigger. Anyone picking this up should not assume the doff→don story.

## Revised recommendation: stop hunting, instrument and harden

Two clean trials say the cost of chasing a repro is high and the odds per attempt are
low. Better value, in order:

1. **Make the next natural occurrence self-documenting.** The crash has happened
   several times during ordinary iteration; it just was not being recorded. Keep a
   logcat streaming to a file during every device session (the recipe's step 4 already
   does this) and keep the file. One captured trace is worth ten staged attempts.
2. **Convert the abort into a dropped frame.** `uniform_buffer.rs:312` is an
   `unwrap`. Handling `SurfaceError::Lost`/`Outdated` and skipping the frame is
   defensible on its own merits regardless of the root cause, and turns a hard crash
   into a hiccup.
3. **Clamp the frame delta on resume.** The one hypothesis that is testable *without*
   a headset: inject a ~10 s stall and see whether the queue wedges. Cheap to try, and
   it either produces a repro or eliminates a candidate.

Only after (1) yields a trace is a targeted fix worth attempting.

## Repro procedure (needs a human in the headset — low yield, see above)

adb cannot simulate donning — the proximity sensor and compositor focus change are
physical. So:

```bash
adb logcat -c && adb logcat -v time > /tmp/doff.txt &
# 1. Headset OFF your head, sitting idle.
adb shell am start -n org.openxrds.devicesdk/android.app.NativeActivity
# 2. Wait ~15 s. Confirm the throttled state appears in the log.
# 3. PUT THE HEADSET ON.  ← the step that matters
```

Then capture:

```bash
grep -nE "panicked at|uniform_buffer|Timeout|SurfaceError|Lost|Outdated" /tmp/doff.txt
grep -nE "onActivityResumed|onActivityPaused|Suspended|Resumed"          /tmp/doff.txt
grep -nE "configuring surface|DOFF_FROM_GUARDIAN|skipping ~"             /tmp/doff.txt
grep -oE "lifecycle=[A-Za-z]+"                                          /tmp/doff.txt | uniq -c
```

The goal is **one captured crash log**, not a fix. Everything above is consistent with
at least three different root causes, and picking between them without a trace would be
guessing. Two staged attempts have already failed, so prefer passive capture during
normal work over further staged runs.

## Candidate fixes, once the cause is known

Listed so the options are on the table, explicitly **not** ranked as a plan yet:

- **Skip the render, keep the loop.** Decouple "pump the event loop for
  `xrWaitFrame`" from "submit a frame", so the app can idle safely while the surface
  or XR session is in transition. The most likely correct shape if the surface really
  is involved.
- **Clamp the frame delta.** If a ~10 s delta is what wedges the queue, a maximum
  delta on resume is a small, well-understood fix — and unlike the others it is
  testable without a headset by injecting a long stall.
- **Handle `SurfaceError::Lost`/`Outdated` explicitly** rather than letting an
  `unwrap` abort. Defensive regardless of cause; would convert a crash into a dropped
  frame.
- **Upstream it.** If it reproduces on plain `bevy` + `winit` on Android with no XR,
  it belongs in a Bevy issue, not in our patch tree.

## Not part of this, but noticed while investigating

`patches/bevy_winit/src/state.rs:604` has an unconditional `eprintln!` firing every
90th redraw — 48 lines in a 45-second run, in `RustStdoutStderr`. It is diagnostic
scaffolding in a vendored patch, and it competes for the logcat ring buffer with the
very output this investigation needs (Trap 2). Left alone deliberately: it is not the
crash, and quietly editing a load-bearing patch while chasing something else is how
the last three regressions happened. Worth gating behind a flag as its own change.
