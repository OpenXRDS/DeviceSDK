# XRDS Editor — User Manual Outline

**Status: outline only.** Table of contents and section scope for a manual
covering the GUI editor (`xrds-editor`). Audience: someone building an XR
scene through the editor — no Rust, no Bevy knowledge required. This is a
different audience from `api-reference-outline.md`; a reader here may never
open a `.rs` file.

## Sources already available to draw from

- `OVERALL_PROGRESS.md`'s "What is complete" list — the most current census
  of what the editor actually does; use it to check this outline's scope
  against reality before writing, since features get added between doc passes.
- `docs/done/GUI_BASEMENT.md`, `GUI_OS.md`, `HUD_EDITOR.md`,
  `PLAYER_ANCHOR.md`, `APK_EXPORT_WORKFLOW.md` — design records for specific
  features, each cited in its section below. These explain *why* something
  works the way it does; the manual should explain *how to use it* and link
  to these for the reasoning, not repeat it.
- `docs/done/xrds-widget-template-plan.md` — the panel/in-world-UI system,
  by far the largest single feature not yet reflected in any user-facing doc.
- `apps/xrds-editor/src/components/KeyboardShortcutsModal.tsx` — the
  authoritative, current shortcut list (read the component, not a stale
  copy of it) for §13.
- Screenshots: none exist yet. This manual is screenshot-heavy by nature
  (palette icons, Inspector layout, the Panels workspace) — capturing those
  is real work to scope separately from the writing.

## 0. Introduction

- What the editor is; what you can build without writing code.
- Launching it (build-from-source note can stay minimal/linked to the API
  manual's platform notes — a GUI user building from source is already
  halfway to the other audience).

## 1. Interface Overview

- The five main panels: Hierarchy, Inspector, Palette, Viewport, Toolbar/
  Menubar. One short subsection each, screenshot per panel.
- Camera navigation (orbit/pan/zoom, WASD+QE fly) and the three gizmo modes
  (translate/rotate/scale) — pull the exact bindings from
  `KeyboardShortcutsModal.tsx` rather than re-deriving them.
- Selection: click, Ctrl+click multi-select, Escape to deselect.

## 2. Building A Scene

- Placing objects from the palette: primitives (Cube/Sphere/Cylinder/
  Capsule/Plane/Tetrahedron), lights (Point/Spot/Directional/Ambient),
  Camera, Audio Clip, Interaction Zone.
- Transform editing in the Inspector; gizmo dragging in the viewport.
- Multi-select, copy/paste, duplicate, delete, undo/redo.
- Hierarchy drag-drop reparenting; grouping under an Empty node.

## 3. Materials

- Base color, emissive, opacity, roughness, metallic, unlit, double-sided,
  alpha mode.
- Texture slots: assigning an imported texture asset to BaseColor/Normal/
  MetallicRoughness/Occlusion/Emissive from the Inspector dropdown.
- What needs importing first (the asset catalog) vs. what's built-in.

## 4. Physics

- The Physics Body dropdown (None/Static/Dynamic) and when each applies.
- Gravity Scale and Mass sliders — live-updating, no reimport needed.
- What each primitive's collider looks like (matches its visible shape;
  `Capsule`'s collider is the shape with rounded caps, not a plain cylinder).
- Grab and throw in Play Mode; Interaction Zones as invisible trigger volumes.

## 5. Lighting & Environment

- The four light types plus Ambient — what each is for, at a glance
  (point = bulb, spot = cone, directional = sun, ambient = fill).
- Environment preview: fog, exposure, IBL, skybox — live in the viewport.

## 6. Text

- Text3D and Extruded Text: placement, sizing, alignment, color.
- Both render as real 3D geometry — visible from any angle, not a flat
  overlay — worth saying explicitly since this was a real point of past
  confusion (see `docs/done/TEXT_ENTITY.md`).

## 7. Player & Camera Anchors

- The Player node and PlayerAnchor children — what "camera anchor" means,
  how the initial spawn anchor is chosen.
- Per-anchor FOV and exposure override.
- Source: `docs/done/PLAYER_ANCHOR.md`.

## 8. Panels — In-World UI

The largest section, and the one with the least existing user-facing
material — most of its source is a design doc (`xrds-widget-template-plan.md`)
written for a future contributor, not an editor user. This section has to
translate concepts, not just restate them.

- The Panels workspace: library / elements / canvas / inspector layout.
- Creating a panel template; the five element kinds (Label, Button, Image,
  Slider, Toggle); drag-to-position and per-element property forms on the
  canvas.
- Two ways to place a template: as a world-space `Panel` node (approach and
  press) vs. head-locked under a Camera Anchor (a HUD).
- **The HUD-specific rule**: a HUD works best info-only (Label/Image) —
  an interactive HUD element captures the pointer everywhere it sits in
  view, permanently, unlike a world panel which only captures when
  approached. (This policy is decided but its editor enforcement — greying
  out interactive templates when linking to a head-locked panel — was not
  yet built as of this outline; update this section once it lands, and
  until then say so plainly rather than implying a safeguard exists.)
- Wiring an element to a Track: per-instance trigger bindings, why two
  placements of one template can drive two different doors/buttons.
- The grab handle: how to move a placed panel (grab the bar underneath, not
  the face — the face stays reserved for pressing its buttons).

## 9. Sequencer — Tracks & Triggers

- Creating a Track; adding timeline keys and actions.
- Trigger bindings: ButtonPress, ZoneEnter/Exit, and the others; which
  target kinds each is available on (a Label offers none, a Button offers
  press/release).
- The Stop-binding case: one button starts a Track, another stops it.
- Source: `docs/done/xrds-trigger-action-v1.md`,
  `xrds-trigger-action-editor-plan.md`.

## 10. Play Mode

- Entering/exiting (F5, Escape); the crosshair + hint HUD.
- Locomotion: flying vs. grounded.
- What's different from edit mode: physics runs, Dynamic bodies move,
  Tracks and triggers fire for real.

## 11. Saving, Loading, and Templates

- Save/load, the file dialog, New Scene.
- Starter templates: Simple 3D, Basic Interactive, VR Experience
  (PlayerSpawn + locomotion), Platformer (kinematic gravity + jump) — what
  each pre-populates and who it's for.

## 12. Exporting

- Export as Application: what it bundles, the `cargo build --release` step,
  where the output lands.
- Scene GLB export is **retired** — say so plainly and explain why in one
  sentence (glTF can't represent panels/triggers/Tracks/anchors/zones) so a
  user who remembers an older version doesn't file it as a missing feature.
- Android/APK-specific export notes, if the GUI exposes that path — check
  current scope against `docs/done/APK_EXPORT_WORKFLOW.md` before writing,
  since export targets have changed over the project's life.

## 13. Keyboard Shortcuts & Tips

- Full shortcut table, transcribed from `KeyboardShortcutsModal.tsx` at
  write time (re-check it hasn't drifted before publishing, not just once
  at outline time).
- A short "common gotchas" list — candidates: the Linux window-resize
  Vulkan stutter (`README.md`), the WebView2 build-cache note (editor picks
  up a rebuild on relaunch now, but this was a real point of confusion
  historically).

## 14. Troubleshooting

- What to do when a scene fails to load, an asset shows as missing, or the
  viewport goes black — grounded in actual diagnosed failure modes from this
  project's history (several are recorded in `docs/done/*` postmortems)
  rather than generic advice.
