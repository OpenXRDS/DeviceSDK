# DeviceSDK — Editor TODO

Prioritised backlog for `xrds-editor-tauri` and the broader SDK.

Items 1–8 are editor polish. Items 10–12 are the core framework milestones needed before this is a real XR framework.

---

## 1. Export Application ✅ Done

Full `cargo build --release` → distributable XR app pipeline.
Details: [TODO_EXPORT.md](TODO_EXPORT.md)

---

## 2. Hierarchy UX ✅ Done

- [x] Right-click context menu — Rename, Duplicate, Delete
- [x] Double-click inline rename (Enter to confirm, Escape to cancel)
- [x] Drag-and-drop reparenting — drag any node onto another to reparent; drop-to-root zone at bottom
- [x] Expand/collapse arrows (▸/▾) for nodes with children; all start expanded

---

## 3. Copy / Paste scene nodes ✅ Done

- [x] `CopySelection` — deep-clones selected subtrees (roots only, skips covered children) into `EditorState.clipboard`
- [x] `CutSelection` — copy + delete originals
- [x] `PasteClipboard` — re-IDs all nodes, remaps parent chain, inserts under selected node or at root
- [x] `has_clipboard` snapshot field — Paste menu item and Ctrl+V gated on non-empty clipboard
- [x] Ctrl+C / Ctrl+X / Ctrl+V keyboard shortcuts; Edit menu updated

---

## 4. Scene environment inspector ✅ Done

- [x] Fog (enable toggle, color, start/end distance) — shown in inspector when nothing selected
- [x] Exposure (enable toggle, Brightness slider: 0=default, +5=brighter, -5=darker)
- [x] IBL section (enable toggle, asset ID fields) — wired to `SetIbl` command
- [x] Environment synced to Bevy runtime every frame — undo/redo and file-open both apply immediately
- [x] `environment.rs` handler persists to `doc.metadata.environment` via `session.edit()`
- [x] SDK fix: `XrdsReceivesEnvironment` marker added to `xrds-runtime` — editor camera opts in cleanly; workaround system removed

---

## 5. GLTF animation clip names ✅ Done

- [x] `XrdsUpdateContext::gltf_clip_names(id)` — uses internal `XrdsStoredGltfHandle` to look up loaded `Gltf` asset and return `(index, name)` pairs
- [x] `EditorState.gltf_clips` cache refreshed every frame in `update()` for all GltfAsset nodes
- [x] `build_payload_dto` uses cache to populate `NodePayloadDto::GltfAsset { clips }`
- [x] Inspector clip dropdown now shows real animation names (falls back to "Clip N" if unnamed)

---

## 6. Reduce bridge log noise ✅ Done

- [x] `is_structural_command()` predicate — spawns, deletes, file I/O, undo/redo, play mode, text commits → `info!`
- [x] All live-preview commands (SetTranslation, SetMaterial, SetPointLight, SetFog…) → `trace!` (silent by default)

---

## 7. Camera FOV live update ✅ Done

- [x] `set_camera_fov_for_node(id, fov_deg)` added to `XrdsUpdateContext` — modifies `Projection::Perspective.fov` in-place
- [x] Inspector `CameraSection` with scrub sliders for FOV (live preview), Near, Far (commit only)
- [x] `SetCameraParams` / `CommitCameraParams` commands with pending state pattern

---

## 8. Camera node viewport selector ✅ Done

Scene camera nodes were invisible in the editor — the orbit camera always rendered and there was no way to preview through a scene camera.

- [x] `apply_camera_selection_system` replaces `deactivate_scene_cameras` — respects `EditorState::active_camera_id`
- [x] `SetActiveCamera { id: Option<u64> }` command — None = editor camera, Some = scene camera node
- [x] Camera selector dropdown in toolbar — only appears when at least one Camera node exists in the scene
- [x] Switching to a scene camera deactivates the editor camera; switching back restores orbit view
- [x] `available_cameras` / `active_camera_id` added to `EditorSnapshot` for React binding
- [x] `active_camera_id` reset on NewScene / OpenScene / Undo / Redo

---

## 9. XR anchor modes

`XrdsTextAnchor` enum variants are declared in `xrds-components`. `World` (default) and
`Billboard` (always faces camera) are already working. The XR-specific modes below need
a PostUpdate system in `xrds-runtime` that reads `OpenXrPlayerRoot` / camera transform
and overrides the entity's world position each frame.

- [x] `HeadLocked` — text stays at a fixed head-relative offset (HUD display)
- [x] `BodyLocked` — follows locomotion position + yaw only, ignores head pitch/roll (hovering menu)
- [x] `ComfortPinned { depth_m }` — same as Billboard but Z-distance clamped to a comfortable range
- [x] `Cylindrical { radius_m }` — text wraps on a virtual cylinder centred on the player
- [x] Scene-graph `XrdsSceneTextAnchor` updated with new variants for save/load round-trips

---

## 10. OpenXR headset runtime (`crates/xrds-openxr`)

- [x] Stereo rendering — dual swapchain, view-indexed render targets, per-view cameras
- [x] Head pose tracking — `OpenXrViews` resource with position + orientation per frame, validity flags
- [x] Runtime detection + desktop fallback — `is_openxr_available()` (Windows registry / Unix lib probe)
- [x] Graphics backends — Vulkan, OpenGL, D3D12; swapchain format negotiation
- [x] Preview window alongside HMD (`preview_window` feature, on by default)
- [x] **Wired into `RuntimeParameters`** — `enable_xr: true` calls `xrds_openxr::add_plugins()`; falls back to desktop with a warning if no runtime found
- [x] Controller input — `XrInput` resource; Oculus Touch + KHR simple bindings; trigger, grip, thumbstick, select (A/X), menu (Y/B), thumbstick-click; edge detection (`select_just_pressed/released`, `menu_just_pressed`, `thumbstick_click_just_pressed`)
- [x] Hand tracking — `XR_EXT_hand_tracking`; joint locations; pinch-to-trigger derivation; seamless fallback when controller goes untracked
- [x] Controller mesh rendering — `XR_FB_render_model` attempted; PCVR (Quest Link) uses fallback ray pointer; standalone Quest path wired but untested
- [x] Haptic feedback — `XrHapticRequest` message; `H`/`J` test keys in xrds-app; `apply_haptic_feedback_system` in xrds-openxr
- [ ] Unblocks: Quest packaging (#11)

---

## 11. Android / Meta Quest build target

Target: **Quest 3 / Quest Pro and newer** (API 32+). Quest 2 is not supported.
Android XR (Google platform) is a separate placeholder — see `android/android-xr/README.md`.

- [x] `android/quest/AndroidManifest.xml` — `com.oculus.intent.category.XR`, `minSdkVersion 32`, GameActivity
- [x] `android/quest/README.md` — full build walkthrough (cargo-ndk → APK → adb)
- [x] `android/android-xr/README.md` — placeholder with diff table and TODO list for when hardware is available
- [x] `.cargo/config.toml` — Android cross-compile section documented
- [x] `apps/xrds-app`: `[lib] crate-type = ["cdylib"]` added; `android_main` entry point wired to `bevy_winit::ANDROID_APP` + logcat
- [x] Asset pipeline — `android/quest/build.sh` bundles `assets/` into APK; `android_main` auto-selects APK or external-storage mode; Bevy `AssetServer` paths stay relative in both
- [ ] Export Application pipeline extended: detect target platform, emit `.apk` instead of desktop binary
- [ ] Test on Quest 3 / Quest Pro via `adb install`

---

## 12. XR interaction primitives

Grab and HUD are the two most common patterns in XR apps. Both need SDK support before
they can be used cleanly in the exported app or the editor preview.

- [x] **HUD text** — `XrdsAPI::spawn_hud_label(text, offset)` spawns a `HeadLocked` text entity; `XrdsText`/`TextParams` promoted to top-level `xrds_runtime` exports; demo in xrds-app updates label on P key
- [x] **XR raycasting** — cast a ray from the aim pose against scene geometry without a
  physics engine; AABB slab-method intersection in `xrds-runtime`; `ctx.raycast(origin, dir, max_dist) -> Vec<XrRayhit>`
- [x] **Grab system** — `XrGrabbable` marker component; `XrGrabEvent` / `XrDropEvent` Bevy
  events fired by SDK; `XrGrabbed` component while held; transform follows hand aim pose;
  `api.make_grabbable(handle)` / `api.make_grabbable_by_id(id)` / `ctx.make_grabbable(id)`
- [x] **Pick-up in exported app** — all GltfAsset nodes marked grabbable in `setup()`; `grab_event_log_system` reads and logs `XrGrabEvent`/`XrDropEvent`

---

## 14. PlayerSpawn Zone ✅ Done

Volume primitive tagged as a spawn region; player teleports to a random point inside on load.

- [x] `XrdsScenePlayerSpawnZone` payload — `size: [f32; 3]`, `player_node_id: Option<u64>` (None = shared across all players)
- [x] `XrdsPlayerSpawnZone` runtime component — tagged on entities during `tag_spawn_zone_entities()` at import
- [x] `random_spawn_zone_position_in_world()` — uniform random XZ within zone footprint, filters by `player_node_id`
- [x] `XrdsAPI::random_spawn_zone_position()` / `random_spawn_zone_position_for(id)` — public SDK surface
- [x] Editor: palette entry, green wireframe gizmo, inspector size sliders + Player dropdown
- [x] `xrds-app`: calls `api.random_spawn_zone_position()` on setup and teleports player if a zone is present

---

## 13. Physics integration

XR interaction (grabbing, throwing, collisions) requires a physics runtime. Pure transform manipulation is not enough.

- [x] Choose backend — `avian3d 0.4` (XPBD, native Bevy 0.17)
- [x] `XrdsPhysicsBody` node payload — `None` / `Static` / `Dynamic` variants; serialised in scene graph; editor dropdown in inspector
- [x] `XrdsCollider` component — cuboid, sphere, cylinder, half-space plane; `SweptCcd` on dynamic bodies
- [x] Gravity and mass properties exposed via `XrdsAPI` and editor inspector
- [x] Editor: collider visualisation gizmo (wireframe shape overlay)
- [x] Interaction zone (`XrdsInteractionZone`) upgraded to use physics trigger volumes
