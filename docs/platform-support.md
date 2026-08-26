# Which features work where

**Written 2026-08-26.** Claims here are checked against the code, not assumed. Where
something is untested it says so rather than guessing.

## Three targets, not two

"Desktop vs Quest" is the wrong split, and using it has already produced one
inaccurate label in the editor. There are three:

| | What it is | GPU | Input |
| --- | --- | --- | --- |
| **Flat** | A windowed app on Windows/Linux/macOS. No headset. | Desktop | Keyboard + mouse |
| **PCVR** | OpenXR on a PC — a Quest over Link, or any PC headset. | Desktop | Controllers (+ keyboard, on the PC) |
| **Standalone** | The APK running on a Quest 3/Pro. | Adreno 740 | Controllers only |

The distinction that matters most is not PC-versus-headset but **flat versus XR**,
because the XR render path spawns its own cameras. PCVR and standalone share that
path exactly — `session.rs:846` has no `cfg` around it — so a feature attached to a
camera the XR path does not create is missing on *both*, however powerful the PC is.

The second distinction is **standalone versus everything else**, and that one is
purely about cost: an Adreno 740 has a 13.9 ms budget at 72 Hz, and a full-screen
pass can eat all of it.

## Support

✓ works · ✗ does not · ⚠ works but not affordable

| Feature | Flat | PCVR | Standalone | Why |
| --- | --- | --- | --- | --- |
| Colour grading | ✓ | ✓ | ✓ | Read during tonemapping; not a pass |
| Exposure, fog, IBL, skybox | ✓ | ✓ | ✓ | Environment-driven, applied to every `Camera3d` |
| Procedural atmosphere | ✓ | ✓ | ⚠ | Renders correctly; **+18.3 ms/frame** on a Quest 3 against a 13.9 ms budget |
| Bloom, tonemapping | ✓ | ✗ | ✗ | **Flat only, by decision — see below.** XR cameras carry no `Hdr`, and `XrdsBloom`/`XrdsTonemapping` reach only `XrdsAPI`-spawned cameras; a scene camera's value is dropped at `node.rs:178` |
| Anti-aliasing (MSAA sample count) | ✓ | ✓ | ✓ | `Msaa` is a required component of every camera; the scene authors the count. 8× is often unsupported on mobile GPUs |
| Video playback | ✓ | ✓ | ✓ | ffmpeg on desktop, MediaCodec + Vulkan on Android; the author does not choose |
| Camera / microphone capture | ✓ | ✓ | ✗ | `nokhwa` and `cpal` are target-gated off Android — they fail at *link* time there |
| Passthrough | ✗ | untested | ✓ | `XR_FB_passthrough`, device-verified on a Quest 3 |
| Grab / throw, controller input | ✗ | ✓ | ✓ | Needs XR controllers |
| Keyboard shortcuts in `xrds-app` | ✓ | ✓ | ✗ | No keyboard on a standalone headset |

## Bloom and tonemapping stay flat-only

**Decided 2026-08-26.** They do not work in XR on either kind of headset, and will
not be made to.

Making them work means giving XR cameras Bevy's `Hdr` — a float intermediate render
target paid **once per eye**, before any effect renders at all. That is a permanent
cost on the platform with the smallest budget, bought for a glow. The same money
spent on anti-aliasing buys something a wearer notices far more, since aliasing at
low pixels-per-degree is the most visible artefact in a headset.

They keep working on flat desktop, where the cost is irrelevant and the effect is
conventional. The types say so in their own rustdoc, because a limitation recorded
only in a document is a limitation nobody reads.

**What to reach for instead**, and it is not a workaround: scene-environment
exposure and colour grading are applied to every `Camera3d`, so they work on all
three targets. Tonal control ships; bloom does not.

## Two failure shapes, and they need different words

**Inert** — the feature is authorable and does nothing. Bloom on a headset is this:
the field saves, round-trips, and never reaches a camera that could use it. Nothing
reports it. This is the failure this project keeps paying for, and the platform
axis is one more way to arrive at it.

**Unaffordable** — the feature works perfectly and costs more than the frame. The
atmosphere is this: sky, horizon, haze and sun disc all confirmed by eye on device,
with no validation errors, at 32 fps. Worse than broken in one way, because it looks
like success right up until someone puts the headset on.

An author needs to be told which one they are choosing, and they are not the same
warning.

## What this implies for the editor

A control that differs by target should say so where the author is looking, not in a
document they have not opened. The editor had exactly one such label — "procedural
sky · desktop" on Atmosphere — and it was **wrong**: atmosphere works on PCVR, which
is not desktop. One ad-hoc label, one inaccuracy.

The badge should distinguish the two shapes above, because "this will cost you a
frame budget" and "this will do nothing" call for different decisions.

## Unverified

Recorded so nobody reads absence as support:

- **Passthrough on PCVR.** `XR_FB_passthrough` is a Meta extension; whether it is
  offered over Link has not been tested here.
- **Anything on Android XR** (the Google platform). Build config exists in
  `android/android-xr/` and is a placeholder — no device has ever run it.
- **macOS**, for any target. Planned, not built.
