# Stereo Preview Feature Plan

Side-by-side left/right eye preview in the desktop editor viewport — no HMD required.

---

## Context & Motivation

Debugging stereo-specific bugs (e.g. HUD visible in left eye only) currently requires an APK build + Quest 3 deploy cycle (~15 min round trip). A stereo preview viewport collapses this to an in-editor iteration loop.

**VIS-DIAG finding (2026-06-23):** The head-locked HUD is present in *both* XR cameras' `VisibleEntities` (CPU-side visibility is correct). The missing right-eye HUD is a render-world issue — camera ordering, extraction, or per-camera phase population in Bevy's render pipeline. This is very hard to instrument without a desktop repro. The stereo preview would let us reproduce and fix it on desktop.

---

## Architecture Recap

The editor renders via a **"hole in WebView"** model:

```text
React SPA (full-window wry WebView overlay)
  └── SetWindowRgn carves a rectangular hole in the overlay
        └── Bevy native DXGI renders through the hole
              └── Single EditorCameraMarker camera with viewport = hole rect
```

React sends viewport bounds via IPC → Rust updates camera viewport + hole region each frame. No render-to-texture; no texture copy to JS.

Adding a second camera is low cost: both cameras write to the same OS window via their respective viewport rects, and the single `SetWindowRgn` hole exposes both halves.

---

## Feature Design

### Mode

A **Stereo Preview** toggle in the toolbar. In stereo mode:

- Viewport hole covers the full width (same as now)
- Left half → left-eye camera (existing `EditorCameraMarker`)
- Right half → right-eye companion camera (`EditorStereoRightCamera`)
- React shows **L** / **R** corner labels inside the viewport
- IPD and FOV are configurable (defaults: IPD=63mm, HFOV=90°)

In mono mode (default): unchanged from today.

### Camera Setup

```text
Mono:   [    EditorCamera (full width)    ]
Stereo: [ EditorCamera (left) ][ RightCam (right) ]
```

Both cameras use the same `PerspectiveProjection` FOV. The right camera is a positional clone of the left camera, offset by `+right * IPD/2` in world space (left camera gets `-right * IPD/2`).

For accuracy, the per-eye FOV can be asymmetric (Quest 3: ~90° H / ~97° V with slight inward skew), but symmetric 90° is sufficient for Phase 1.

### head_locked Anchor Behavior

`head_locked_system` uses `pick_head_camera` which today requires `Projection::Custom` (an XR camera). In the editor there is no XR camera, so head-locked entities are not updated — they stay at their authored position.

**Phase 2** will add a fallback: if no XR camera exists and stereo preview is active, treat the left editor camera as Camera 0 for `head_locked_system`. This lets us reproduce the HUD stereo render-world bug on desktop.

---

## Implementation Plan

### Phase 1 — Split Viewport (no anchor changes)

**Goal:** Two cameras, side-by-side, showing the same scene from slightly different positions. No HMD needed for basic stereo parallax testing.

#### 1a. IPC Protocol (React → Rust)

Add a `stereo_preview` message:

```ts
window.__xrds__.send("stereo_preview", { enabled: boolean, ipd_mm: number, fov_deg: number })
```

Rust handler in `wry_overlay.rs` stores into a `StereoPreviewState` resource.

#### 1b. Stereo Camera Companion (`viewport_camera.rs`)

```rust
// StereoPreviewState { enabled: bool, ipd_m: f32, fov_deg: f32 }  (Bevy Resource)
```

System `update_stereo_preview_camera`:

- When `enabled` transitions `false→true`: spawn `EditorStereoRightCamera` entity
  with `Camera { viewport: Some(right_half), order: 1, .. }` + `Camera3d`
- When `enabled` transitions `true→false`: despawn it
- Each frame when enabled: copy left-camera transform, apply `+right * ipd/2`; left camera gets `-right * ipd/2`

Left camera viewport = `[x, y, w/2, h]`
Right camera viewport = `[x + w/2, y, w/2, h]`
`SetWindowRgn` hole = `[x, y, w, h]` (unchanged from mono — already full width)

#### 1c. Viewport Bounds IPC Handling (`wry_overlay.rs`)

When stereo mode is active, `drain_responses_and_viewport` splits the received rect into two halves and updates both cameras. The `SetWindowRgn` hole stays as the full viewport rect.

#### 1d. React Toolbar Button

A `StereoPreviewToggle` button in the toolbar (near the play button). Sends IPC on click. Subscribes to editor snapshot field `stereo_preview_active: bool` for toggle state sync.

React viewport component renders `<div class="eye-label left">L</div>` / `<div class="eye-label right">R</div>` overlays (positioned in the hole area via CSS) when stereo mode is active.

#### Files Changed

| File | Change |
|------|--------|
| `apps/xrds-editor/src-tauri/src/viewport_camera.rs` | `StereoPreviewState` resource, `update_stereo_preview_camera` system, viewport split logic |
| `apps/xrds-editor/src-tauri/src/wry_overlay.rs` | IPC handler for `stereo_preview` message, hole stays full-width |
| `apps/xrds-editor/src-tauri/src/bevy_scene.rs` | Register `update_stereo_preview_camera` in PostUpdate; add `stereo_preview_active` to editor snapshot |
| `apps/xrds-editor/src/components/Toolbar.tsx` | Stereo toggle button |
| `apps/xrds-editor/src/components/Viewport.tsx` | L/R corner labels |

---

### Phase 2 — Anchor / Head-Locked Simulation

**Goal:** Head-locked entities (HUD text, UI panels) behave correctly per-eye in the editor, reproducing the stereo render-world bug on desktop.

Changes to `head_locked_system` (or a new `editor_head_locked_system`):

- If `StereoPreviewState.enabled` and no XR camera exists, pick the left editor camera as Camera 0
- Position the HUD relative to that camera, same math as the XR path
- This exercises the same Bevy multi-camera rendering path that fails on Quest 3

Once we can reproduce the HUD-missing-right-eye bug on desktop, we can attach RenderDoc or add render-world logging to find the root cause.

#### Additional Phase 2 Items

- Asymmetric FOV option (Quest 3 actual values)
- IPD slider in editor UI (range 55–75mm)
- Crosshair / convergence plane indicator overlay
- Option to show each eye's frustum wireframe in the mono view

---

## Open Questions

1. **Camera order conflict**: Editor camera has `order: 0`. The right-eye companion needs `order: 1`. Does this conflict with XR cameras (also `order: 0/1`) if XR and editor run simultaneously? (They won't — XR mode and editor mode are mutually exclusive by design.)

2. **Deactivating editor cameras in play mode**: `deactivate_scene_cameras` in `xrds-app` only deactivates `RenderTarget::Window` cameras. The editor stereo camera uses a window target — confirm it gets deactivated correctly when entering XR play mode.

3. **Render-world bug root cause candidates**: After Phase 2 desktop repro:
   - Per-camera render phase population skipping Camera 1
   - `RenderVisibleEntities` extraction ordering
   - `bevy_rich_text3d` mesh update running after render-world extract for one camera only

---

## Current HUD Bug Status

| Layer | Status | Evidence |
|-------|--------|----------|
| CPU visibility (`VisibleEntities`) | ✅ Correct | VIS-DIAG: `has_hud=true` for both XR cameras |
| `NoCpuCulling` fix | ✅ Applied | `session.rs` updated |
| Render world extraction | ❓ Unknown | Need render-world diag or desktop repro |
| Projection / depth | ❓ Unknown | Could be reverse-Z edge case for Camera 1 |

**Next step for the Quest bug:** Implement Phase 2 of this plan (desktop repro), OR add `[RENDER-DIAG]` logging inside Bevy's `queue_material_meshes` / `batch_and_prepare_render_phase` for Camera 1.
