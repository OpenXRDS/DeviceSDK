# World-Space UI — Implementation Plan

Last updated: 2026-07-13 (Phases 1–7 complete, including canvas-window editor, widget
resize handles, image asset picker, and desktop play-mode mouse interaction)

## What is World-Space UI?

**World-space UI** (also called *diegetic UI* in XR design literature) refers to interactive UI
panels that exist as physical objects inside the 3D scene — anchored at a world transform, rendered
as meshes, and interacted with by pointing an XR controller ray at them.

This is distinct from the **HUD** system, which is a screen-space overlay (or a camera-following
text mesh) that the user always sees regardless of where they look. The key difference:

| | HUD | World-Space UI |
| --- | --- | --- |
| Position | Fixed to screen / follows camera | Anchored at a world position |
| Content | Text only | Buttons, sliders, toggles, images, labels |
| Interaction | None — read only | Ray-pointer press, drag |
| Moves with player | Yes | No — player walks up to it |
| Rendered as | Bevy 2D UI node or camera-tracked mesh | 3D mesh quad in the scene |
| Use case | Score, health, hints, crosshair | Menus, control panels, settings, inventory |

The term **world-space UI** is the standard in Unity ("World Space Canvas"), Unreal ("3D Widget /
Widget Component"), and Meta's XR design guidelines. **Diegetic UI** is the equivalent UX/design
term (meaning the UI exists *inside* the world of the experience, not layered on top of it).

---

## Goal

Implement `XrdsWorldPanel` and its widget family — interactive UI surfaces that live at fixed world
positions and respond to XR controller ray-pointer input.

---

## What Already Exists (Foundation)

- `bevy_rich_text3d` — mesh-based flat text (used by `XrdsText`)
- `bevy_mod_outline` — hover/selection highlight on entities
- `raycast_world()` — ray vs. AABB hit detection, sorted nearest-first
- `XrGrabEvent` / `XrDropEvent` — grab press/release events with XRDS ids
- Hover detection loop in `grab.rs` — per-frame outline on nearest grabbable entity
- Text anchor modes — Billboard, HeadLocked, ComfortPinned, Cylindrical

None of the widget/panel layer exists yet.

---

## Architecture Decisions

- **Mesh-based panels**: flat quad geometry, unlit material, optional rounded corners via UV shader.
  No render-to-texture; all children are world-space mesh entities parented to the panel root.
- **Pointer ray**: separate from the grab ray — world-UI ray always fires, grab ray only fires when
  trigger held. XR controller aim pose → ray → hit nearest `XrdsWorldSurface` tagged entity.
- **Hit locality**: convert world hit point to panel-local UV (0..1, 0..1). All widget hit tests
  use local UV, not world AABB, so panels work at any transform.
- **State machine per widget**: `Idle → Hovered → Pressed → Released` driven by pointer events.
- **Non-expert API**: `api.spawn_world_panel()` / `api.spawn_world_button()` / etc. return typed
  handles. Expert path: insert raw components directly.

---

## Phase 1 — Pointer and Panel Foundation

- [x] **`XrdsWorldPointer` component** (`xrds-components/src/world_ui/pointer.rs`)
  - Marks an entity as a world-UI ray source (one per XR controller hand)
  - Fields: `hand: XrGrabHand`, `max_distance: f32`, `active: bool`

- [x] **`XrdsWorldSurface` component** (`xrds-components/src/world_ui/surface.rs`)
  - Tags a panel root entity as a raycast target for world-UI pointer
  - Fields: `size: Vec2` (metres, local XY plane), `enabled: bool`
  - Used by pointer system to convert world hit → panel-local UV

- [x] **Pointer system** (`xrds-runtime/src/xrds_api/world_ui_pointer.rs`)
  - Runs per-frame; casts ray from each `XrdsWorldPointer` entity
  - Finds nearest `XrdsWorldSurface` hit within `max_distance`
  - Computes panel-local hit UV from world intersection point
  - Writes result into `XrdsWorldPointerState` resource (one entry per hand)
  - Fires `XrWorldHoverEnterEvent { panel_id, hand, uv }` / `XrWorldHoverExitEvent` on transitions

- [x] **Pointer cursor visual**
  - Small sphere or disc mesh spawned as child of controller entity
  - Snaps to ray hit point on `XrdsWorldSurface`; hidden when no panel hit

- [x] **`XrdsWorldPanel` primitive** (`xrds-components/src/world_ui/panel.rs`)
  - Fields: `size: Vec2`, `color: Color`, `corner_radius: f32`, `opacity: f32`
  - Spawns a flat quad mesh with unlit material
  - Parent entity for all child widgets

- [x] **`api.spawn_world_panel(params)`** in `XrdsAPI`
  - Returns `XrdsHandle<XrdsWorldPanel>`
  - Inserts `XrdsWorldSurface` automatically

---

## Phase 2 — Core Widgets

### Label

- [x] **`XrdsWorldLabel` component** (`xrds-components/src/world_ui/label.rs`)
  - Fields: `local_position: [f32; 2]`
  - Entity also carries `Text3d` + `Text3dStyling` from `bevy_rich_text3d`; spawned as child of panel

- [x] **`api.spawn_world_label(panel_handle, params)`** in `XrdsAPI`
  - Returns `Handle<XrdsWorldLabel>`; update text via `ctx.set_world_label_text(handle, "...")`

### Button

- [x] **`XrdsWorldButton` component** (`xrds-components/src/world_ui/button.rs`)
  - Fields: `local_position`, `size`, `normal_color`, `hover_color`, `pressed_color`
  - State: `XrdsWorldButtonState { Idle, Hovered, Pressed }`

- [x] **Button hit system** (`xrds-runtime/src/xrds_api/world_ui_button.rs`)
  - Reads `XrdsWorldPointerState` + `XrInput` trigger state each frame
  - Converts panel UV → panel-local metres; checks button AABB
  - State machine: `Idle → Hovered → Pressed → Released`
  - Swaps `MeshMaterial3d` from pre-built `XrdsWorldButtonMaterials` cache
  - Fires `XrWorldButtonPressEvent { button_entity }` / `XrWorldButtonReleaseEvent { button_entity }`

- [x] **`api.spawn_world_button(panel_handle, params)`** in `XrdsAPI`
  - Returns `Handle<XrdsWorldButton>`; spawns background quad + text label child
  - Events consumed via `ctx.world_button_presses()` / `ctx.world_button_releases()`

### Image

- [x] **`XrdsWorldImage` component** (`xrds-components/src/world_ui/image.rs`)
  - Fields: `local_position`, `size`; spawned as textured quad child of panel
  - Material: `base_color_texture` + `tint` via `StandardMaterial`

- [x] **`api.spawn_world_image(panel_handle, params)`** in `XrdsAPI`

---

## Phase 3 — Compound Widgets

### Slider

- [x] **`XrdsWorldSlider` component** (`xrds-components/src/world_ui/slider.rs`)
  - Fields: `min`, `max`, `value`, `size`, `local_position`, `track_color`, `thumb_color`, `thumb_size`
  - Visual: track quad + thumb quad (repositioned by `world_ui_slider_system` each frame)
  - `dragging_hand: Option<XrGrabHand>` tracks active drag state internally

- [x] **Slider drag system** (`xrds-runtime/src/xrds_api/world_ui_slider.rs`)
  - Trigger press within (±size[0]/2, ±max(size[1], 3cm)/2) bounds starts drag
  - While trigger held: maps panel UV.x → value in `[min, max]`; repositions thumb
  - Fires `XrWorldSliderChangeEvent { slider_entity, value, hand }` per-frame while dragging

- [x] **`api.spawn_world_slider(panel_handle, params)`**; **`ctx.set_world_slider_value(handle, v)`**

### Toggle

- [x] **`XrdsWorldToggle` component** (`xrds-components/src/world_ui/toggle.rs`)
  - Fields: `checked`, `size`, `local_position`, `track_off_color`, `track_on_color`, `thumb_color`
  - Visual: track quad (colour updates in-place via `Assets<StandardMaterial>`) + thumb quad

- [x] **Toggle system** (`xrds-runtime/src/xrds_api/world_ui_toggle.rs`)
  - Trigger press flips `checked`; swaps track colour; repositions thumb; fires `XrWorldToggleEvent`

- [x] **`api.spawn_world_toggle(panel_handle, params)`**; **`ctx.set_world_toggle(handle, bool)`**

---

## Phase 4 — Layout

- [x] **`XrdsWorldLayout` component** on panel (`xrds-components/src/world_ui/layout.rs`)
  - Variants: `None` (manual positioning), `VStack { gap }`, `HStack { gap }`, `Grid { cols, gap }`
  - `XrdsWorldLabel` gains `layout_size: [f32; 2]` (slot hint for layout; default 20 cm × 6 cm)
  - Convenience constructors: `XrdsWorldLayout::vstack(gap)`, `.hstack(gap)`, `.grid(cols, gap)`

- [x] **Layout system** (`xrds-runtime/src/xrds_api/world_ui_layout.rs`)
  - Runs every frame, after pointer system, before button/slider/toggle interaction systems
  - Collects direct widget children of each layout panel; computes positions; writes
    `local_position` (for hit-test) and `Transform` (for rendering)
  - Panels without an `XrdsWorldLayout` component are untouched — zero overhead

---

## Phase 5 — Scene Graph Integration

- [x] **`XrdsSceneWorldPanel` payload** (`xrds-scene-graph/src/scene/payload.rs`)
  - Serializable panel definition: size, color, corner_radius, opacity, layout, widgets
  - Optional fields (`corner_radius`, `opacity`, `layout`, `widgets`) skipped when default

- [x] **`XrdsSceneWorldWidget` enum**
  - Variants: `Label(...)`, `Button(...)`, `Image(...)`, `Slider(...)`, `Toggle(...)`
  - All fields serializable via `serde`; companion `XrdsSceneWorldLayout` mirrors runtime layout

- [x] **`XrdsSceneRuntimeComponent::WorldPanel`** in `xrds-scene-graph/src/scene/node.rs`
  - Carries `(XrdsWorldPanel, Vec<XrdsSceneWorldWidget>, XrdsSceneWorldLayout)` for import
  - `to_runtime_node()` converts `XrdsSceneNodePayload::WorldPanel → XrdsSceneRuntimeComponent::WorldPanel`

- [x] **Runtime import** — `import_runtime_nodes` (api.rs) and `spawn_runtime_component` (reimport.rs)
  - Spawns panel via `spawn_with_id`, then each widget, then applies layout component

- [x] **Round-trip test** — `crates/xrds-scene-graph/src/tests/world_ui.rs`
  - `world_panel_round_trip`: full document → JSON → deserialise → assert_eq
  - `world_panel_minimal_json_compact`: verifies optional fields absent when default
  - `world_panel_to_runtime_node`: verifies `to_runtime_node()` produces correct component/widget/layout

- [x] **Editor inspector** — panel properties in the inspector; widgets edited in the
  canvas editor window (see Phase 7)

---

## Phase 6 — Editor Integration

- [x] **Palette entry** (`palette.rs`)
  - `"WorldPanel"` arm added to `build_primitive_node`; default transform eye-height, 1 m in front
  - `XrdsSceneWorldPanel::default()` spawned; `XrdsSceneWorldPanel` added to imports

- [x] **Inspector DTO + command** (`bridge.rs`, `inspector.rs`)
  - `NodePayloadDto::WorldPanel { size, color, corner_radius, opacity }` added to enum
  - `build_payload_dto` arm maps `XrdsSceneNodePayload::WorldPanel → NodePayloadDto::WorldPanel`
  - `EditorCommand::SetWorldPanelParams` added; handled in `apply_inspector_command` (full reimport)
  - `is_structural_command` in `bevy_bridge.rs` updated to include `SetWorldPanelParams`

- [x] **Hierarchy kind string** — already present (`payload_kind_name` / `payload_kind` return `"WorldPanel"`); no further change needed. Widgets are inline in the payload (not document nodes) so they do not appear as hierarchy children.

- [x] **Panel gizmo in viewport** (`viewport_gizmo.rs`, `bevy_scene.rs`)
  - `world_panel_gizmo_system`: blue wireframe cuboid (depth ≈ 0) for every panel; bright cyan when selected
  - Registered in `bevy_scene.rs` PostUpdate alongside `spawn_zone_gizmo_system`

---

## Phase 7 — Editor Widget Authoring

Widgets live inline in the panel payload (`XrdsSceneWorldPanel::widgets`), not as document
nodes — so they are edited through the WorldPanel inspector, not the hierarchy panel.
Every widget mutation is structural (full reimport), same as `SetWorldPanelParams`.

### Editor UX decisions

- **Canvas editor window** (primary editing surface, mirrors the HUD template editor):
  the WorldPanel inspector shows panel-level sliders plus an **Edit Widgets ↗** button that
  opens a window-style WYSIWYG editor (`WorldPanelCanvasOverlay.tsx`) filling the editor
  almost fully. Title bar has **✓ Save & Close** (keep changes) and **✕ Cancel** (Esc) —
  Cancel restores the panel state captured at open via `SetWorldPanelParams` +
  `SetWorldPanelLayout` + `SetWorldPanelWidgets`. Edits apply live to the document so the
  3-D viewport previews them while the window is open. The canvas renders the panel at its
  real aspect with the background colour, and draws every widget approximately as it
  appears in 3-D (button plate + label, slider track/fill/thumb, toggle pill, label text
  scaled by `font_size × px-per-metre`).
- **Drag to position / resize**: widgets drag with the pointer (when layout = None) and
  commit `local_position` on release. The selected widget shows a SE corner handle that
  resizes about the widget centre (`size` for button/image/slider/toggle; labels scale
  `font_size` with the drag height and update `layout_size`). Click selects; a bottom
  editor bar exposes per-kind fields (text, font size, colours, sizes, min/max/value,
  checked, list order) with live-local edit + commit-on-blur so full reimports don't fire
  per keystroke.
- **Image asset picker**: the Image widget's editor bar has a **Browse…** button that opens
  a native file dialog (`pick_texture` kind in `wry_overlay.rs`) and commits the chosen
  path into `asset_path`.
- **Layout preview**: the overlay re-implements the runtime layout maths
  (`world_ui_layout.rs` vstack/hstack/grid) in TS so auto-layout positions preview exactly;
  dragging is disabled while a layout is active.
- **Add widget**: header buttons (Label / Button / Image / Slider / Toggle) append a widget
  with its `Default` values.
- **Panel props in overlay**: bottom bar edits width/height/background/alpha and layout
  mode/gap/cols without leaving the editor.
- **Scale hidden for panels**: panel dimensions come from `size` only; the transform Scale row
  is hidden in the inspector for WorldPanel nodes to avoid double-applying.
- **Opacity slider removed**: `opacity` is a runtime master-fade field (kept in the document
  format and passed through unchanged); background translucency is styled via color alpha.

### Bridge layer (`bridge.rs`, `bridge.ts`)

- [x] **`WorldWidgetDto`** — serde-tagged enum mirroring `XrdsSceneWorldWidget`
  (`Label`, `Button`, `Image`, `Slider`, `Toggle`) with all authored fields
- [x] **`WorldLayoutDto`** — tagged enum: `None` | `VStack { gap }` | `HStack { gap }` |
  `Grid { cols, gap: [f32; 2] }`
- [x] **Extend `NodePayloadDto::WorldPanel`** with `layout: WorldLayoutDto` and
  `widgets: Vec<WorldWidgetDto>`
- [x] **New `EditorCommand` variants** (all structural → full reimport):
  - `AddWorldPanelWidget { id, kind }` — append default widget of `kind`
  - `RemoveWorldPanelWidget { id, index }`
  - `MoveWorldPanelWidget { id, index, delta }` — reorder by ±1
  - `SetWorldPanelWidget { id, index, widget }` — full replace of one widget
  - `SetWorldPanelWidgets { id, widgets }` — replace the whole list (Cancel/revert)
  - `SetWorldPanelLayout { id, layout }`

### Command handlers (`inspector.rs`, `bevy_bridge.rs`)

- [x] DTO conversion both ways (`world_widget_dto` / `world_widget_from_dto`,
  `world_layout_dto` / `world_layout_from_dto`) + `default_widget_for_kind`
- [x] Handlers in `apply_inspector_command`, guarding `index` bounds; return `true`
- [x] Add the five commands to `is_structural_command`

### Frontend (`Inspector.tsx`, `bridge.ts`)

- [x] TS types for `WorldWidget` / `WorldLayout` + the five commands
- [x] `WorldPanelCanvasOverlay.tsx` — full-screen WYSIWYG editor (drag widgets, click to
  select, per-kind editor bar, add-widget buttons, panel props + layout controls)
- [x] `WorldPanelSection` in the inspector: panel sliders + widget count + "Edit Widgets ↗"
  button opening the overlay (wired through `App.tsx` like the HUD template editor)
- [x] Layout preview in the overlay mirrors runtime vstack/hstack/grid maths; dragging
  disabled when layout ≠ None

### Desktop play-mode interaction

On desktop there is no OpenXR runtime, so `XrInput` was never populated and the world-UI
systems bailed out — widgets were inert in editor play mode.

- [x] `XrInput` is now always initialised by the runtime (`install.rs`) and re-exported
  from `xrds-runtime`, so the pointer/button/slider/toggle systems run on desktop too
- [x] `apps/xrds-editor/src-tauri/src/play_pointer.rs` — `mouse_world_ui_input_system`
  (PreUpdate, after input processing) synthesises the **right-hand** XR pointer while
  play mode is active: pose = ray from the pawn camera through the mouse cursor,
  select/trigger = left mouse button. Hover, press, slider drag, and toggle flip all
  work with the mouse exactly as with a controller ray, including the hit-point cursor.

### Out of scope for Phase 7

- Widget drag-positioning in the 3-D viewport (gizmo-based) — the canvas editor covers
  positioning for now
- `examples/world_space_ui.rs` sample app — API surface is demonstrated in the doc below

---

## Events Summary

| Event | Fields | When |
| --- | --- | --- |
| `XrWorldHoverEnterEvent` | `panel_id: XrdsId, hand, uv: Vec2` | Pointer enters panel surface |
| `XrWorldHoverExitEvent` | `panel_id: XrdsId, hand` | Pointer leaves panel surface |
| `XrWorldButtonPressEvent` | `button_entity: Entity, hand` | Trigger pressed while button hovered |
| `XrWorldButtonReleaseEvent` | `button_entity: Entity, hand` | Trigger released after button press |
| `XrWorldSliderChangeEvent` | `slider_entity: Entity, value: f32, hand` | Slider dragged to new value |
| `XrWorldToggleEvent` | `toggle_entity: Entity, checked: bool, hand` | Toggle flipped |

> **Note**: Panel events use `XrdsId` (panels are registered XRDS nodes). Widget events use
> `Entity` (labels, buttons, sliders, toggles are widget children, not standalone scene nodes).
> Compare via `ev.button_entity == btn.entity()` etc.

---

## XrdsAPI Surface (Implemented — Phases 1–4)

```rust
fn setup(&mut self, api: &mut XrdsAPI) {
    // Panel — positioned in the world like any other XRDS node.
    let panel = api.spawn_world_panel(XrdsWorldPanel::default()
        .with_size(0.6, 0.4)
        .with_color(0.1, 0.1, 0.1, 0.9));
    api.set_transform(&panel, Transform::from_xyz(0.0, 1.5, -1.0));

    // Phase 2 widgets — attached to panel, positioned in panel-local space (metres).
    self.lbl = api.spawn_world_label(&panel, XrdsWorldLabelParams {
        text: "Value: 0.50".to_string(),
        font_size: 0.05,
        local_position: [0.0, 0.15],
        ..default()
    });
    self.btn = api.spawn_world_button(&panel, XrdsWorldButtonParams {
        label: "Reset".to_string(),
        size: [0.18, 0.06],
        local_position: [0.0, 0.05],
        ..default()
    });
    let _img = api.spawn_world_image(&panel, XrdsWorldImageParams {
        asset_path: "textures/icon.png".to_string(),
        size: [0.08, 0.08],
        local_position: [-0.22, 0.15],
        ..default()
    });

    // Phase 3 widgets.
    self.sld = api.spawn_world_slider(&panel, XrdsWorldSliderParams {
        min: 0.0, max: 1.0, value: 0.5,
        local_position: [0.0, -0.05],
        ..default()
    });
    self.tog = api.spawn_world_toggle(&panel, XrdsWorldToggleParams {
        checked: false,
        local_position: [0.15, -0.12],
        ..default()
    });

    // Phase 4 — Layout.
    // Auto-arrange all widgets on a second panel in a vertical stack.
    let panel2 = api.spawn_world_panel(XrdsWorldPanel::default()
        .with_size(0.3, 0.5)
        .with_color(0.08, 0.08, 0.12, 0.9));
    api.set_transform(&panel2, Transform::from_xyz(0.8, 1.5, -1.0));
    api.spawn_world_label(&panel2, XrdsWorldLabelParams {
        text: "Settings".to_string(), font_size: 0.05, ..default()
    });
    api.spawn_world_toggle(&panel2, XrdsWorldToggleParams { ..default() });
    api.spawn_world_slider(&panel2, XrdsWorldSliderParams { ..default() });
    // Apply layout AFTER spawning widgets so they're available as children.
    api.set_world_panel_layout(&panel2, XrdsWorldLayout::vstack(0.01));
}

fn update(&mut self, ctx: &mut XrdsUpdateContext) {
    // Button press resets slider and label.
    for ev in ctx.world_button_presses() {
        if ev.button_entity == self.btn.entity() {
            ctx.set_world_slider_value(&self.sld, 0.5);
            ctx.set_world_label_text(&self.lbl, "Value: 0.50");
        }
    }
    // Slider drag updates label in real time.
    for ev in ctx.world_slider_changes() {
        if ev.slider_entity == self.sld.entity() {
            ctx.set_world_label_text(&self.lbl, format!("Value: {:.2}", ev.value));
        }
    }
    // Toggle changes panel opacity (illustrative).
    for ev in ctx.world_toggle_events() {
        if ev.toggle_entity == self.tog.entity() {
            // ev.checked is the new state
        }
    }
}
```

---

## File Map

```text
crates/xrds-components/src/world_ui/
    mod.rs            — module doc + re-exports
    pointer.rs        — XrdsWorldPointerState, XrdsWorldPointerHit, XrdsWorldPointerCursors
                        XrWorldHoverEnterEvent, XrWorldHoverExitEvent
    surface.rs        — XrdsWorldSurface
    panel.rs          — XrdsWorldPanel, XrdsWorldPanelParams
    label.rs          — XrdsWorldLabel, XrdsWorldLabelParams
    button.rs         — XrdsWorldButton, XrdsWorldButtonState, XrdsWorldButtonParams
                        XrWorldButtonPressEvent, XrWorldButtonReleaseEvent
    image.rs          — XrdsWorldImage, XrdsWorldImageParams
    slider.rs         — XrdsWorldSlider, XrdsWorldSliderParams, XrWorldSliderChangeEvent
    toggle.rs         — XrdsWorldToggle, XrdsWorldToggleParams, XrWorldToggleEvent
    layout.rs         — XrdsWorldLayout (VStack / HStack / Grid)

crates/xrds-runtime/src/xrds_api/
    world_ui_pointer.rs   — pointer ray cast, cursor visuals, hover enter/exit events
    world_ui_button.rs    — button hit test, Idle/Hovered/Pressed state machine, material swap
    world_ui_slider.rs    — slider drag (trigger-held → UV.x → value), thumb reposition
    world_ui_toggle.rs    — toggle press (flip checked), track colour update, thumb reposition
    world_ui_layout.rs    — layout computation (VStack/HStack/Grid), Transform + local_position write

crates/xrds-scene-graph/src/scene/
    payload.rs            — XrdsSceneWorldPanel, XrdsSceneWorldWidget, XrdsSceneWorldLayout,
                            XrdsSceneWorldLabel/Button/Image/Slider/Toggle (all serde)
    node.rs               — XrdsSceneRuntimeComponent::WorldPanel; to_runtime_node conversion

crates/xrds-scene-graph/src/tests/
    world_ui.rs           — round-trip, compact JSON, and runtime-node conversion tests

apps/xrds-editor/src-tauri/src/
    play_pointer.rs       — desktop play-mode mouse → world-UI pointer bridge
    inspector.rs          — WorldPanel DTO build + widget/layout command handlers
    viewport_gizmo.rs     — world_panel_gizmo_system (wireframe in viewport)

apps/xrds-editor/src/components/
    WorldPanelCanvasOverlay.tsx — window-style WYSIWYG widget editor (drag/resize/edit,
                                  Save & Close / Cancel, layout preview, asset picker)

examples/
    world_space_ui.rs     — (not yet implemented; see XrdsAPI Surface section)
```

---

## Open Questions

1. **Rounded corners** — implement as UV-distance discard in fragment shader, or approximate with
   more quad geometry? Shader approach is cleaner but adds a custom material.
2. **Scroll views** — needed for lists? Defer until a concrete use case requires it.
3. **Text input** — virtual keyboard integration (especially on Quest) is a large scope; defer.
4. **Multi-panel depth sorting** — panels at different depths need pointer to pick nearest; current
   AABB raycast already returns sorted hits, so this should work naturally.
5. **Physical touch** — grab-system reach-and-touch vs. ray-pointer press; consider supporting both
   interaction modes on the same button for near/far interaction parity.
