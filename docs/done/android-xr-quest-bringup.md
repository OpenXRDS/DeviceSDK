# Android XR / Quest 3 Bringup — Debugging Notes

Findings from the initial bringup of the XRDS runtime on Meta Quest 3 (June 2026).
These are non-obvious architectural constraints, not general Rust/Bevy/OpenXR basics.

---

## Bug 1 — Font panic on Android (`no default font found`)

**Symptom:** App crashes immediately on device with a panic from `cosmic-text` inside
`bevy_rich_text3d`.

**Root cause:** `runtime.rs` was inserting `bevy_rich_text3d::LoadFonts` with paths like
`assets/fonts/NotoSans-Regular.ttf`. These resolve to filesystem paths. On Android the APK
assets are not accessible as filesystem paths — they live inside the APK archive and are
only reachable via `AAssetManager`. `cosmic-text` panics at first text render if no fonts
were loaded.

**Fix:** `crates/xrds-runtime/src/runtime.rs` — filter font paths with
`.filter(|p| p.exists())` before inserting `LoadFonts`. Only insert the resource when at
least one path actually exists on the filesystem (dev mode with external storage) and skip
it silently in APK mode.

---

## Bug 2 — bevy_winit patch `Cargo.toml` had invalid `resolver = "2"`

**Symptom:** Build error about invalid Cargo manifest field.

**Root cause:** The patch was initially copied from the cargo registry cache which includes
`resolver = "2"` in `[package]`. That field is only valid in workspace root manifests.

**Fix:** Remove `resolver = "2"` from `patches/bevy_winit/Cargo.toml`.

---

## Bug 3 — bevy_winit event loop stalls after ~6 frames (the core XR blocker)

**Symptom:** App launches, `handle_events` logs "tick frame=0" through "tick frame=6",
then nothing. The OpenXR session state machine freezes in `IDLE` forever.

**Root cause:** A chain of three facts conspires:

1. Quest's VR compositor calls `Activity.onPause()` when it takes over the display. Android
   then destroys the window surface.
2. Bevy sets `ControlFlow::Wait` in `Continuous` mode (non-desktop path). To keep the loop
   alive it calls `window.request_redraw()`. With no surface, `request_redraw()` is a
   no-op — no `RedrawRequested` event is ever dispatched.
3. bevy_winit guards `app.update()` behind `!ran_update_since_last_redraw`. Without a
   `RedrawRequested` event to reset that flag, it stays `true` permanently and
   `run_app_update()` is never called again.

The `OpenXrSchedules::Update` schedule (which drives `xrWaitFrame`, session state
transitions, etc.) only runs inside `app.update()`, so the entire XR runtime freezes.

**Fix:** Two patches to `patches/bevy_winit/src/state.rs`:

1. **`ControlFlow::Poll` on Android** — in `about_to_wait()`, for `UpdateMode::Continuous`,
   set `ControlFlow::Poll` on Android instead of `ControlFlow::Wait`. The event loop then
   calls `about_to_wait()` continuously without needing any surface event to wake it.

2. **Reset `ran_update_since_last_redraw` on Android** — in `redraw_requested()`, for
   Android + `Continuous` mode, unconditionally reset `ran_update_since_last_redraw = false`
   so that `run_app_update()` fires on every loop iteration. The actual frame rate is
   then throttled by `xrWaitFrame` blocking ~11 ms per display period once the session is
   Running.

Why `ControlFlow::Poll` alone is not enough: without resetting the flag, `should_update()`
still returns false and `run_app_update()` is skipped.

Why the flag reset alone is not enough: without `ControlFlow::Poll`, the event loop still
sleeps forever waiting for a (never-arriving) event.

**Confirmation:** After the fix, `handle_events` frame counter increments at ~90 Hz
(throttled by display vsync while in IDLE state, later by `xrWaitFrame` when Running).

---

## Bug 4 — `xrEndFrame` rejects every frame with `ERROR_POSE_INVALID`

**Symptom:** Content visible in HMD (purple background + HUD text) but eye focus feels
wrong/flat (effectively mono). Logcat floods with
`XR: xrEndFrame failed callN: ERROR_POSE_INVALID`.

**Root cause (part A):** `OpenXrViews` was initialized with `openxr::View::default()`.
The default `openxr::Quaternionf` is all-zeros `(0, 0, 0, 0)` — not a unit quaternion,
hence not a valid rotation. `xrEndFrame` validates poses in the submitted composition layers
and rejects the frame.

**Root cause (part B):** Projection layers were built and submitted even before the session
reached `SYNCHRONIZED` state, i.e., before `openxr_locate_views` had ever run. The `run_if`
guard on `openxr_locate_views` (`openxr_in_state_synchronized`) correctly prevents locating
views before tracking is valid, but nothing prevented the stale zero-quaternion views from
being submitted in the composition layer.

The Quest compositor attempts to render anyway using a fallback pose, which is why content
was *visible* but *flat* — both eye cameras effectively shared identity pose with no IPD
offset.

**Fix (part A):** `crates/xrds-openxr/src/openxr/session.rs` —
initialize `OpenXrViews` with `Posef::IDENTITY` (orientation `w=1`) and a reasonable
default FOV (±45°) instead of `openxr::View::default()`.

**Fix (part B):** `crates/xrds-openxr/src/openxr/render.rs` — in `openxr_end_frame`,
check `OpenXrDeviceState` and only call `builder.build(world)` when the device is in
`Synchronized | Visible | Focused` state. Before that, submit an empty layer list
(`vec![]`), which is valid OpenXR and signals "nothing to display this frame" without
corrupting the compositor's pose history.

---

## Bug 5 — One eye's geometry invisible with GPU indirect drawing (multi-camera)

**Symptom:** Scene objects (e.g. a cube) render correctly in one eye but are completely
absent in the other. The floor (or any other object) renders in both eyes. Swapping which
eye gets `NoIndirectDrawing` flips which eye shows the object. Both cameras have identical
`RenderVisibleEntities` counts and identical `BinnedRenderPhase<Opaque3d>` bin counts.

**Root cause:** Bevy 0.17's `GpuPreprocessingMode::Culling` path (GPU indirect draw +
occlusion culling) maintains a global work-items buffer shared across all cameras in a
`MultidrawableMesh` phase. When two XR cameras both use `MultidrawableMesh`, their GPU
preprocessing dispatches reference overlapping work-item offsets, so one camera's
preprocessing clobbers the other's indirect draw parameters. The result is that exactly
one camera's geometry survives per object — whichever camera's dispatch runs second
overwrites the first's instance count.

This affects Android/Quest 3 only. Desktop (Windows/Linux) cameras are unaffected because
they use a single camera and the offset conflict never arises.

**Confirmed by bisect:**

- Both cameras GPU indirect → cube only in left eye
- Right eye CPU batching, left eye GPU indirect → cube only in right eye (swapped)
- Both cameras CPU batching → cube in both eyes ✓

**Fix:** Add `NoIndirectDrawing` to every XR camera in
`crates/xrds-openxr/src/openxr/session.rs` (`spawn_camera`). This forces
`GpuPreprocessingMode::PreprocessingOnly` for those cameras — GPU transform preprocessing
still runs but the indirect draw dispatch and GPU occlusion culling are bypassed. Each
camera issues direct draw calls via CPU-side batching.

**Trade-off:** GPU occlusion culling is disabled for XR cameras. For a sparse scene this
has no measurable impact. For a dense scene with heavy occlusion, draw call count will be
higher than optimal. A proper fix would require patching Bevy's GPU preprocessing to
isolate per-camera work-item ranges in the global buffer.

---

## Architecture notes

- `OpenXrSchedules::Update` is inserted **before `First`** in `MainScheduleOrder`. It runs
  at the top of every `app.update()` call. System set order:
  `HandleEvents → UpdateSessionStates → PreFrameLoop → WaitFrame → FrameLoop → PostFrameLoop`

- `openxr_wait_frame` only runs when `OpenXrSessionState::Running`. Before that, the loop
  runs at display vsync speed (Android Vulkan FIFO present mode).

- `openxr_locate_views` and `openxr_update_view_projection` run in `PostUpdate` with
  `run_if(openxr_in_state_synchronized)`. They must run before `TransformSystems::Propagate`.

- The render app runs synchronously on the main thread (`PipelinedRenderingPlugin` disabled).
  Render pipeline order: `BeginFrame → PreRender (acquire/wait swapchain) → camera render →
  PostRender (release/end frame)`.

- `handle_events` processes **exactly one** `xrPollEvent` per `app.update()`. With ~90 fps,
  IDLE → READY → SYNCHRONIZED → VISIBLE → FOCUSED transitions still complete in under 100 ms.

- Font assets inside the APK are NOT accessible as filesystem paths on Android. Only
  external storage paths (`/sdcard/Android/data/<pkg>/files/`) are filesystem-accessible.
  Bevy's `AssetServer` can read from APK via `AAssetManager` when `asset_path` is `None`.
