# HUD System — Design Plan

## Problem Statement

A head-locked HUD must be **independent per player** (and per camera/anchor), just as
each player has their own camera. A HUD is authored once as a reusable template and
then linked to one or more PlayerAnchors. At runtime each active anchor instantiates
its own copy — when the anchor despawns, its HUD goes with it.

Simply placing HudCanvas/HudItem as scene nodes in the hierarchy fails this: those nodes
would be shared singletons with no per-player ownership.

## Analogy to Other Engines

| Engine             | Template unit                    | Instantiation                           | Ownership           |
| ------------------ | -------------------------------- | --------------------------------------- | ------------------- |
| Unreal (UMG)       | Widget Blueprint class           | `CreateWidget(PlayerController, Class)` | PlayerController    |
| Unity (UI Toolkit) | UXML asset / Prefab              | `UIDocument` component per camera       | Camera GameObject   |
| Godot              | `.tscn` scene file               | `instantiate()` per player              | Player node         |
| **XRDS**           | `XrdsHudTemplate` in hud_library | runtime clone per active PlayerAnchor   | PlayerAnchor entity |

---

## Data Model

```text
XrdsSceneDocument
├── nodes:       Vec<XrdsSceneNode>      ← scene hierarchy (unchanged)
├── assets:      Vec<XrdsSceneAsset>     ← GLTF / audio / textures (unchanged)
└── hud_library: Vec<XrdsHudTemplate>   ← NEW: authored HUD templates

XrdsHudTemplate
├── id:     HudTemplateId   (u64, doc-unique)
├── name:   String
├── depth:  f32             camera-space depth in metres (default 0.5)
└── items:  Vec<XrdsHudItemDef>
        ├── id:        HudItemDefId   (u64, template-local)
        ├── name:      String         key for runtime slot updates
        ├── position:  [f32; 2]      canvas-local X right, Y up (metres)
        ├── text:      String         authored default
        ├── font_size: f32
        └── color:     [f32; 4]      RGBA
```

`XrdsScenePlayerAnchor` gains one field:

```rust
pub hud_template_id: Option<HudTemplateId>
```

`HudCanvas` and `HudItem` are **removed from the scene node palette entirely**.
They exist only inside `hud_library` templates.

---

## API Surface

### Authoring (`XrdsAPI`)

```rust
// Link a template to an anchor at spawn time
api.link_hud(anchor_handle, template_id);
```

### Per-frame updates (`XrdsUpdateContext`)

```rust
// Update a named item on the HUD owned by a specific anchor
ctx.set_hud_item(anchor_handle, "hp",     "HP: 100",    None);
ctx.set_hud_item(anchor_handle, "status", "Reloading…", Some([1.0, 0.5, 0.0, 1.0]));
```

`set_hud_item` signature:

```rust
fn set_hud_item(
    anchor: &Handle<XrdsPlayerAnchor>,
    item:   &str,
    text:   &str,
    color:  Option<[f32; 4]>,
)
```

---

## Runtime Lifecycle

```text
PlayerAnchor activated
  └─ find hud_template_id on anchor component
       └─ clone template items into live entities
            ├── root entity: XrdsHeadLocked { local_offset: (0, 0, -depth) }
            │                parented to anchor entity
            └── N child entities: one Text3d per item, at item.position
                 └── tagged XrdsStoredHudInstance { items: Vec<(name, entity)> }

PlayerAnchor deactivated / despawned
  └─ despawn_recursive on HUD root  →  all item entities gone automatically
```

---

## Implementation Phases

### Phase 1 — Data layer ✅

#### `xrds-scene-graph`

- [x] Add `HudTemplateId(u64)` newtype
- [x] Add `HudItemDefId(u64)` newtype
- [x] Add `XrdsHudItemDef { id, name, position, text, font_size, color }`
- [x] Add `XrdsHudTemplate { id, name, depth, items: Vec<XrdsHudItemDef> }`
- [x] Add `hud_library: Vec<XrdsHudTemplate>` field to `XrdsSceneDocument`
- [x] Add `hud_template_id: Option<HudTemplateId>` to `XrdsScenePlayerAnchor`
- [x] Derive `Serialize`/`Deserialize` — backward-compatible (field defaults to `None`/`[]`)
- [x] Remove `XrdsSceneHudCanvas`, `XrdsSceneHudItem`, `HudCanvas`/`HudItem` payload variants

#### `xrds-components`

- [x] Add `XrdsHudItemDef` (mirrors scene-graph struct, used at runtime)
- [x] Add `XrdsHudTemplate { id, name, depth, items }` as a non-ECS value type
- [x] Remove `XrdsHudCanvas`, `XrdsHudItem` components

---

### Phase 2 — Runtime ✅

- [x] `XrdsStoredHudInstance` component: `items: Vec<(String, Entity)>` — name → child entity
- [x] `spawn_hud_instance_for_anchor(world, anchor_entity, template)` — spawns 3D text entities under anchor with `XrdsHeadLocked`
- [x] HUD spawned at scene import in `tag_player_anchor_entities` (no per-frame system needed; `head_locked_system` handles active-anchor filtering already)
- [x] `api.link_hud(anchor_id, Option<template>)` — despawns old HUD, spawns new one from template
- [x] `ctx.set_hud_item(anchor_id, name, text, color)` — patches `Text3d` on named child entity via `XrdsStoredHudInstance`
- [x] Remove `spawn_hud_canvas_descriptor`, `spawn_hud_item_descriptor`
- [x] Remove `HudCanvas`/`HudItem` arms from reimport and registry

---

### Phase 3 — Editor bridge (`src-tauri`) ✅

#### New `EditorCommand` variants (hud_library operations)

```rust
CreateHudTemplate   { name: String }
DeleteHudTemplate   { id: u64 }
RenameHudTemplate   { id: u64, name: String }
SetHudTemplateDepth { id: u64, depth: f32 }
AddHudItem          { template_id: u64 }
RemoveHudItem       { template_id: u64, item_id: u64 }
RenameHudItem       { template_id: u64, item_id: u64, name: String }
SetHudItemPosition  { template_id: u64, item_id: u64, position: [f32; 2] }
SetHudItemText      { template_id: u64, item_id: u64, text: String }
SetHudItemFontSize  { template_id: u64, item_id: u64, font_size: f32 }
SetHudItemColor     { template_id: u64, item_id: u64, color: [f32; 4] }
LinkHudTemplate     { anchor_id: u64, template_id: Option<u64> }
```

#### Snapshot additions

- [x] `EditorSnapshot` gains `hud_library: Vec<HudTemplateDto>`
- [x] `HudTemplateDto { id, name, depth, items: Vec<HudItemDefDto> }`
- [x] `NodePayloadDto::PlayerAnchor` gains `hud_template_id: Option<u64>`
- [x] Remove `NodePayloadDto::HudCanvas`, `NodePayloadDto::HudItem`

#### Command handlers (`inspector.rs` / new `hud_library.rs`)

- [x] All `CreateHudTemplate` / `Delete` / `Rename` / `Depth` → edit `doc.hud_library`
- [x] All item mutations → find template by id, mutate item in-place
- [x] `LinkHudTemplate` → edit `PlayerAnchor.hud_template_id` in doc, trigger reimport
- [x] Remove old `SetHudCanvas*` / `SetHudItem*` handlers

---

### Phase 4 — React editor UI ✅

#### HUD Library panel (new, separate from hierarchy)

```text
┌─ HUD Library ─────────────────────────────────────┐
│  + New Template                                    │
│  ┌──────────────────────────────────────────────┐  │
│  │ ☰  Cockpit HUD          [Edit]  [✕]          │  │
│  │ ☰  Minimap Overlay      [Edit]  [✕]          │  │
│  └──────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────┘
```

- [x] New `HudLibraryPanel.tsx` component, shown in left sidebar below PlayerPanel
- [x] List of templates with create / rename (double-click) / delete
- [x] "Edit ↗" opens the HUD Canvas overlay for that template
- [x] Template list sourced from `snapshot.hud_library`

#### PlayerAnchor inspector additions

- [x] Dropdown: "HUD Template" — lists all templates by name, None option
- [x] Sends `LinkHudTemplate { anchor_id, template_id }` on change

#### HUD Canvas overlay (repurposed from scene-node editing to template editing)

- [x] `HudCanvasOverlay` receives `templateId` instead of `panelId`
- [x] Reads template from `snapshot.hud_library.find(t => t.id === templateId)`
- [x] All drag / edit commands use `template_id` + `item_id` (not scene node ids)
- [x] "Add Item" sends `AddHudItem { template_id }`
- [x] "Remove item" sends `RemoveHudItem { template_id, item_id }`
- [x] Depth slider sends `SetHudTemplateDepth { id: template_id, depth }`

#### Palette / hierarchy cleanup

- [x] Remove `HudCanvas` and `HudItem` from `Palette.tsx` text group
- [x] Remove `HudCanvas` and `HudItem` from `KIND_ICON` map

---

## Out of Scope (for now)

- Per-item visibility toggle
- Item drag-to-reorder within template
- Background quad / panel border
- Per-item alignment override
- Animated item transitions
- Canvas grid snapping
- Multiple HUD templates active simultaneously on one anchor
- HUD template export as standalone asset file

---

## File Checklist

```text
xrds-scene-graph
  src/scene/payload.rs          remove HudCanvas/HudItem variants
  src/scene/node.rs             remove HudCanvas/HudItem from XrdsSceneRuntimeComponent
  src/document/core.rs          add hud_library field + HudTemplateId / XrdsHudTemplate types
  src/scene/player_anchor.rs    add hud_template_id field

xrds-components
  src/primitives/hud_panel.rs   remove XrdsHudCanvas, XrdsHudItem; add XrdsHudTemplate value type
  src/primitives/mod.rs         update pub use

xrds-runtime
  src/xrds_api/spawn.rs         add spawn_hud_instance; remove canvas/item descriptors
  src/xrds_api/api.rs           add link_hud; remove HudCanvas/HudItem arms
  src/xrds_api/context.rs       add set_hud_item
  src/xrds_api/state.rs         add XrdsStoredHudInstance; remove XrdsStoredHudPanel
  src/xrds_api/registry.rs      remove HudCanvas/HudItem registrations
  src/xrds_api/reimport.rs      remove HudCanvas/HudItem arms
  src/xrds_api.rs               update pub use

apps/xrds-editor/src-tauri
  src/bridge.rs                 new HudTemplateDto, new EditorCommands, remove old HUD DTOs
  src/hud_library.rs            NEW — command handlers for hud_library mutations
  src/inspector.rs              PlayerAnchor DTO gains hud_template_id; remove HudCanvas/Item handlers
  src/bevy_bridge.rs            is_structural_command updated; add hud_library to snapshot
  src/hierarchy.rs              remove HudCanvas/HudItem from payload_kind_str
  src/palette.rs                remove HudCanvas/HudItem spawn arms

apps/xrds-editor/src
  src/types/bridge.ts           HudTemplateDto, new commands, PlayerAnchor hud_template_id
  src/components/HudLibraryPanel.tsx   NEW — template list panel
  src/components/HudCanvasOverlay.tsx  repurpose: template_id based, not panelId
  src/components/Inspector.tsx  PlayerAnchor section gains HUD template dropdown
  src/components/Palette.tsx    remove HudCanvas/HudItem
  src/styles/editor.css         hud-library-panel styles
```
