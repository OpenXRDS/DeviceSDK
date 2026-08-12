# VFX / Particle-Effects System — Plan Index

**Status: Phases 1-4 complete; effects are authorable in the editor and
fireable from a Track. Phases 1-2 verified on hardware — `XrdsEffect` is
spawnable, tunable, round-trips through `scene.json`, and renders on standalone
Quest 3. Phase 3 (editor authoring) is next. `bevy_hanabi` rejected, `bevy_firework` adopted.** Particles are verified rendering on desktop, PCVR **and
standalone Quest 3** — but only via `bevy_firework` (CPU-simulated, batched
mesh). `bevy_hanabi` draws nothing on Qualcomm Adreno, a known upstream bug
(`djeedai/bevy_hanabi#548`) with no fix and no owner; see the Phase 0/0b
write-ups for the controlled A/B that established this. Everything below the
roadmap has been rewritten against `bevy_firework`'s API — if a section still
reads as hanabi-specific, treat it as stale.

This is the living index for the whole
feature area — updated as phases complete, mirroring how
`docs/xrds-trigger-action-backlog.md` tracks a feature area rather than one
shipped change. Each phase gets its own `docs/done/vfx-phaseN-*.md` write-up
as it actually ships; this file stays the roadmap, not a growing megafile.

## Why

`OVERALL_PROGRESS.md`'s Gap Analysis lists particle/VFX as the
highest-priority missing feature — "no groundwork started," unlike animation
or post-processing which have partial coverage.

## Grounding research

- **`bevy_firework` 0.8 is the adopted backend** — CPU-simulated, batched/
  instanced-mesh rendered, and **verified rendering on standalone Quest 3**
  (Phase 0b). 0.8 is the last release tracking Bevy 0.17 (0.9 → 0.18,
  0.10 → 0.19). Its only feature, `physics_avian` (default-on), depends on
  `avian3d ^0.4` — exactly this workspace's version, so collision support
  integrates rather than conflicts if we ever want it.
- **`bevy_hanabi` 0.17 was tried first and rejected on hardware.** It matches
  Bevy 0.17.2 and is the more mature, GPU-compute-driven option, but it renders
  **nothing** on Adreno — reproduced in immersive XR *and* in a single-camera
  flat window, with zero validation errors. Confirmed upstream as
  `djeedai/bevy_hanabi#548` (Adreno 830/840, hanabi 0.19, wgpu 29, 2D), so the
  bug spans hanabi 0.17→0.19, wgpu 26→29, Adreno 740→840, and both 2D and 3D.
  Still `need-repro`/unowned upstream. **Do not reintroduce it as the default
  backend without on-device proof that this is fixed.**
- **CPU simulation is a real constraint, not a footnote.** Budget in the
  *thousands* of live particles, not hanabi's hundreds of thousands. That is an
  acceptable ceiling for 72–90fps stereo on mobile silicon, but it must be
  documented in the SDK surface rather than discovered by a user.
- **Particle colours must be LDR (≤1.0) unless the camera is explicitly HDR.**
  `bevy_firework`'s own `sparks` example uses values like
  `LinearRgba::new(30., 20., 3., 1.)` because it pairs them with `Hdr` +
  `Bloom`. Our XR eye cameras (`xrds-openxr/session.rs`) carry neither, so
  anything >1.0 clamps and every particle renders pure white — observed
  on-device. Phase 1's `color_start`/`color_end` must not silently assume bloom.
- **The existing custom-shader machinery (`XrdsRuntimeMaterial`, an
  `ExtendedMaterial<StandardMaterial, _>` in `xrds_api/material.rs`) is
  irrelevant to this feature.** It's a PBR texture-slot patch with no vertex
  shader, no blend-mode control, no custom pipeline — VFX needs
  unlit/additive, camera-facing, per-instance quads, none of which that
  extension can express. `bevy_hanabi` brings its own render-graph
  integration; nothing here needs to plug into the existing one.
- **`XrdsBillboard` (`xrds_api/billboard.rs`) is not viable for particles.**
  It's a per-entity `Transform`-rewrite system in `PostUpdate` — fine for one
  quad (a text label), wrong at particle counts (N entities = N transform
  writes + N `GlobalTransform` propagations). Real particle facing belongs in
  the shader, which `bevy_firework` provides via its own
  `render::CustomMaterialPlugin`.
- **"Emitter" already means something else in this codebase** — a node that
  *fires a trigger* (`trigger_action.rs`'s watcher/Custom-trigger vocabulary:
  "emitters are enabled watchers' `fires` names"; a retired Tauri
  `spawn_snapshot_emitter`). The new type must not be named `XrdsEmitter` or
  it collides with an established, unrelated meaning. **Use `XrdsEffect`.**
- **No existing "SDK-baked preset library" pattern exists to reuse.** The
  closest precedents are (a) `XrdsSceneAssetKind`'s file-backed asset model
  — wrong fit, it validates against a real file on disk
  (`validate_binary_asset_source`), and an effect isn't a file; (b) the
  panel-template registry (`document.panels: Vec<XrdsPanelTemplate>`,
  `XrdsPanelTemplateId`) — the right *shape* for a future template library,
  but a promotion path for **later**, not something to build without a
  concrete second-instance need.
- **`SurfaceInterpreterRegistry` doesn't require a `XrdsGeometrySource`
  recipe.** `XrdsWorldPanel`/`XrdsAudioClip`/`XrdsText` are all
  `register_entity`-only (`registry.rs:69-80`). `XrdsEffect` should follow
  that shape, not the mesh-rebuild recipe path `XrdsCapsule` used.
- **`ParticleSystemPlugin` needs the exact test-harness gating `OutlinePlugin`
  already has** (`install.rs`) — **verified**, not assumed: it adds
  `render::CustomMaterialPlugin`, whose `build()` calls
  `app.sub_app_mut(RenderApp)` (`bevy_firework/src/render.rs:48`). The headless
  `xrds_test_app()` harness has no `RenderPlugin`, so without
  `#[cfg(not(test))]` every test calling `XrdsAPI::attach` would panic.
- **The Android/Quest build path is confirmed and concrete**:
  `android/quest/build.sh` runs
  `cargo ndk -t arm64-v8a -P 32 -o android/quest/jni build --release -p
  xrds-app --no-default-features` — it builds the `xrds-app` binary
  specifically. There is no generic `cargo run --example` path to Android —
  verification has to go through `apps/xrds-app`.

## Phased roadmap

```text
Phase 0   Quest/Android verification spike        ← DONE: hanabi failed
Phase 0b  Backend re-selection + re-verification  ← DONE: bevy_firework passed
Phase 1   Runtime-only XrdsEffect                 ← DONE (Quest verified)
Phase 2   Document authoring (xrds-scene-graph)  ← DONE
Phase 3   Editor authoring (apps/xrds-editor)     ← DONE
Phase 4   Trigger-action hookup (PlayEffect)      ← DONE
Phase 4b  StopEffect + per-row When Finished       ← DONE (device test owed)
Phase 5a  Visual fidelity fields                  ← DONE
Phase 5   Template library / preset expansion — speculative, gated on real need
```

Each phase only starts once the previous one is verified. Phase 0 earned its
keep: the backend that looked obviously correct on paper renders nothing on a
first-class target, and that cost a few builds to discover instead of three
layers of SDK wrapper.

- [x] **Phase 0** — Quest/Android verification spike. Desktop and PCVR passed;
      **standalone Quest failed** → `bevy_hanabi` rejected.
- [x] **Phase 0b** — Backend re-selection. `bevy_firework` 0.8 verified
      rendering on standalone Quest 3 via a controlled A/B. Gate cleared.
- [x] **Phase 1** — Runtime-only `XrdsEffect` on `bevy_firework`. Code complete,
      desktop visual check confirmed, and **Quest re-verification passed**: both
      a `Trail` and an `auto_play` `Burst`, authored into `scene.json` and
      reimported, render on standalone Quest 3.
- [x] **Phase 2** — Document authoring. `XrdsEffect` round-trips through
      `scene.json`; 203 runtime + 180 scene-graph tests passing.
- [x] **Phase 3** — Editor authoring. Effects placeable + tunable in the GUI;
      two UI bugs found by using it and fixed.
- [x] **Phase 4** — `XrdsAction::PlayEffect` via `queue_particles`, with two
      authoring diagnostics. **On-device firing not yet verified.**
- [x] **Phase 5a** — Visual fidelity: `blend`/`size_end`/`drag`/`fade_edge`/
      `fade_scene`, defaults chosen to change nothing.
- [ ] **Phase 5** — Template library / preset expansion (still not scheduled).

**Two work items this phase surfaced that are NOT part of this plan** — logged
here so they are not lost, but they belong in their own tracking:

1. **Android window-lifecycle crash (pre-existing, not VFX).** Launching with
   the headset off and then donning it destroys/recreates the native window;
   Bevy renders into the dead surface and aborts via
   `bevy_render/render_resource/uniform_buffer.rs:312`
   (`write_buffer_with(..).unwrap()`), then a queue-submission timeout and a
   panic-in-destructor. Reproduces with **no** particle crate present. Workaround
   is procedural (wear the headset before launch), which is not acceptable for
   shipped apps. Real fix: gate the render schedule while the window is gone.
2. **Bevy is two releases behind** (0.17 vs 0.19). This already forced pinning
   `bevy_firework` to 0.8 instead of latest, and will keep narrowing ecosystem
   choices.

## Explicitly out of scope (for this whole plan — not phased, not scheduled)

- **Direct exposure of `bevy_firework`'s full settings surface** through
  `XrdsAPI` — `scale_curve`/`FireworkCurve`, multi-stop `FireworkGradient`,
  `emissive_color`, the texture slots (`base_color_texture`,
  `normal_map_texture`, `orm_texture`), `pbr`, `event_handlers`, or more than one
  `ParticleSettings`/`EmissionSettings` entry per spawner. The SDK exposes
  exactly one of each with curated scalars; an author who needs the rest drops to
  the expert layer and uses `bevy_firework` directly via
  `api.add_startup_system`/`add_update_system`.
- **Particle collision** (`ParticleCollisionSettings`, behind the crate's
  `physics_avian` feature). It would integrate cleanly with our `avian3d 0.4`,
  which is precisely why it is tempting — but it is a separate capability with
  its own performance profile, not part of a first cut.
- **2D particle effects**, matching the SDK's 3D-only primitive/asset model.
- **A GPU-simulated backend.** `bevy_hanabi` is out until
  `djeedai/bevy_hanabi#548` is fixed *and* re-verified on device. Reopening this
  is a backend swap behind `XrdsEffect`, which is exactly why Phase 1 must not
  leak `bevy_firework` types through the public API.
- **Hand-rolling our own particle system.** Phase 0b retired this: a maintained
  crate works on our hardware, so building instanced billboard quads ourselves
  would be redundant effort.
- **Networked particle-effect synchronization.** Blocked on the same
  authority-model decision already flagged for
  `XrdsAction::SendNetworkMessage` in `docs/xrds-trigger-action-backlog.md`.
- **glTF export of effects.** Scene GLB export is already retired
  project-wide; an `XrdsEffect` node's `gltf_export_class()` is `NodeOnly`.
- **Automatic LOD or particle-count scaling based on device performance.**
  Real, but a cross-cutting performance feature, not VFX-specific.
- **Post-processing interactions** (particle-specific bloom tuning, etc.).
  Note the earlier assumption that "bloom applies for free" is **wrong on our XR
  cameras** — they have neither `Hdr` nor `Bloom`, which is why HDR colour values
  clamp to white (see Grounding research). Adding HDR/bloom to the XR cameras is
  a render-pipeline decision well outside this plan.
- **Editor-side effect authoring tools beyond parameter sliders** — no
  visual curve editor, no timeline scrubbing of a burst.

## Cross-phase verification checklist

- [x] A particle backend verified on real Quest hardware before Phase 1 code is
      written (`bevy_firework`, Phase 0b).
- [ ] `cargo check --workspace --all-targets` clean after every phase —
      including the headless test harness, which is what catches a missing
      `#[cfg(not(test))]` on a `RenderApp`-touching plugin.
- [ ] No phase regresses `cargo test -p xrds-runtime -p xrds-scene-graph
      -p xrds-editor` (currently 198 passing in xrds-runtime) or the frontend's
      `npx vitest run`/`npm run build`.
- [ ] **Re-verify on Quest at the end of each phase that touches rendering**,
      not just on desktop. Phase 0 is the standing proof that desktop success
      says nothing about Adreno.
- [ ] Particle colours stay ≤1.0 anywhere the SDK sets defaults or examples.
- [ ] No `bevy_firework` type appears in a public `XrdsAPI`/descriptor
      signature — the backend must stay swappable (see Out of scope).
- [ ] Each phase gets its own commit(s), split by concern.

## Spike cleanup — DONE

All Phase 0/0b scaffolding is removed (`bevy_hanabi` dep and feature, the two
smoke-test spawners, the `SmokeTestBurst` marker and head-lock system,
`flat-2d-smoke`, `AndroidManifest.flat.xml`, `build.ps1 -FlatPanel`, the
trace-logging branch, and the temporary `gen_effect_scene` generator). Verified:
workspace check clean, 203 runtime + 182 scene-graph + 35 editor tests, and both
`xrds-app` and the `particle_effect` example still build.

**Kept on purpose**, as flagged below: the `XrHapticRequest` registration (a real
latent-bug fix for any XR-off run) and the non-panicking wgpu
`on_uncaptured_error` handler. Also kept: `EXTRA_CARGO_FEATURES` in both build
scripts, which is a general-purpose knob rather than spike-specific.

**The flat-panel diagnostic is gone but its recipe is preserved** in the Phase 0b
write-up above — including the two traps that cost real time (the VR-category
manifest freezing the headset, and legacy `aapt` demanding the literal filename).
Recreating it is a few minutes of work if another "is this the platform or our XR
path?" question ever comes up.

## (historical) Spike cleanup ledger

All of this is throwaway scaffolding from Phase 0/0b, currently live in the
working tree. Keep the hanabi half reachable until the `#548` comment thread is
done with us, then remove:

- [ ] `hanabi-smoke-test` feature + `bevy_hanabi` dep (root + `xrds-app`) and
      `spawn_hanabi_smoke_test_effect`.
- [ ] `firework-smoke-test` feature and `spawn_firework_smoke_test_effect` —
      superseded once `XrdsEffect` exists; fold its verified parameters into
      `examples/rendering/particle_effect.rs`.
- [ ] `SmokeTestBurst` marker + `keep_smoke_test_effect_in_view`.
- [ ] `flat-2d-smoke` feature, `android/quest/AndroidManifest.flat.xml`, and
      `build.ps1 -FlatPanel`. **Keep the trap notes** in the Phase 0 write-up —
      they are the expensive part, not the code.
- [ ] The `hanabi-smoke-test` trace-logging branch in `android_main`.

Two Phase 0 changes are **not** cleanup and should stay:

- The `XrHapticRequest` registration in `install_xrds` — a real latent-bug fix
  affecting any XR-off run.
- The non-panicking wgpu `on_uncaptured_error` handler — defence in depth, with
  its comment already corrected to state honestly what it does not cover.

## Phase 1 design (revised for `bevy_firework`)

The original Phase 1 was written against hanabi's `EffectAsset` + `ExprWriter`
model. `bevy_firework` is a plain `Component`, which makes several steps simpler
— notably updates, which no longer need an asset rebuild.

### Two kinds, mapped to real API

| `XrdsEffectKind` | `bevy_firework` |
| --- | --- |
| `Burst` — one-shot radial spawn | `EmissionPacing::OneShot(count)` |
| `Trail` — continuous spawn | `EmissionPacing::rate(per_second)` |

### Curated parameter mapping (verified field names)

| `XrdsEffect` field | `bevy_firework` target |
| --- | --- |
| `auto_play` | selects pacing / `starts_enabled` — see below |
| `burst_count` (u32, Burst only) | `EmissionPacing::OneShot(count)` |
| `spawn_rate` (Trail only) | `EmissionPacing::rate(per_second)` |
| `lifetime_secs` | `ParticleSettings.lifetime: RandF32` |
| `size` (min/max) | `ParticleSettings.initial_scale: RandF32` |
| `color_start`, `color_end` | `ParticleSettings.base_color: FireworkGradient<LinearRgba>` — two stops, **clamped ≤1.0** |
| `speed` | `EmissionSettings.initial_velocity_radial: RandF32` |
| `omnidirectional` | selects radial vs. cone (they are mutually exclusive) |
| `spread_deg` (cone only) | `EmissionSettings.initial_velocity: RandVec3 { direction, spread }` |
| `gravity` | `ParticleSettings.acceleration: Vec3` |
| `emission_radius` | `EmissionSettings.emission_shape: EmissionShape::Sphere(r)` |

`RandF32`/`RandVec3` get `constant()` from the **`RandValue` trait**, which must
be in scope — and they come via `bevy_firework::bevy_utilitarian::...`, the
crate's own re-export, so no direct `bevy_utilitarian` dependency is needed.
Radial emission uses `initial_velocity_radial` with `initial_velocity` set to
`RandVec3::constant(Vec3::ZERO)`; a directional cone uses the reverse. Both are
confirmed working on device.

**Two fields were split apart after review, before Phase 2 could serialize
them** — cheap now, expensive once they are in `scene.json`:

- A single overloaded `rate` became `burst_count` (a count) and `spawn_rate` (a
  frequency). One number meaning two different things made switching `kind`
  silently reinterpret it; each kind now reads its own field and ignores the
  other. A test asserts a `Trail` uses `spawn_rate` while `burst_count` is set to
  a conflicting value.
- Radial emission became an explicit `omnidirectional: bool` instead of being
  inferred from `spread_deg >= 180`. The backend treats radial and cone as
  distinct paths, and a "180 degree cone" produced a lopsided hemisphere that
  still looked plausible at a glance.
- **`auto_play: bool` was added**, which Phase 4 would otherwise have been
  blocked on. Without it a `Burst` can only ever fire once: the backend's
  `OneShot` pacing disables its own emission the instant it fires
  (`bevy_firework core.rs:397`), so an authored burst would go off at scene load
  and never again — useless for trigger-driven VFX, which is the whole point of
  `XrdsAction::PlayEffect`.

`auto_play` maps onto **two different backend mechanisms**, because there is no
single "idle" switch covering both kinds:

| kind | `auto_play` | pacing | `starts_enabled` |
| --- | --- | --- | --- |
| Burst | true | `OneShot(burst_count)` | true |
| Burst | false | `OnDemand` | **true** |
| Trail | true | `rate(spawn_rate)` | true |
| Trail | false | `rate(spawn_rate)` | false |

The non-obvious row is `Burst`/`false`: an `OnDemand` spawner **must** keep
`starts_enabled: true`, because the emission loop returns early on `!enabled`
*before* it reads the queued count (`core.rs:387`) — a disabled on-demand
spawner silently ignores every fire request. There is a test asserting exactly
this, since it is invisible from the API surface.

Phase 4's `PlayEffect` therefore becomes a `ParticleSpawnerData::queue_particles(
burst_count)` call on a target whose `auto_play` is `false`. No despawn/respawn.

Default is `auto_play: true`, chosen so that spawning a default effect visibly
does *something* — an inert default is a poor first experience on the
non-expert path. Note this differs from what authoring probably wants: a burst
dragged into a scene almost always wants a trigger, so Phase 3's palette should
consider defaulting it to `false` (and may want a "test fire" affordance, since
an idle burst shows nothing in the editor viewport).

### Implementation steps

- [ ] Move the dependency: `bevy_firework` from `apps/xrds-app` to
      `crates/xrds-runtime/Cargo.toml` (mirror `avian3d`'s wiring). Prefer
      `default-features = false` — the only feature is `physics_avian`, and
      collision is out of scope; confirm it still compiles, since that feature
      is default-on.
- [ ] `crates/xrds-components/src/primitives/effect.rs`: `XrdsEffect`
      descriptor + `XrdsEffectKind`, shaped exactly like `capsule.rs` — plain
      data, `XrdsObject`/`XrdsComponent`/`XrdsMutableComponent`, no Bevy
      `Component` derive, no `bevy_firework` types in its public fields.
- [ ] `install.rs`: `#[cfg(not(test))] app.add_plugins(ParticleSystemPlugin::default())`
      beside `OutlinePlugin`. The gate is mandatory — see Grounding research.
- [ ] `spawn.rs`: `spawn_effect_descriptor` building one `ParticleSpawner` from
      the descriptor, plus `Transform` + `XrdsStored(descriptor)`.
- [ ] `registry.rs`: `register_entity::<XrdsEffect, _>` only (no
      `register_recipe_only` — no mesh to rebuild) + `register_clone`.
- [ ] `updaters.rs`: **mutate the `ParticleSpawner` component in place.** Unlike
      the hanabi plan, no despawn/respawn and no asset rebuild is needed.
- [ ] `xrds_api.rs`: `set_effect_params`, mirroring `set_capsule_geometry`.
- [ ] `examples/rendering/particle_effect.rs`: one `Burst` retriggered
      periodically plus one `Trail`, using the LDR colours proven on device.

### Verification

- [ ] `cargo check --workspace --all-targets` clean (proves the `cfg(not(test))`
      gate).
- [ ] `cargo test -p xrds-runtime` — no regression from 198; add a
      spawn-and-inspect test asserting `ParticleSpawner` exists with expected
      params after `set_effect_params`.
- [ ] Desktop visual check via the new example.
- [ ] **Quest re-verification** of that example's effect through `xrds-app`,
      headset worn before launch.

## Phase write-ups

### Phase 0 — desktop leg: done

- `bevy_hanabi = "0.17"` added to `[workspace.dependencies]`
  (`default-features = false, features = ["3d"]`), matching `avian3d`'s exact
  wiring. Referenced from `apps/xrds-app` only, behind a new, off-by-default
  `hanabi-smoke-test` feature (`dep:bevy_hanabi`) — not from `xrds-runtime`,
  per the plan's "isolate the spike" reasoning.
- Spike code lives in `apps/xrds-app/src/main.rs`, entirely
  `#[cfg(feature = "hanabi-smoke-test")]`-gated: `HanabiPlugin` added in
  `configure()`, and `spawn_hanabi_smoke_test_effect` (a `PostStartup` system,
  `.after(spawn_app_camera)`) builds a one-shot radial burst — position/velocity
  sphere modifiers, a yellow→red color-over-lifetime gradient — via
  `bevy_hanabi::EffectAsset`'s `ExprWriter`/`Module` builder, then parents the
  `ParticleEffect` entity to `AppCamera` at a fixed `-1.5` Z offset so it's
  always in view regardless of scene content. Raw `bevy_hanabi` API only, no
  XRDS wrapper — deliberate, per the plan.
- **One real correction during implementation**: I initially wrote
  `ColorOverLifetimeModifier { gradient, blend: ColorBlendMode::Modulate }`
  from memory/web-summary recall. It doesn't compile against the vendored
  0.17.0 source — the struct also requires a `mask: ColorBlendMask` field.
  docs.rs 404'd on the exact page, so I read the crate source directly from
  the local cargo registry cache instead of guessing further, and used the
  `ColorOverLifetimeModifier::new(gradient)` constructor, which fills sane
  defaults for both `blend` and `mask`.
- **Android build-script support added** (needed regardless of Phase 0's
  outcome, so worth keeping): `android/quest/build.sh` and `build.ps1` both
  gained an optional `EXTRA_CARGO_FEATURES` env var, appended as `--features
  ...` to the `cargo ndk` invocation. Lets a one-off verification feature (like
  `hanabi-smoke-test`) be built without editing the script, and is reusable for
  any future spike.
- **Verification performed:**
  - `cargo check -p xrds-app --features hanabi-smoke-test` — clean.
  - `cargo build -p xrds-app --features hanabi-smoke-test` — clean.
  - `cargo check --workspace --all-targets` (no feature) — clean, confirming
    the spike is fully inert by default.
  - `cargo test -p xrds-scene-graph` — 177 passed, unaffected (used briefly to
    generate a minimal valid `scene.json` for the desktop run via
    `XrdsSceneDocument::default().save_json(...)` in a throwaway test that was
    added and removed in the same session).
  - **Real desktop launch**, `target/debug/xrds-app.exe`, on an NVIDIA RTX
    5090 (Vulkan backend): no HMD attached, so OpenXR fell back to desktop
    rendering as designed (the `anyhow` backtrace in the log at that point is
    diagnostic capture on that *expected* fallback path, not a crash — traced
    through the log line by line to confirm). `bevy_hanabi::plugin: Initializing
    Hanabi for GPU adapter NVIDIA GeForce RTX 5090` logged successfully, the
    window opened, `[hanabi-smoke-test] burst effect spawned, parented to
    AppCamera at -1.5 Z` printed, and the process stayed stable for 10+ seconds
    with no further errors. Left running for the user to inspect visually
    rather than attempting a screenshot (no project skill covers launching
    this app, and a raw winit/Bevy window doesn't fit any of the standard
    GUI-automation patterns available in this sandbox with confidence — the
    user checked directly instead).

### Phase 0 — PCVR-via-Link leg: done, plus one real bug found and fixed

Confirmed working: with Quest Link actually active (an earlier attempt failed
because Link wasn't active — see below), the same desktop binary rendered the
looping burst through the headset via `xrds-openxr`'s PCVR path. This is the
first real evidence `bevy_hanabi` renders correctly *through a headset*, not
just on a discrete desktop GPU rendering to a monitor.

**A real, unrelated bug surfaced along the way and was fixed, not just
diagnosed.** The first attempt failed with `OpenXR unavailable... No XR
system found`. `crates/xrds-openxr/src/openxr/init.rs`'s `instance.system(...)`
call was made exactly once, with the resulting error discarded before the
generic "not found" bail — so there was no way to distinguish "runtime
missing" from "runtime present, form factor momentarily unavailable." Traced
to the exact OpenXR error (`ERROR_FORM_FACTOR_UNAVAILABLE`, which the spec
documents as transient, distinct from `ERROR_FORM_FACTOR_UNSUPPORTED`) and
added a bounded retry (`get_hmd_or_handheld_system`, 5s timeout, 200ms
interval, retries only on the transient error). In this instance the root
cause turned out to be Link simply not being active yet — confirmed by the
user — but the retry and improved error reporting are a genuine, permanent
fix to a real gap (a single unretried `xrGetSystem` racing Link's session
negotiation is a known, common failure mode for OpenXR PCVR apps), not
scoped back out once the immediate mystery was solved. Kept in the codebase.

Also fixed: the spawner was `SpawnerSettings::once(...)`, a single burst easy
to miss in a headset. Changed to `SpawnerSettings::burst(count, period)` —
repeats forever — so it's confirmable at a glance rather than requiring
being in exactly the right place at exactly the right moment.

Also fixed: the smoke test's own confirmation messages used `eprintln!`,
which this app never redirects to Android's logcat (only `log::` macros are,
via `android_logger`, tag `xrds`) — invisible on Android specifically, the
one platform this phase most needs it visible on. Switched to `log::`, which
required promoting the `log` crate from an Android-only dependency to a
cross-platform one in `apps/xrds-app/Cargo.toml` (a no-op addition on
desktop, since `bevy_log`'s plugin already bridges the `log` facade there).

### Phase 0 — Quest standalone leg: not yet run (needs physical hardware)

This is the actual decision gate the phase exists for — PCVR-via-Link
confirms the API usage and that the dependency renders correctly through a
headset, but says nothing about the *standalone* Quest path: a mobile
Vulkan/Adreno compute-shader pipeline with no discrete desktop GPU involved
at all. Needs to be run by whoever has the headset:

```powershell
# Windows, from repo root — prerequisites already confirmed present in this
# environment (cargo-ndk, aarch64-linux-android target, ANDROID_HOME, and the
# OpenXR loader already fetched into android/quest/libs/arm64-v8a/).
$env:EXTRA_CARGO_FEATURES = "hanabi-smoke-test"
.\android\quest\build.ps1

adb install -r android/quest/build/xrds-app.apk

# Dev mode: push the same minimal scene.json used for the desktop leg, plus
# the real assets/ dir — the app's shader-progress HUD label needs a real
# font, an empty assets/ will panic with "no default font found".
adb push target/debug/scene.json /sdcard/Android/data/org.openxrds.devicesdk/files/
adb push assets/ /sdcard/Android/data/org.openxrds.devicesdk/files/assets/

adb shell am start -n org.openxrds.devicesdk/android.app.NativeActivity

# Watch for the smoke test's own confirmation (now log::, not eprintln! —
# see above for why that distinction mattered) and any Vulkan/compute errors:
adb logcat -s xrds
```

### Phase 0 — Quest standalone leg: FAILS THE GATE — hanabi simulates, never renders

**Read this section, not an earlier draft of it.** An earlier version of this
write-up claimed the burst "renders and simulates for roughly 10 seconds" and
was "confirmed visually". Both claims were **wrong** and are retracted: the
only evidence behind them was the spike's own *spawn* log line, which proves
the entity was created, not that a single pixel was drawn. The burst has never
been seen on standalone Quest. Recording the mistake because it is the exact
trap this phase exists to avoid — treating "the code ran" as "the feature
works".

**Confirmed working on real standalone hardware** (no PC, no Link; `cargo ndk`
build, launched natively):

- `HanabiPlugin` initialises: `Initializing Hanabi for GPU adapter
  Adreno (TM) 740`. That message is the *success* branch of hanabi's only hard
  device gate (`Limits::max_bind_groups >= 4`, `plugin.rs:270`), so nothing is
  being silently gated out.
- Its compute/init/update shaders compile on the Adreno driver (verified via
  `naga`/`wgpu` compile tracing at debug level).
- The effect entity spawns and is simulated.

**Confirmed NOT working**: no particles are visible on-headset, against a
black empty scene where they would be unmistakable. Verified with placement
proven correct (see below), zero wgpu validation errors, zero pipeline or
shader errors, and the process alive and rendering everything else (HUD text
renders fine on the very same cameras).

**A real bug in the spike, found and fixed — but it was not the cause.** The
burst had been parented to `AppCamera` at a local `(0, 0, -1.5)`. `AppCamera`
is also `OpenXrPlayerRoot` (`spawn_app_camera`), i.e. the **player root**; the
XR eye cameras are its children and carry the head pose. So the offset pinned
the burst 1.5m forward of the *root*, at floor level, not rotating with gaze.
On-device telemetry showed the head at `(0.007, 1.247, 8.064)` while the root
sat at the origin — the burst was ~9.5m away and ~1.2m below eye level, far
outside the FOV. PCVR's tracking origin happened to line the two up, which is
why it looked fine there. Fixed by `keep_smoke_test_effect_in_view`, which
head-locks via `head.rotation() * Vec3::NEG_Z * 1.5` off eye 0's
`GlobalTransform` in `PostUpdate` after `TransformSystems::Propagate` (never
from an authored world position — the standing pitfall here). Verified
numerically on-device: delta magnitude exactly 1.50m, tracking head movement.
**Particles still invisible after this fix**, which is what makes the
remaining failure a genuine render failure rather than a placement mistake.

**Leading hypothesis for the no-render (strong, but NOT proven).** hanabi
draws particles *exclusively* through GPU indirect draw — it allocates
`BufferUsages::INDIRECT` tables of `GpuDrawIndexedIndirectArgs`
(`render/mod.rs:2865-2921`) and has no CPU-batched fallback. Our XR eye
cameras carry `NoIndirectDrawing` (`xrds-openxr/session.rs:775`) for a
documented reason: *"Bevy 0.17's GPU indirect preprocessing
(GpuPreprocessingMode::Culling) mishandles multi-camera XR on Android: work
items for the two eye cameras share global offsets and interfere, causing one
eye to lose all geometry."* That flag governs Bevy's own mesh batching;
hanabi never consults it and has no equivalent opt-out, so it keeps issuing
exactly the class of indirect draw this platform is documented to mishandle.

What makes this the leading theory rather than a guess: PCVR runs the **same
two-eye camera code** through `xrds-openxr` and renders particles correctly,
so multi-camera *logic* is not the differentiator — the platform is (desktop
GPU vs Adreno/Android). What keeps it a hypothesis: confirming it needs a GPU
capture (RenderDoc / Adreno profiler), which is out of reach from here. No
upstream fix or tracking issue was found; the closest analogue is Godot's
`GPUParticles3D` failing on Quest (godotengine/godot#83275), i.e. GPU
particles on this hardware class is a known-hazard area generally.

**Important caveat on that theory, so it isn't over-read:** hanabi does *not*
ride Bevy's mesh-preprocessing path. It never references `NoIndirectDrawing`
or `GpuPreprocessing` anywhere (grepped: zero hits); it maintains its own
`INDIRECT` buffers and queues into the standard `Transparent3d`/`Opaque3d`
phases per view. So the specific Bevy bug our `session.rs` comment describes is
*different code* from hanabi's indirect path. The two share only "issues
indirect draws on Adreno", which is suggestive, not equivalent. Corollary:
removing `NoIndirectDrawing` from the XR cameras would not help hanabi (it
would only break mesh rendering in one eye) — don't spend a build on it.

**Ruled out by direct source inspection** (recording these so they aren't
re-litigated):

- *Missing `Msaa` on the XR cameras.* hanabi's queue query is
  `Query<(&RenderVisibleEntities, &ExtractedView, &Msaa)>`
  (`render/mod.rs:5178,5551`), so a view lacking `Msaa` would be skipped
  silently — an attractive theory, since `session.rs` spawns the eye cameras
  with no explicit `Msaa` and neither `Camera` nor `Camera3d` lists it in
  `#[require(..)]`. It is nevertheless **not** the cause: `bevy_render`'s
  `CameraPlugin` calls `app.register_required_components::<Camera, Msaa>()`
  (`bevy_render/src/camera.rs:56`), so every camera gets `Msaa` at runtime.
- *A GPU feature/limit gate.* hanabi's only hard gate is
  `max_bind_groups >= 4`, and the device logged the success branch.
- *Swallowed GPU errors.* Zero wgpu validation errors and zero
  `non-fatal wgpu error` handler firings across a full worn-headset run.

**RESOLVED by the flat-panel control test: it is the platform, not our XR
path.** The test above was run (see "Flat-panel diagnostic" below for how). On
standalone Quest, rendering through a **single** camera to a **Window** render
target with **no OpenXR session at all**, with frames confirmed advancing
(~1900+ frames across 9 diagnostic samples) and the burst placed dead-centre
1.5m ahead of the camera: the scene's cube and HUD text render correctly and
**the particles are still absent**. Zero panics, zero wgpu errors.

That control removes every XR-specific variable at once — two cameras,
`RenderTarget::TextureView`, `Projection::custom`, `NoIndirectDrawing`,
`NoCpuCulling`, and OpenXR itself. The full matrix:

| Configuration | Cameras | Target | Particles |
| --- | --- | --- | --- |
| Windows desktop, discrete GPU | 1 | Window | **yes** |
| PCVR via Link (desktop binary) | 2 XR | TextureView | **yes** |
| Quest standalone, immersive XR | 2 XR | TextureView | **no** |
| Quest standalone, flat 2D panel | 1 | Window | **no** |

The only variable that tracks the outcome is Android/Adreno. **Conclusion:
`bevy_hanabi` 0.17's GPU particle rendering does not work on this
driver/hardware, independent of XR.** The indirect-draw dependency remains the
most plausible *mechanism* (it is the one thing hanabi does that ordinary mesh
rendering does not, and mesh rendering works fine on the same device), but the
mechanism is now secondary — the verdict does not depend on it.

**Do not read this as "Quest cannot do particles."** Phase 0b below renders
particles on the same headset, through the same two XR eye cameras, using a
CPU-simulated crate. The finding is specific to `bevy_hanabi`, not to the
device, the Adreno driver's particle capability in general, or our XR pipeline.

**Flat-panel diagnostic — how to re-run it.** Two pieces, both to be deleted
with the spike:

- `flat-2d-smoke` Cargo feature (`apps/xrds-app`) forces
  `RuntimeParameters::enable_xr = false`.
- `android/quest/AndroidManifest.flat.xml` + `build.ps1 -FlatPanel`.

```powershell
$env:EXTRA_CARGO_FEATURES = "hanabi-smoke-test,flat-2d-smoke"
./android/quest/build.ps1 -FlatPanel
```

Two traps this cost real time on, both worth not rediscovering:

1. **The manifest is mandatory, not optional.** Disabling XR while keeping the
   shipping manifest *freezes the headset*: `com.oculus.intent.category.VR`
   grants immersive VR focus, so the app owns the display but never submits an
   OpenXR frame and the compositor has nothing to present. Recover with
   `adb shell am force-stop org.openxrds.devicesdk`.
2. **Legacy `aapt` ignores `-M`'s filename** — it demands a file literally
   named `AndroidManifest.xml` ("ERROR: No AndroidManifest.xml file found."),
   so `build.ps1` copies the flat variant into `$BuildDir` under that name.

Also note a flat panel window only advances frames while **focused** — an
unfocused panel sits at `lifecycle=Idle`/`Suspended` and renders nothing, which
looks exactly like a render bug. Confirm frames are advancing (count the
`burst placed at` diagnostic lines) before drawing any conclusion.

**Mechanism narrowed by trace logging: the chain breaks BEFORE pipeline
specialization.** With `trace` enabled (see below), a worn-headset immersive run
producing ~11,600 frames emitted **zero** hanabi render-chain messages — no
`Specializing render pipeline`, no batch/queue output — while trace from
`android_activity`, `jni` and `cosmic_text` flowed freely (107k lines). Since
`queue_effects` only reaches its `trace!` after batches exist, the effect is
most likely never getting **extracted/batched** into the render world, rather
than being drawn and discarded. That reframes the indirect-draw theory: an
indirect draw that is never issued cannot be the fault. Not yet proven — the
clean way to finish this is a *differential* trace (same build on desktop, where
it works, versus Quest) to see exactly which stage is present on one and absent
on the other.

**How to get hanabi trace on Android (two non-obvious traps).** `trace!` is
invisible by default and its absence means nothing unless trace is verifiably
flowing — this produced one wrong conclusion in this doc already:

1. `android_logger` is configured at `LevelFilter::Debug`; `with_max_level`
   gates *before* the filter, so it must be raised to `Trace` and the filter
   used to hold everything else down.
2. **`bevy_hanabi=trace` as a filter directive does not work.** With
   `LogPlugin` disabled these are `tracing` events crossing to `log` via
   tracing's fallback bridge, and hanabi's records arrive with target
   `event <src-file-path>` (visible in logcat as tag `event C:\...\plugin.rs`),
   not `bevy_hanabi`. Use a global `trace` with the noisy crates suppressed
   (`naga`, `wgpu*`, `offset_allocator`, `bevy_render`, ...) — see the
   `hanabi-smoke-test` branch of `android_main`. Verify it worked by counting
   `V/xrds` lines before drawing conclusions.

**Do not attribute the ~9.8s freezes to hanabi.** They are the OS throttling an
**unworn** headset: in the run above, 7 of 9 `bevy_time::virt` stalls carry the
~9.77s signature and the system log shows 5 `DOFF_FROM_GUARDIAN` events. A
subjective report of post-shader-compile stuttering was observed on-headset but
is **not** established as hanabi-induced by any log evidence; measuring that
needs an fps comparison between the `hanabi-smoke-test` and control builds with
the headset continuously worn.

**Unrelated real bug this test surfaced and fixed.** With XR off, the app
panicked on its first frame: `haptic_test_key_system` holds
`MessageWriter<XrHapticRequest>`, and that message is registered only by
`XrInputPlugin`, which is absent whenever XR is off — Bevy 0.17 escalates the
failed parameter validation to a panic. Fixed by registering it in
`install_xrds` beside the existing `XrInput` init (idempotent, so it is a no-op
when the real plugin loads). This would have hit **any** desktop run without an
HMD, not just this diagnostic.

**The crash was a separate, pre-existing bug — and my first diagnosis of it
was wrong.** Corrected root cause: the app renders across an Android **window
destroy/recreate**, which happens when you launch with the headset off and
then put it on (`glue: Resume` → `WindowFocusChanged` → `NativeWindowCreated`).
Rendering into the dead surface panics, in this order:

```text
bevy_render/render_resource/uniform_buffer.rs:312  called `Option::unwrap()` on a `None` value   (x9)
wgpu-26.0.1/src/backend/wgpu_core.rs:1994          Buffer::get_mapped_range -> handle_error_fatal
wgpu-core-26.0.1/src/device/queue.rs:206           We timed out while waiting on the last successful submission to complete!
core/src/panicking.rs:233                          panic in a destructor during cleanup -> non-unwinding panic, aborting
```

Line 312 is `queue.write_buffer_with(..).unwrap()` — a plain unwrap in
upstream `bevy_render` that returns `None` when a buffer allocation fails.
**Launching with the headset already worn avoids it entirely** (a full run,
zero panics). The earlier passthrough/gralloc `Texture::create_view` theory in
this doc is superseded: that error was one downstream symptom of the same dead
surface, and plugging it only moved the crash to the `unwrap` above. This is
not VFX-specific — `uniform_buffer.rs` backs every view — and belongs in its
own work item, not this plan.

**On the wgpu error handler added to `xrds_api/install.rs`:** it replaces
wgpu's default panicking uncaptured-error handler with a logging one. It is
defensible on its own terms but it **did not fix the crash and never once
fired** on device — the panics above go through `handle_error_fatal`, plain
`.unwrap()`s, and a submission timeout, none of which route through the error
sink. Kept as defence in depth with its comment corrected to say exactly that.
Also worth knowing: it means a future wgpu validation error will be a log line
rather than a crash, so grep for `non-fatal wgpu error` when something renders
wrong. (Checked here: zero occurrences, so nothing was being masked.)

**Methodology notes that actually mattered.** Streaming `adb logcat` to a file
*before* launch is required — dumping with `-d` afterwards loses startup
messages, because per-frame `XR-DIAG`/`naga` debug tracing rotates the ring
buffer within seconds. This produced a false "hanabi isn't compiled in" alarm
early on, disproved with `strings libxrds_app.so | grep -c hanabi`. Separately:
`eprintln!`/`println!` **is** captured on Android, under the
`RustStdoutStderr` tag, not `xrds` — an earlier claim in this doc that it is
not redirected was wrong in the general case. Filter both tags. Finally, the
recurring `bevy_time::virt: ...skipping ~9.8s` lines are the OS throttling an
**unworn** headset (`DOFF_FROM_GUARDIAN` in the system log), not an app stall —
diagnose with the headset on.

**Decision gate: FAILED as specified.** The plan's gate reads "if this fails
outright ... stop and re-plan", and its Phase-1 precondition is "Phase 0
passed on real Quest hardware before Phase 1 code is written". Compute runs,
rendering does not, on a first-class target platform. Phase 1 is therefore
**not** unblocked. Options, for the user to choose:

1. **Confirm the indirect-draw theory** — get a RenderDoc/Adreno capture, or
   test hanabi on Android in a single-camera non-XR window. Highest
   information gain; needs tooling not available in this environment.
2. **Raise it upstream** with the minimal repro we now have (hanabi + two XR
   cameras on Adreno) — the right move if the theory holds, but puts the
   feature on someone else's schedule.
3. **Re-plan around a non-hanabi path** for mobile XR — the plan already lists
   "a CPU-simulated fallback" as explicitly out of scope and *"a different
   plan (possibly a much larger one)"*. That framing still holds.
4. **Proceed with Phase 1 desktop/PCVR-only, with the Quest gap recorded** —
   contradicts the gate as written, so only with eyes open: it means building
   SDK layers on a dependency that does not render on Quest.

Cross-phase checklist, accurately: crash-free launch **achieved** (headset worn
before launch); particle burst visually confirmed **NO** on standalone (yes on
desktop and PCVR); `adb logcat` checked for Vulkan/compute errors — **none
found**, which is itself the notable result; thermal/frame-time sanity **not
assessed**, moot until anything renders.

---

### Phase 0b — `bevy_firework` on Quest: PASSES, gate cleared

**Particles render on standalone Quest 3.** Confirmed on-headset in immersive
XR: a continuous stream of particles, head-locked 1.5m ahead, visible through
both eyes. Log for the same run: spawner created, **75** head-lock diagnostic
samples (~16,200 frames), `xr_eyes=2`, **zero** panics, **zero** wgpu errors,
one throttle stall (a brief doff).

**This was a controlled A/B, which is what makes it conclusive.** Identical
headset, identical two-eye XR camera setup, identical placement code (both
spikes share the `SmokeTestBurst` marker and the one
`keep_smoke_test_effect_in_view` system, specifically so a placement difference
could not masquerade as a rendering difference), identical build/deploy path and
launch protocol. The single variable was the drawing crate:

| Crate | Simulation | Draw path | Quest standalone |
| --- | --- | --- | --- |
| `bevy_hanabi` 0.17 | GPU compute | GPU indirect draw | **nothing renders** |
| `bevy_firework` 0.8 | CPU | batched/instanced mesh | **renders** |

So the fault is **`bevy_hanabi`'s compute + indirect-draw path specifically** —
not the Adreno 740, not Android, and not our XR render pipeline. That also
retires the "different, larger plan" branch: no hand-rolled particle system is
needed, because a maintained crate already works here.

**Two cosmetic artefacts, both from the spike's own parameters — not bugs:**

- *Particles appeared white, not orange.* The gradient used HDR values
  (`LinearRgba::new(30., 20., 3., 1.)`, copied from the crate's `sparks`
  example, which pairs them with `Hdr` + `Bloom`). Our XR eye cameras carry
  neither, so anything >1.0 clamps to white. Keep particle colours ≤1.0 unless
  the camera is explicitly HDR — worth remembering for Phase 1's `color_start`/
  `color_end` parameters, which must not silently assume bloom.
- *They streamed upward like a fountain.* That is exactly what was asked for:
  `initial_velocity.direction: Vec3::Y` with `acceleration` left at its default
  of zero, i.e. up and no gravity. An omnidirectional burst wants
  `EmissionShape::Sphere` with a radial velocity, and a falling spark wants a
  negative-Y `acceleration`.

Both confirm the parameter surface behaves predictably, which is the real
prerequisite for Phase 1's curated parameter set.

**Consequence for the plan:** Phase 1 should be built on **`bevy_firework` 0.8**
(the last release tracking Bevy 0.17 — 0.9 is 0.18, 0.10 is 0.19). Its optional
`physics_avian` feature depends on `avian3d ^0.4`, which is exactly the version
this workspace already uses, so its collision integration lines up with our
physics engine rather than fighting it. Accept the trade knowingly: CPU
simulation is comfortable in the thousands of particles, not hanabi's hundreds
of thousands — an acceptable ceiling for 72–90fps stereo on mobile silicon, and
it should be stated in the SDK docs rather than discovered by a user.

**Also worth noting:** we are two Bevy releases behind (0.17 vs 0.19), which is
why `bevy_firework` had to be pinned to 0.8 rather than latest. That lag will
keep constraining ecosystem choices.

**Confirmed as a known upstream bug: `djeedai/bevy_hanabi` issue #548.** Another
team reports the same symptom — effects report `is_ready() == true` with active
spawners and correct visibility flags, yet draw zero pixels — on Adreno 830/840
with hanabi 0.19.0 / wgpu 29.0.4, on Android, over Vulkan. They cite Qualcomm
**vendor ID 20803**, exactly what our adapter reports. They suspect "a
driver-specific issue with indirect draw calls or compute-shader-written
buffers" and shipped an adapter-gated fallback to CPU particles — independently
the same workaround Phase 0b validated here.

Read across both reports, the bug spans **hanabi 0.17 → 0.19**, **wgpu 26 → 29**,
**Adreno 740 → 840**, and **both 2D and 3D**. So it is a Qualcomm-driver-family
problem, it is not fixed by upgrading, and it is not phase-specific. The issue
is still labelled `need-repro` and unconfirmed by the maintainer, with no fix or
target version.

Practical consequences for this plan:

- Do **not** wait for an upstream fix. It is unconfirmed, has no owner, and
  affects every version pair either team has tried.
- Our data meaningfully widens that report (third Adreno generation, older
  version pair, 3D, plus a controlled A/B against a CPU crate). A write-up ready
  to post as a comment on #548 is kept out-of-tree in the session scratchpad as
  `hanabi-548-report.md` — it is a one-off external artefact, not project
  documentation, so it is deliberately not committed here.
- Keep the hanabi spike code reachable behind its feature flag until that
  comment is posted, in case the maintainer asks for more runs.

---

### Phase 1 — runtime-only `XrdsEffect`: code complete, desktop verified

**Landed.** `cargo check --workspace --all-targets` clean; `cargo test -p
xrds-runtime` at **200 passing** (was 198). Desktop visual check confirmed by
running `cargo run --example particle_effect` — window opened on an RTX 5090
(Vulkan), particles visible, clean exit, no panics.

Files:

- `crates/xrds-components/src/primitives/effect.rs` — `XrdsEffect` +
  `XrdsEffectKind`, shaped like `capsule.rs`. **No `bevy_firework` type appears
  in any public field**, which is the guardrail that lets the backend be swapped
  again if it ever needs to be.
- `crates/xrds-components/src/values.rs` — `EffectParams` (+ `lib.rs` export).
- `crates/xrds-runtime/Cargo.toml` — dependency moved off `xrds-app`,
  `default-features = false` to drop `physics_avian`.
- `install.rs` — `ParticleSystemPlugin` under `#[cfg(not(test))]`.
- `spawn.rs` — `build_particle_spawner` + `spawn_effect_descriptor`.
- `registry.rs` — `register_entity` + `register_clone`, no recipe.
- `updaters.rs` — `register_stored_effect_updaters`.
- `api.rs` — `XrdsAPI::set_effect_params`.
- `tests/effect.rs` — two tests.
- `examples/rendering/particle_effect.rs` (+ `Cargo.toml` `[[example]]`).

**Design decisions worth not re-deriving:**

- *Colours clamp to LDR inside `build_particle_spawner`.* An author who writes
  `30.0` gets the brightest representable colour instead of a white blob. A test
  feeds `30.0` and asserts `1.0`.
- *`omnidirectional` is an explicit flag*, not inferred from the spread angle,
  and `spread_deg` is ignored when it is set. The backend treats directional and
  radial as mutually exclusive (one sets `initial_velocity`, the other
  `initial_velocity_radial`), and a cone at 180° yields a lopsided hemisphere
  that still looks plausible in a screenshot — hence its own test.
- *`burst_count` and `spawn_rate` are separate fields*, each used by exactly one
  kind. See the Phase 1 design notes for why the overloaded single field was
  rejected.
- *Updates rebuild the `ParticleSpawner` component in place.* No recipe
  round-trip, no asset re-add — this is the concrete payoff of `bevy_firework`
  being a plain component where hanabi needed an asset. Particles already alive
  finish under the old settings, which reads as a transition rather than a pop.
- *No `XrdsColor`/`XrdsMaterialParams` updaters are registered for effects.*
  They would silently no-op on a non-mesh; better to not offer them.
- *No `NoFrustumCulling` on the effect entity*, unlike the mesh primitives. It is
  not needed: the entity gets no `Aabb`, so Bevy performs no frustum culling on
  it — and empirically it rendered through both XR eye cameras in Phase 0b. Worth
  re-checking if an `Aabb` ever starts being backfilled onto effects.

**Owed before Phase 2 is called done** (the cross-phase checklist requires
re-verifying rendering on Quest, since Phase 0 is the standing proof that desktop
success says nothing about Adreno):

- [x] **Done.** A generated `scene.json` containing a `Trail` and an
      `auto_play` `Burst` was bundled into the APK (`build.ps1 -SceneDir`) and
      both rendered on standalone Quest 3 — 4 entities loaded, zero panics, zero
      wgpu errors. This exercised the *document* path end to end
      (`XrdsSceneEffect` -> `XrdsEffect` -> `ParticleSpawner` -> pixels), so it
      covers Phase 2 as well.

      **A placement trap found while doing it, worth carrying into Phase 3:**
      the effects were first authored at `z = -1.5` ("1.5m in front"), but
      `xrds-app`'s default spawn is `(0, 1.6, 8.0)` looking down -Z. That put
      them 9.5m away, where a 1.2m separation subtends ~7 degrees and both read
      as a single distant blob. Nothing was wrong with the rendering. The
      editor palette must place effects relative to the view/spawn rather than
      the origin, or authors will hit this immediately — and unlike a mesh, a
      misplaced particle effect gives no silhouette to hint where it went.

**Deferred, deliberately:** `set_effect_params_for_node` on `XrdsUpdateContext`.
Nothing needs per-frame parameter mutation yet; it belongs with Phase 3's editor
live-preview, which is where the same pattern already exists for other
primitives.

---

### Phase 2 — document authoring: done

`XrdsEffect` now saves to and loads from `scene.json`. Suites: **203**
(xrds-runtime), **180** (xrds-scene-graph), 35 (xrds-editor), workspace check
clean.

Files:

- `scene/payload.rs` — `XrdsSceneNodePayload::Effect(XrdsSceneEffect)`,
  `XrdsSceneEffect`, `XrdsSceneEffectKind`, per-field serde defaults, and the
  `gltf_export_class()` arm (`NodeOnly` — glTF has no emitter vocabulary, and
  scene GLB export is retired project-wide anyway).
- `scene/node.rs` — `XrdsSceneRuntimeComponent::Effect`, the
  `to_runtime_node_*` arm, `from_xrds_effect()`.
- `scene/../lib.rs` — `XrdsEffect`/`XrdsEffectKind` imports.
- `xrds-runtime`: `helper.rs` export branch, `api.rs` + `reimport.rs` arms.
- `apps/xrds-editor/src-tauri`: `hierarchy.rs` + `inspector.rs` kind strings
  only — the exhaustive matches force them; real editor authoring is Phase 3.
- Tests: `xrds-runtime/tests/effect.rs` (file round-trip + reimport),
  `xrds-scene-graph/tests/effect.rs` (three serde tests).

**A separate wire enum, deliberately.** `XrdsSceneEffectKind` duplicates
`XrdsEffectKind` instead of serializing the runtime enum. The serialized names
are a file-format contract; the runtime enum should stay free to be renamed or
extended without silently breaking every saved scene.

**Every field has a serde default, and there is a test pinning what those
defaults are.** The format is already in users' hands, so a scene written before
a field existed must still load. The defaults are not arbitrary — they are the
values verified on Quest hardware, so a drift to zeros (an easy accident when
adding a field) would yield invisible effects on load instead of an error. One
test asserts a bare `{}` deserializes to exactly those values; another asserts a
partial payload merges rather than being rejected.

**The one branch the compiler cannot check** is `helper.rs`'s
`XrdsStored<XrdsEffect>` export arm — omitting it exports the node as `Empty`
and silently drops the effect from the saved scene. The round-trip test is the
only real guard, and its failure message says so explicitly, so a future
regression points straight at the cause.

**Verified rather than assumed:** `document/material.rs`'s
`node_material_ref`/`_mut` need no arm — they end in `_ => None`, so an effect
correctly reports no material. Its colour is its own gradient, not an
`XrdsSceneMaterial`.

**A test bug worth recording**, since it is the mapping table earning its keep:
the round-trip test initially asserted that a `Trail` with `auto_play: false`
produces `OnDemand` pacing. It does not — `OnDemand` is the `Burst` path; a
`Trail` idles via `starts_enabled` and keeps rate-based pacing. The code was
right and the test was wrong.

**Not in Phase 2:** the editor only learned the kind *string*. There is no
palette entry, no `NodePayloadDto`, no Inspector section — so an effect authored
in Rust survives a save/load through the editor, but cannot yet be placed or
tuned from the GUI. That is Phase 3, and per the Phase 1 notes the palette should
default `auto_play` to `false` and consider a "test fire" affordance, since an
idle burst renders nothing in the viewport.

---

### Phase 3 — editor authoring: done

Effects are placeable and tunable from the GUI. Workspace check clean; 203
runtime + 183 scene-graph + 35 editor Rust tests; `tsc` clean; 122 vitest;
production build ok. Manually confirmed in the editor: placement and live
Inspector tuning both work.

- `bridge.rs` — `NodePayloadDto::Effect` (no `material` field) and
  `SetEffectParams`, which sends every field on each edit. Same
  live-preview-on-every-event shape as `SetCapsuleGeometry`, which keeps the
  backend free of partial-merge logic.
- `palette.rs` — **two** entries, `EffectBurst` and `EffectTrail`, in a new
  "Effects" group. Two earlier decisions land here: a palette-placed `Burst` gets
  `auto_play: false` (deliberately unlike the Rust default — a hand-placed burst
  wants a trigger), and effects default to **y = 1.4**, not the meshes' 0.5.
- `inspector.rs` / `editor_state.rs` / `bevy_scene.rs` — DTO population, the
  command handler, pending state, and the live-preview apply.
- `context.rs` — `set_effect_params_for_node`, the setter deferred out of Phase 1
  and added here once the Inspector actually needed it.
- Frontend — `bridge.ts` types + icons, `Palette.tsx` metadata/group,
  `Inspector.tsx`'s `EffectParamsSection`.

**Deliberate UI choices:**

- Only the count field the current kind *reads* is shown (`burst_count` for
  Burst, `spawn_rate` for Trail). Showing both invites tuning a value the runtime
  ignores.
- The auto-play consequence is spelled out inline, including that an idle burst
  draws nothing in the viewport — otherwise that looks like a broken effect.
- Unknown `kind` strings are rejected rather than defaulted: silently turning a
  Trail into a Burst on a frontend typo would be a miserable edit to debug.

**Two bugs found by using it, both fixed:**

- **The grabbable checkbox was offered on effects.** The Inspector already had
  `KINDS_WITHOUT_GEOMETRY` for exactly this — grab raycasts against `Aabb`, which
  Bevy derives from `Mesh3d`, and `spawn_effect_descriptor` inserts neither. The
  checkbox armed `XrGrabbable` on an entity that could never be picked up.
  `"Effect"` is now on that list.
- **Burst and Trail shared one hierarchy icon.** A consequence of kind being a
  *field* on one payload rather than two variants (unlike lights). `payload_kind`
  now returns `"EffectBurst"`/`"EffectTrail"`; verified first that this field is
  display-only (icon + kind badge, no logic branches on it). The Inspector's
  `payload_kind_name` still reports `"Effect"`, which correctly names the payload.

**Left alone on purpose:** the trigger-binding slot appears on effects. That
section is not kind-gated for *any* node, by an explicit existing decision
("Every kind is shown, always — including ones this node can't fire yet — with a
trailing hint saying what's missing, rather than silently shortening the list").
Special-casing effects would reintroduce the ambiguity that comment was written
to remove — and Phase 4 makes effects a prime trigger *target* anyway.

---

### Phase 4 — `XrdsAction::PlayEffect`: done

A Track can now fire an effect. This is what `auto_play: false` was added for in
Phase 1.

Mechanism: `ParticleSpawnerData::queue_particles(count)`, drained next frame
under `OnDemand` pacing. It is **additive**, so two triggers firing one effect on
the same frame emit both bursts rather than one overwriting the other. No
despawn/respawn, no asset rebuild.

- `scene/trigger_action.rs` — the `XrdsAction::PlayEffect { count: Option<u32> }`
  variant, the `XrdsActionKnown` shadow entry, the `From` arm, the
  `KNOWN_ACTION_KINDS` wire tag, **and two new diagnostics**.
- `xrds-runtime` — the `on_start` arm (fire-and-forget: returns `true`
  immediately rather than stalling choreography until particles die) plus
  `fire_effect_in_world` in `helper.rs`.
- Editor — `XrdsActionDto::PlayEffect`, the string factory, both conversions.
- Frontend — the `XrdsAction` union member plus all four near-duplicate constant
  sets (`TRACK_ACTION_KINDS`, `ACTION_KINDS`, `ACTION_ICONS`, `ACTION_COLOR`,
  `summarizeAction`). There is still no shared constant between
  `sequencer.ts` and `SequencerInspector.tsx`; this phase inherits that
  duplication rather than fixing it.

**`count: Option<u32>`** overrides the node's authored `burst_count` when `Some`,
so one effect node can be fired at different intensities from different triggers
without duplicating it. `None` is the normal case and reads as
"PlayEffect (authored count)" in the Sequencer.

**Two diagnostics, because both failure modes are invisible on-device:**

- *Error* — `PlayEffect` targeting a node with no effect payload.
- *Warning* — `PlayEffect` targeting an effect whose `auto_play` is **on**. Not a
  type error, just impossible: one-shot pacing spends itself at scene load, so
  the action can never fire it. The message says to turn Auto Play off.

The runtime logs a matching warning when a fire queues nothing, since the static
check can't cover `SelfNode`/`TriggerSource` targets that resolve at fire time.

**Tests** (`xrds-scene-graph/tests/effect.rs`) cover the two hand-synced,
non-compiler-enforced sites: miss `KNOWN_ACTION_KINDS` or the shadow enum and the
action silently deserialises as `Unknown` — Track still loads, key keeps its slot,
nothing fires, no error anywhere. A second test asserts both diagnostics fire and
that a correctly-wired `auto_play: false` effect raises neither.

**A serde constraint found while testing:** `{"kind":"PlayEffect"}` with no
`data` does **not** parse. Serde's adjacent tagging requires `content` for a
struct variant, and `#[serde(default)]` on the inner field cannot supply a
missing `data`. This matches every other struct-shaped action
(`PlayGltfAnimation`, `SetTransform`) and the editor always emits `data`, so it is
a consistency rather than a gap — but hand-written JSON needs `"data":{}` at
minimum. My first test asserted the data-less form worked; the code was right and
the test was wrong.

**Not yet verified on device.** `PlayEffect` is tested at the document and unit
level but has not been fired on hardware. That wants a scene with a zone or
button trigger wired to an idle burst.

---

### Phase 5a — visual fidelity fields: done

Five curated fields added end to end (descriptor → `EffectParams` → scene format
→ editor DTO → Inspector UI). Workspace clean; 203 runtime + 183 scene-graph + 35
editor tests; `tsc` clean; 122 vitest; build ok.

| field | backend | why |
| --- | --- | --- |
| `blend` (`XrdsEffectBlend`) | `BlendMode` | **`Add` is the only way to get glow here** — the XR cameras have no bloom pass, so overlapping-brightness is the whole toolkit |
| `size_end` | `scale_curve` (2 stops) | grow/shrink over life |
| `drag` | `linear_drag` | separates smoke that settles from shrapnel that coasts |
| `fade_edge` | `fade_edge` | rounds the quad into a puff |
| `fade_scene` | `fade_scene` | kills the hard cut line where particles meet geometry |

**Curated to three blend modes** — `Blend`, `Add`, `Multiply`. `Opaque` and
`Premultiplied` stay expert-layer. Like `XrdsEffectKind`, the wire enum
(`XrdsSceneEffectBlend`) is separate from the runtime one so the serialized names
are a format contract.

**Defaults were chosen to change nothing.** `drag: 0.2`, `fade_edge: 0.7`,
`fade_scene: 1.0` are exactly `bevy_firework`'s own `ParticleSettings` defaults,
which `build_particle_spawner` was already inheriting through
`..Default::default()`. So effects have *always* had soft edges and scene fading;
this phase only makes them tunable. Existing scenes look identical, and a test
pins those values so a future field addition can't silently drift them to zero.

`size_end` maps to a two-stop `FireworkCurve` from `1.0` to `size_end`, which is
identical to the previous `constant(1.0)` at its default. A single scalar rather
than a curve editor, matching the `color_start`/`color_end` pair — a real curve
editor remains out of scope.

**Unknown `blend` strings are rejected, not defaulted**, in both the editor
command handler and serde. A test asserts `{"blend":"Screen"}` is an error: a
silent fallback to `Blend` would make a typo look like a rendering bug.

**Still unexposed, in rough value order** if this is ever revisited:
`CountOverDuration` pacing (a third kind: "emit N over T seconds"),
`EmissionMode::Nested` (particles spawning particles — the crate's namesake
fireworks, needs the one-`ParticleSettings` cap lifted),
`base_color_texture` (sprite particles, the largest fidelity jump, needs asset
plumbing), and `emissive_color` (pointless without a bloom pass).

---

### Phase 4 follow-up — `PlayEffect` did nothing for half the cases

Found by visual check in the editor: a `PlayEffect` key in the Sequencer fired
nothing, for **both** a Burst and a Trail. The "both" was the diagnostic clue —
it pointed at the mapping rather than at the Sequencer.

**Root cause.** `ParticleSpawnerData::queue_particles` only increments
`manual_queued_count`, and bevy_firework only *drains* that counter under
`EmissionPacing::OnDemand` (`core.rs:400`). Two of the four authorable
combinations therefore ignored the queue entirely:

| kind | `auto_play` | pacing | first cut |
| --- | --- | --- | --- |
| Burst | false | `OnDemand` | worked |
| Burst | true | `OneShot` (self-disables at load) | **silent no-op** |
| Trail | false | rate | **silent no-op** |
| Trail | true | rate | **silent no-op** (already running, so looked fine) |

Worse, the old code *returned `true`* in every case — it had queued a number
nobody would read — so even the "did nothing" warning never fired.

**Fix.** `fire_effect_in_world` now does what the kind actually needs:

- **Trail** — enable emission. If it is already `active()`, do nothing rather
  than re-inserting, which would clear live particles and make a running plume
  hiccup on every trigger.
- **Burst** — ensure `OnDemand` pacing (re-inserting only when it is not already
  so, to avoid discarding a previous burst's particles), then queue.

Re-inserting `ParticleSpawner` is the mechanism for re-enabling emission, because
`EmissionData::enabled` is private; `sync_spawner_data` re-derives it from
`starts_enabled` on any `Changed<ParticleSpawner>`, and it does not touch
`manual_queued_count`, so queueing across a re-insert is safe.

**Semantics changed deliberately:** `auto_play` now governs load-time behaviour
*only*. `PlayEffect` works regardless of it. An author wiring a trigger to an
effect has stated their intent unambiguously, and the old design turned a
reasonable setting into a silent failure.

Consequently the "Effect fires itself on load" diagnostic was **wrong** — it
claimed the effect "cannot fire again". Reworded to what is now true: the effect
will *also* fire once at load, which is usually unwanted, so turn Auto Play off if
it should only fire from the Track. The runtime warning was likewise narrowed to
the two real failures (not an effect node; count resolved to zero).

**Tests** cover all four combinations (`play_effect_fires_every_authorable_
combination`), the `count` override, and firing a non-effect node. Note the return
value alone would *not* have caught the original bug — the assertions on pacing
and `starts_enabled` are what fail against the old implementation.

---

### Phase 4b — `StopEffect` + per-row `When Finished`

Prompted by editor testing, which surfaced a chain of problems: `PlayEffect` fired
nothing for a Trail; the Stop button was disabled exactly when cleanup was needed;
resetting on completion then deleted an effect the same frame it fired. Rather
than keep patching, this follows what other engines do — verified against their
docs rather than recalled.

**How other engines handle it.** Unreal Sequencer has a per-track **`When
Finished`**: *Restore State* (default) or *Keep State*, which "determines on a
per-track basis whether tracks should return to their pre-animated state or keep
changes when the sequence finishes" — explicitly including particle tracks. Unity
distinguishes `StopEmitting` from `StopEmittingAndClear`; Niagara distinguishes
`Deactivate` from `Kill`. Godot keys `emitting` directly. The common thread: **the
engine never infers this — the author states it.**

**`XrdsAction::StopEffect`** — a soft stop: emission ceases, particles already
alive fade out (Unity's `StopEmitting`, Niagara's `Deactivate`).

Implemented by clearing `ParticleSpawnerData::emission`, and the mechanism is the
point: `EmissionData::enabled` is private, and *any* change to `ParticleSpawner`
triggers `sync_spawner_data`, which resets `ParticleSpawnerData::particles` — a
hard kill. With `emission` empty, `active()` is false so `spawn_particles` skips
the entity, while `update_particles` (a separate system, no `active()` gate) keeps
ageing live particles. `fire_effect_in_world` rebuilds when it finds `emission`
empty, so a stopped effect is re-fireable — needed for a Burst, whose pacing is
already `OnDemand` and would otherwise skip the rebuild and queue into a dead
spawner.

**Per-row `XrdsWhenFinished { Restore, Keep }`** on `XrdsTrackAsset`, defaulting
to `Restore` and skipped on serialise so existing documents are untouched. It
governs *natural completion only*; Stop and Play restore everything regardless,
since Stop means "reset" and a new run must not inherit the last one's leftovers.

Completion also **soft-stops** effects rather than re-applying params, so a burst
fired near the end fades instead of blinking out. That combination is what let the
hidden rule go: the previous `include_effects: false` special case — "never
restore effects on completion" — is **deleted**. The behaviour is now the author's
choice and visible as a per-row `rst`/`keep` toggle in the Sequencer.

**A wrong comment corrected while here.** `updaters.rs` claimed live particles
"keep their original settings until they expire" when an effect is retuned. False:
`sync_spawner_data` wipes them, so retuning restarts the population. Invisible on
a continuous Trail (it refills instantly), which is why it went unnoticed.

**Tests.** `StopEffect` wire round-trip (payload-less, so unlike `PlayEffect` a
bare `{"kind":"StopEffect"}` *does* parse); that it does not raise the
`PlayEffect`-specific auto-play warning; that it leaves `ParticleSpawner`
untouched (the invariant that makes it soft rather than hard); that a stopped
Burst re-fires; `When Finished` defaults, Keep round-trip, and rejection of
unknown values.

**Test-harness limit worth knowing.** `active()` going false and particles
actually ageing out **cannot** be unit-tested: `ParticleSystemPlugin` is
`#[cfg(not(test))]` (it needs the RenderApp), so bevy_firework's systems never run
in the headless harness and `emission` stays empty. `EmissionData` has private
fields and no constructor, so a populated emission cannot be faked. The tests
guard the invariant; the visible fade needs a desktop or on-device check.

**Still owed:** on-device verification of `PlayEffect`/`StopEffect` and `Add`
blending. `StopEffect` is confirmed working in the editor.

---

### Follow-up — the "playhead stops at PlayEffect" report, and a reverted fix

A Track with no explicit duration whose only event is `PlayEffect` ends at that
event's own timestamp: `effective_duration_secs()` is
`max(at_secs + self_duration_secs())`, and an instantaneous action contributes 0.
So firing and completing coincide, the playhead cannot pass the event, and its
result is undone before it is visible.

**Attempted fix, reverted: padding the auto duration.** Appending a 0.5s tail
after a trailing instantaneous event fixed the playhead — and broke rapid
re-firing. The agent (and its asset locks) stayed alive past the last event, so a
second firing was refused as "another Track already holds its assets". Two
existing tests caught it immediately: a threshold crossing that should fire on
both the up- and down-crossing fired once, and a disable-then-re-enable sequence
stopped removing its marker. **Runtime timing is not worth bending to fix an
authoring-time surprise**, so this was rolled back in full.

**Shipped instead: an authoring diagnostic.** A warning when an auto-duration
Track has a `PlayEffect` at its very end and that row is `Restore`, telling the
author to set a duration or switch the row to `Keep`.

Scope matters here. The first version warned about *any* instantaneous last
event, and three existing tests immediately caught it firing on healthy documents
— a trailing `SetVisible` or `ModifyHealth` is entirely normal, because its result
persists and is plainly visible. Only an effect needs time on the clock to be seen
at all. Narrowed accordingly, and silent on `Keep` rows since those already mean
"leave it running".

**Also:** the per-row control was reported as impossible to find. It was a bare
grey glyph; it is now a filled `RST`/`KEEP` chip with a tooltip that states the
consequence ("what it did to this node stays…") rather than naming the mode.

### Resolution — effects-only Tracks supplement their own duration

Settled after the reverted blanket padding above. Two changes, both scoped by the
lesson that attempt taught.

**1. An effects-only Track grants itself a tail.** In
`effective_duration_secs`, when there is no explicit `duration_secs` and *every*
key is an effect action and the last event is instantaneous, the Track runs for
`EFFECT_ONLY_TRACK_TAIL_SECS` (2.0s) past it. Sized against `XrdsEffect`'s own
1.5s default `lifetime_secs`, so a default burst completes rather than being cut
off.

Why this is safe where the blanket version was not: the Tracks that broke were
`ModifyHealth` and `SetElementEnabled` reactions. Those are fire-and-forget and
*must* release their asset locks immediately, or a rapid re-fire is refused. An
effects-only Track is the opposite — its entire purpose is something with a
visible lifetime. Mixed Tracks still get no tail, for the same reason.

**2. The Duration control is now a control.** It was a dim label beside an
unstyled box and read as decoration. It is the setting an author reaches for when
a Track misbehaves, so: uppercase label, proper field styling, and — the useful
part — a placeholder showing the value actually in use (`auto 3.5s`) instead of a
bare "auto". The tooltip explains both modes, including that an effects-only Track
gets extra time, and that events past a fixed duration never fire.

**Consequently the warning narrowed itself.** `effect_on_track_end_diagnostics`
now asks `effective_duration_secs()` rather than recomputing the end, so
effects-only Tracks — which fix themselves — no longer trigger it. What remains is
the genuinely still-broken case: a `PlayEffect` at the end of a *mixed* Track,
which gets no tail. Tests cover effects-only staying silent, mixed warning, and a
`Keep` row silencing it.

### Sequencer — drag an event along its lane to retime it

Keys are draggable horizontally, committing through the existing `SetTrackKey`
command (no new plumbing: it already carries `at_secs` and the Rust handler
already re-sorts the row).

Details that matter more than the drag itself:

- **Pointer capture** on pointer-down, so a drag survives the cursor leaving the
  lane instead of stopping dead at the boundary.
- **A 3px dead zone.** Without it every click carrying a one-pixel twitch would
  silently retime the event — a destructive edit disguised as a selection.
- **Selection follows the event.** Rust re-sorts the row by time, so the dragged
  key's index changes. The same stable sort is mirrored locally to find where it
  landed; otherwise the selection would stay on whatever slid into the old slot.
- **Snap to 0.05s, Shift for fine.** Round numbers at normal zoom without giving
  up precision.
- Clamped to `[0, duration]`, `grab`/`grabbing` cursors, and the dragged key is
  raised so it is never hidden behind one it is dragged past.

### On-device finding — `blend` is a no-op in `bevy_firework` 0.8

Three otherwise-identical effects were placed side by side on a Quest 3 —
`Blend`, `Add`, `Multiply` — and rendered **indistinguishably**. Not a tuning
problem; the backend ignores the setting:

- `render.rs:875` hardcodes `blend: Some(BlendState::ALPHA_BLENDING)` on the
  fragment target rather than deriving it from `alpha_mode`.
- The value *does* reach `FireworkUniform.alpha_mode`, but `particles.wgsl`
  declares that field and never reads it.

So no blend mode can differ, in any scene, on any platform. Our side is plumbed
correctly (descriptor → params → scene format → both the material data and the
shader uniform), which is why this was invisible until something was actually
looked at on a headset.

**This retracts a claim made earlier in this document.** Phase 5a introduced
`blend` with `Add` billed as "the only way to get glow here", since the XR cameras
have no bloom pass. That was wrong: with this backend there is currently **no** way
to get an additive glow. The reasoning about bloom was sound; the conclusion that
`Add` provided a substitute was not, and it was never verified on device before
being written down.

**Kept rather than removed.** The value is stored and travels correctly, so it
starts working the moment upstream honours it; deleting it would mean threading the
same field back through five layers later. But it is now marked non-functional in
the Inspector — a control that silently does nothing is the same trap as the
grabbable checkbox on an effect, which this plan already fixed once.

**Diagnostic worth reusing:** three variants side by side turned "Add is hard to
notice" — which had been sitting unresolved through two rounds of colour tuning —
into a definite answer in one look. Two variants could not distinguish "subtle" from
"broken"; three could, because `Multiply` should have been obvious.

### On-device pass — results

Quest 3, Adreno 740, Vulkan. Zero panics, zero wgpu errors across every run.

**Passed:**

- Effects still render after Phases 3/4/4b/5a — no regression from any of it.
- `auto_play: false` genuinely idles on device: the plume stayed invisible until
  fired.
- **`PlayEffect` and `StopEffect` fired from a Track**, driven by a real trigger
  (`Grabbed` on a cube). Log: `GRABBED` ×5, `fire_and_stop` started, and **zero**
  `trigger-action ... did nothing` warnings — so both actions found their target
  and did work. The plume started, ran its 3s, and faded rather than vanishing.
- Locomotion and controller visuals, once the deleted `PostStartup` registration
  was restored.

**Failed / found:**

- `blend` is a no-op — see the previous section. Not a device issue; the backend
  ignores it everywhere.
- **`ZoneEnter` cannot be fired by the player.** See below.

### SDK gap — a player cannot enter an `InteractionZone`

`zone_collision_system` (`xrds_api/zone.rs`) consumes avian3d
`CollisionStart`/`CollisionEnd`, so both bodies need colliders. Zones get
`Collider + Sensor + CollisionEventsEnabled` in `spawn.rs`, but **nothing gives the
player camera or player root a collider**, so no collision event can ever involve
the player. Confirmed on device: walking into a marked zone produced zero zone
events.

"Walk into a trigger volume" is a fundamental XR interaction, so this is a real gap
rather than a curiosity. It is **not** fixed here: giving the player a collider is a
design decision with consequences worth deciding deliberately — whether the player
pushes rigid bodies, what shape and height the body is, and how it interacts with
grab. Now planned separately in `docs/player-body-collider-plan.md`, which also
records a second blocker found while writing it up: the player has no `XrdsId`
either, so `zone_collision_system` would still drop the event even with a collider
present.

Workaround for authors today: trigger from `Grabbed`/`Dropped` on an object, or from
world-panel `ButtonPress` — all of which do work.

**Testing lesson, now in `docs/quest-device-test-recipe.md`:** an unmarked trigger
volume is indistinguishable from a broken one. Two attempts were spent assuming the
volume had been missed before checking whether the event could fire at all. A
visible marker mesh, plus one grep for the event in the log, would have found it
immediately.
