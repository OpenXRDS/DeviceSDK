# Text Entity Strategy in DeviceSDK

## Three Distinct Text Scenarios

### 1. 3D World Text (`XrdsText`)

**What it is:** Text tessellated into mesh geometry, rendered by the 3D pipeline.
**Plugin path:** `bevy_rich_text3d = "0.5"` (targets Bevy `^0.17.0`, confirmed compatible).
**When to use:** Scene labels, name tags, signage, any text that lives as a spatial object in the world.
**Why default:** XR apps are 3D-first. Text that floats in world space must be a real 3D object — it should cast light response, occlude and be occluded, and appear in exported apps without requiring a `Camera2d`.
**Status:** ✅ Implemented. `spawn_text_descriptor` uses `Text3d` + `Text3dStyling` from `bevy_rich_text3d`. No `Text2d` involved. Verified in `examples/3d_text.rs` — live color/content updates work.

#### Variant: Flat Billboard Text (Nameplate)

A sub-case of 3D world text used for character nameplates, health bars, or floating labels that must always face the player.

- **Depth = 0** — glyph mesh is flat, no extrusion. Purely cosmetic difference from a sign.
- **Billboard constraint** — a per-frame Bevy system overwrites the entity's rotation to align `-Z` toward the active 3D camera (world-up preserved). Does **not** require `Camera2d`. `bevy_mod_billboard` was evaluated and dropped — last supported Bevy 0.14, abandoned since mid-2024.
- **Parenting pattern:**

```text
Character (root)
  └── NameplateAnchor  (offset transform, e.g. +1.8 m on Y)
        └── XrdsText   (flat mesh, anchor: Billboard)
```

**SDK surface:** `XrdsTextAnchor::Billboard` variant. Default is `XrdsTextAnchor::World`.
**Status:** ✅ Implemented. `XrdsBillboard` marker component inserted at spawn time; `billboard_system` rotates the entity's `+Z` toward the active `Camera3d` every PostUpdate frame while preserving world-up. Anchor is persisted in the scene document via `XrdsSceneTextAnchor` (backward-compatible `#[serde(default)]`). Editor palette exposes a **Billboard** primitive — if spawned as a child it lands at Y=2.0 in parent-local space (nameplate height); standalone at Y=1.5 world-space.

---

### 2. Extruded (Volumetric) 3D Text (`XrdsExtrudedText`)

**What it is:** Text with real Z-depth — each glyph is a solid mesh with a front face, back face, and side walls. Responds to PBR lighting and casts depth as a physical object.
**Plugin path:** `bevy_fontmesh = "0.1.1"` (targets Bevy `^0.17.0`, confirmed compatible).
**When to use:** Architectural lettering, logo placement, signage that should read as physical objects in the scene. Distinct from `XrdsText` (flat mesh, atlas-rendered, efficient per-frame updates) in both look and cost.
**Status:** ✅ Implemented. `spawn_extruded_text_descriptor` uses `TextMesh` + `TextMeshStyle` from `bevy_fontmesh`. Verified in `examples/extruded_text.rs`. Wired to the editor — palette, inspector (text/font_size/depth/alignment/color), hierarchy icon.

**Key difference from `XrdsText`:**

| | `XrdsText` | `XrdsExtrudedText` |
| --- | --- | --- |
| Geometry | Flat quad mesh (atlas-based) | Full 3D solid mesh |
| Depth | 0 (flat) | Configurable (`depth: f32`) |
| Lighting | Unlit (self-lit atlas texture) | PBR (`StandardMaterial`) |
| Update cost | Cheap — change vertex color or atlas UV | Expensive — mesh regeneration required |
| Color update | In-place vertex color | In-place `StandardMaterial` (no reimport) |

**Runtime note:** Color-only changes update the `StandardMaterial.base_color` in-place (no entity despawn/respawn) via `pending_extruded_color` in the editor. Structural changes (text content, font size, depth, alignment) trigger a full reimport.

---

### 3. Surface / Overlay Text (`Text2d` on a 3D entity)

**What it is:** `Text2d` (or a render-to-texture quad) positioned in 3D world space, used as a surface element rather than a standalone spatial object.
**When to use:** Text rendered onto a cinema screen, video overlay caption, monitor display, or any in-world surface that acts as a 2D canvas.
**Why separate:** The content model is different — the text is *part of* a surface, not a freestanding entity. The authoring primitive should reflect that: a screen/canvas entity with a text payload, not a standalone text node.
**Note:** Requires `Camera2d` and is therefore unsuitable as a default in pure-3D exported apps.

---

### 4. UI Text (`bevy::ui`)

**What it is:** Screen-space text rendered by Bevy's UI pipeline.
**When to use:** HUD, menus, debug overlays, editor panels (e.g., the `xrds-editor` itself).
**Why separate:** UI text has no world-space transform. It belongs to the 2D UI layer and is outside the scope of scene-graph nodes entirely. Already handled by the editor's egui layer and Bevy UI — no `XrdsText` involvement.

---

## XR-Specific Variants (no equivalent in flat-screen engines)

All XR-specific variants are **3D mesh text** — same `XrdsText` geometry and 3D rendering pipeline, no `Camera2d` required. The anchor mode is purely a **transform-update policy**: a Bevy system that rewrites the entity's transform each frame according to the user's headset or body pose. The glyph mesh itself does not change.

Standard game engines only distinguish world-space vs. screen-space text. XR adds a third axis: **the text's spatial relationship to the user's body and head**. This determines comfort, legibility, and presence.

### Spatial Anchor Modes

**Head-locked** *(optional — deferred)*
Follows headset rotation exactly — always centered in view regardless of where the user looks. Use sparingly: persistent head-locked content causes motion sickness. Legitimate for urgent, brief notifications.

**Body-locked** *(optional — deferred)*
Follows the user's locomotion and torso orientation but *not* head rotation. Stays in a consistent region of the user's peripheral space (e.g., lower-left). Suitable for persistent status indicators.

**Comfort-zone depth pinning** *(optional — deferred)*
The text's world position is used for direction, but its depth is clamped to an ergonomic reading range (typically 0.5–2 m from the user). Prevents vergence-accommodation conflict. Requires headset pose data from `xrds-openxr`.

**Cylindrical / curved text** *(optional — deferred)*
Text wrapped around a cylinder centered on the user at a fixed radius. Most complex to implement — requires custom mesh layout projection at glyph level.

**Angular-size preserved text** *(optional — deferred)*
Text scales proportionally with distance to maintain constant angular size in the user's FOV.

### Anchor Mode Summary

| Mode | Status | Position anchor | Rotation anchor | Depth | Primary use case |
| --- | --- | --- | --- | --- | --- |
| World (default) | ✅ Done | World transform | World | World | Signs, architecture labels |
| Billboard | ✅ Done | World position | Camera-facing (preserves Y-up) | World | Nameplates, floating labels |
| Comfort-pinned | Optional — deferred | World direction | Camera-facing | Fixed ~1.5 m | Hand tooltips, interaction hints |
| Body-locked | Optional — deferred | User body | Body-forward | Fixed | Persistent status, inventory |
| Head-locked | Optional — deferred | Headset | Headset | Fixed | Urgent notifications only |
| Cylindrical | Optional — deferred | User body center | Cylinder surface normal | Fixed radius | VR dashboards, radial menus |

### SDK implication

`XrdsTextAnchor` enum on `XrdsText` spawn params. `World` and `Billboard` have runtime behavior; XR modes are deferred.

```rust
pub enum XrdsTextAnchor {
    World,                              // ✅ default — active
    Billboard,                          // ✅ active — XrdsBillboard marker + billboard_system
    ComfortPinned { depth_m: f32 },     // optional — needs headset pose
    BodyLocked,                         // optional — needs headset pose
    HeadLocked,                         // optional — needs headset pose
    Cylindrical { radius_m: f32 },      // optional — needs custom mesh layout
}
```

---

## Decision Summary

| Scenario | Primitive | Pipeline | Camera2d required | Status |
| --- | --- | --- | --- | --- |
| Spatial world text | `XrdsText` (mesh-based) | 3D | No | ✅ Done |
| Extruded / volumetric text | `XrdsExtrudedText` (solid mesh) | 3D PBR | No | ✅ Done |
| Surface / video overlay | Canvas entity + text payload | 2D / RTT | Yes (or RTT) | — |
| HUD / menus | `bevy::ui` | UI | No (own camera) | — |

**Rule of thumb:** If the text exists as an object *in the world*, use `XrdsText` (flat mesh) or `XrdsExtrudedText` (solid, PBR). If it exists *on a surface*, it is a property of that surface entity. If it exists *on screen*, it is UI. For XR, additionally choose an anchor mode that matches the user-relationship of the content.

---

## Implementation Plan

### Priority 1 — 3D Mesh Text (base primitive) ✅ Complete

- [x] Add `bevy_rich_text3d = "0.5"` to `xrds-runtime`
- [x] Replace `Text2d` spawn in `XrdsText` with `bevy_rich_text3d` mesh text (`Text3d` + `Text3dStyling`)
- [x] Expose font, size, color, alignment in `XrdsText` spawn params
- [x] Verify text is visible in an exported app with no `Camera2d`
- [x] Example: `examples/3d_text.rs` — live color pulse and per-second counter update

### Priority 2 — Billboard Anchor ✅ Complete

- [x] Add `XrdsTextAnchor` enum to `XrdsText` spawn params (`World` default, `Billboard` variant)
- [x] `XrdsBillboard` marker component — inserted during spawn when `anchor = Billboard` (`billboard.rs`)
- [x] `billboard_system` — rotates `+Z` toward active `Camera3d` each frame, preserves world-up (`Vec3::Y`), accounts for parent rotation via inverse parent quaternion
- [x] Registered in `install.rs` PostUpdate, after `XrdsUpdateSystemSet`, before `VisibilityPropagate`
- [x] `XrdsSceneTextAnchor` added to scene graph (`payload.rs`) with `#[serde(default)]` — backward-compatible; `to_runtime_node` / `from_xrds_text` round-trip preserves anchor
- [x] Editor palette **Billboard** primitive — spawns `XrdsText` with `anchor: Billboard`; Y=2.0 when parented (nameplate height), Y=1.5 standalone
- [x] Child-node spawn offset bug fixed — geometry-count X offset suppressed for child nodes so position is truly parent-local
- [x] Inspector shows "local to parent" label on Transform section for child nodes

### Priority 3 — Extruded Text ✅ Complete (added scope)

Implemented via `bevy_fontmesh = "0.1.1"`. Not in original plan — added as a distinct primitive type.

- [x] `XrdsExtrudedText` type with `text`, `font_size`, `depth`, `color`, `alignment` fields
- [x] `spawn_extruded_text_descriptor` using `TextMesh` + `TextMeshStyle`
- [x] `examples/extruded_text.rs` — three entities with different depths, Y-rotation animation
- [x] Editor integration: palette entry, inspector section, hierarchy icon `E³`
- [x] Race condition fix: `run_xrds_app_update` moved to `PostUpdate` (see below)
- [x] Color-only inspector edits update `StandardMaterial` in-place (no reimport)

### Architecture — Deferred-Command Race Condition ✅ Fixed

Recurring crash pattern: regular `Update` systems queued deferred commands for XRDS entities; `reimport_scene_in_world` (exclusive system) despawned those entities; commands then applied to dead entities → panic.

- [x] `run_xrds_app_update` moved from `Update` to `PostUpdate` (`runtime.rs` `OnUpdate::add_systems`)
- [x] `XrdsUpdateSystemSet` added — public `SystemSet` label for ordering by downstream crates
- [x] PostUpdate `ensure_visibility_hierarchy_components_system` ordered `.after(XrdsUpdateSystemSet)` so entities spawned by reimport are covered before `VisibilityPropagate`
- [x] Bevy's flush guarantee: all `Update` deferred commands are applied before `PostUpdate` starts — no per-case workarounds needed for future external crates

### Deferred — Optional Anchor Modes

Reserve variants in `XrdsTextAnchor` now. Implement systems when XR headset pose is wired through `xrds-openxr`.

- `ComfortPinned` — clamp depth to ergonomic range using headset `GlobalTransform`
- `BodyLocked` — derive body orientation from headset yaw only (strip pitch/roll)
- `HeadLocked` — parent entity to headset camera entity each frame
- `Cylindrical` — project glyph positions onto cylinder surface at spawn/update time
