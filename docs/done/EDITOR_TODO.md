# XRDS Editor — Feature Todo List (archived)

**Archived 2026-08-12.** Nearly everything here shipped. Every status below was
re-verified against the code at archival: three items listed as open turned out to be
done, two were half-done, and one "completed" claim turned out to be false. The
corrections are inline.

**The items still genuinely open moved to `OVERALL_PROGRESS.md`**
("Missing Parts / Remaining Work") so they stay somewhere live rather than being buried
in `docs/done/`. Each is marked below with a pointer.

This list also predates much of what it never covered — particle effects
(`XrdsEffect`), `XrdsCapsule`, the Track/Sequencer system and panel/widget templates all
landed after it was last touched, each with its own doc in `docs/done/`.

---

Features needed to make the editor a capable XR content authoring tool.
HMD / OpenXR runtime integration is out of scope here.

Each item notes whether it is **editor-only** (SDK already supports it) or requires
**SDK work** first.

---

## Completed ✅

### P0 — Critical basics

- ✅ **Delete selected node** — `Del` key removes the node and all descendants.
- ✅ **Duplicate selected node** — `Ctrl+D` clones the subtree with fresh IDs.
- ✅ **Inline rename in hierarchy** — double-click a node label to edit the name in place.
- ✅ **Spot Light inspector** — inner/outer cone angles (°) and shadows checkbox exposed.
- ✅ **Camera far plane** — editable DragValue alongside near/FOV.
- ✅ **Audio autoplay checkbox** — `XrdsSceneAudioClip::autoplay` exposed in the inspector.
- ✅ **Ambient Light in palette + inspector** — palette entry and full inspector section added.

### Viewport & debug overlays

- ⚠️ **Wireframe toggle** — *this claim is no longer true.* It was built on Bevy's
  `WireframeConfig.global`, which is gone from the tree entirely: `WireframePlugin` is
  unsupported on WebGL/WebGPU and crashed. Selection highlighting now uses
  `bevy_mod_outline` (`OutlineVolume`, `bevy_scene.rs:79`). Do not reintroduce
  `WireframePlugin`.
- ✅ **Floor grid** — XZ plane reference grid (±10 m, minor 1 m / major 5 m lines, coloured origin axes); toolbar toggle.
- ✅ **Light ray debug (All / Sel)** — two independent toolbar toggles:
  - *Rays: All* — draw light shapes for every visible light node.
  - *Rays: Sel* — draw light shapes only for the selected node.
  - PointLight: 6 axis rays + 3 range circles.
  - SpotLight: axis ray, 8 cone-edge lines, outer + inner cone circles.
  - DirectionalLight: sun icon (disk + 8 radial spokes + direction arrow).
- ✅ **Selection outline** — orange wireframe on selected mesh (walks async glTF child tree).

### Gizmo

- ✅ **Constant screen-size gizmo** — scales with `camera_distance * 0.18` (min 0.4) so handles are always reachable.
- ✅ **World-space translate arrows** — arrows always `+X/+Y/+Z` regardless of node rotation.
- ✅ **Stable gizmo drag** — projects from fixed `drag.origin` each frame, no drift.
- ✅ **Wider hit target** — `GIZMO_HIT_PX` raised to 20 px.
- ✅ **Arrow-key nudge** — in Translate mode: 0.1 m per press, Shift = 1.0 m.

### Lights

- ✅ **Live light color + param preview** — pending-pattern for all four light types; changes show in the viewport immediately; committed on mouse release.
- ✅ **Physical-unit labels** — `cd`, `m`, `lx`, `cd/m²` shown next to each slider.
- ✅ **Better default light intensities** — PointLight 10 000 cd, SpotLight 50 000 cd (10°/25° cone).
- ✅ **Light icon meshes** — flashlight GLB attached as a child of every PointLight/SpotLight/DirectionalLight node; correct -Z orientation.
- ✅ **Camera icon meshes** — camera body GLB attached to every Camera node.
- ✅ **Sun gizmo for directional lights** — circle disk perpendicular to light direction, 8 radial spokes, yellow direction arrow; camera-distance-aware size.

---

## P1 — XR authoring essentials

Required before real XR content (lighting environments, textured materials, scale editing) is possible.

- ✅ **Environment settings panel** — Fog (color, near/far distance) and Exposure (EV100) controls
  in the inspector's "Scene Environment" collapsing section. Always visible regardless of selection.
  IBL and skybox deferred until the asset catalog UI is built.

- ✅ **Emissive color** — emissive color picker added to the material section of the inspector.

- ✅ **Alpha mode selector** — Auto / Opaque / Mask / Blend dropdown + alpha cutoff DragValue
  (shown only when Mask is selected). Also fixed a pre-existing bug where alpha mode was always
  hardcoded to Opaque when reading from the document and when committing.

- ✅ **Double-sided toggle** — checkbox added to the material section.

- ✅ **Scale gizmo mode** — `S` key switches to scale handles in the viewport; per-axis cube
  handle drag scales the selected node on that axis. Toolbar "Scale [S]" button added.

- ✅ **Frame Selected** — `F` key re-centers the orbit camera on the selected node's world position.

- ✅ **Material texture slot UI** — shipped as `TextureSlotRows` (`Inspector.tsx:1137`),
  used by both material sections and fed the asset list directly, so the "blocked on
  asset catalog browser UI" caveat no longer applies.

- 🟡 **Passthrough / blend mode flag** — **SDK done, editor not.** `XrdsXrBlendMode`
  (`Opaque` / `AlphaBlend`) exists at `scene/node.rs:20`, and the runtime already handles
  `EnvironmentBlendMode` and the `fb_passthrough` extension. Only the editor toggle is
  missing. → `OVERALL_PROGRESS.md`

---

## P2 — Workflow improvements

Quality-of-life features for iterative XR content authoring.

- ✅ **Play mode** — Space starts play (snapshots document); Esc stops and restores the snapshot.
  Play/Stop button in toolbar. Scene is locked to read-only in play state (no gizmo drag, no edit).

- ✅ **Node copy / paste** — `Ctrl+C` copies the selected subtree; `Ctrl+V` pastes it as a sibling
  with fresh IDs. Clipboard persists across selections.

- ✅ **Multi-select** — `Shift+Click` adds to selection, `Ctrl+Click` toggles in both the hierarchy
  and the viewport. Gizmo moves to the centroid of all selected nodes; translate drag moves all
  of them simultaneously. Delete and arrow-key nudge apply to all selected nodes. Inspector shows
  a count summary when more than one node is selected. Copy/paste handles multiple subtrees.
  Box-select deferred to a follow-up.

- ✅ **Hierarchy right-click context menu** — Rename, Duplicate, Delete options on every node row.
  Editor-only.

- ✅ **Grid / snap** — hold `Ctrl` while dragging the translate gizmo to snap to the step size
  shown in the toolbar (click the button to cycle 0.1 / 0.25 / 1.0 m).
  Editor-only.

- 🟡 **Spatial audio parameters** — **SDK done, editor not.** `XrdsSceneAudioClip`
  already carries `distance_model`, `min_distance`, `max_distance`, `rolloff_factor` and
  `hrtf` (`payload.rs:1178-1186`). No inspector UI reads them yet.
  → `OVERALL_PROGRESS.md`

- ✅ **Scene metadata panel** — editable scene name and author fields always visible in the
  inspector's "Scene" collapsing section.
  Editor-only: fields already exist in the SDK struct.

---

## P3 — Advanced XR features

Needed for fully interactive XR applications; most require new SDK node types.

- ✅ **Interaction zone node** — shipped as
  `XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone)` with
  `XrdsInteractionZoneShape` and `XrdsGrabType`, plus palette and inspector entries.

  **Caveat worth carrying forward:** a zone cannot currently be entered by the *player* —
  zone events come from avian3d collisions and nothing gives the player a collider. See
  `docs/done/player-body-collider-plan.md`.

- [ ] **Spatial anchor node** — AR authoring primitive whose transform is defined relative to a
  named world anchor persisted across sessions.
  Requires new payload in `xrds-scene-graph` + OpenXR persist-anchor API at runtime.
  → `OVERALL_PROGRESS.md`

- 🟡 **Animation / timeline** — **partly superseded.** The Track/Sequencer system shipped
  (`docs/done/xrds-sequencer-v2-implementation-plan.md`, `xrds-track-model-plan.md`) and
  covers timed *action* sequencing, `SetTransform` included. A keyframe *curve* editor for
  transform/material properties is still unbuilt. The "exports as a glTF animation track"
  half is void — glTF *export* is retired project-wide. → `OVERALL_PROGRESS.md`

- [ ] **LOD groups** — author multiple detail levels per node; runtime selects by camera distance.
  Requires SDK support before editor work.
  → `OVERALL_PROGRESS.md`

- [ ] **Performance budget panel** — live read-only counters for triangle count, draw calls, and
  estimated texture memory for the current scene.
  Editor-only instrumentation; data available via Bevy diagnostics.
  → `OVERALL_PROGRESS.md`
