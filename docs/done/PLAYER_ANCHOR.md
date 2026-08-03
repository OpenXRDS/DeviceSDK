# PlayerAnchor Design Plan

## Node Hierarchy

Three distinct concepts, each with a clear single responsibility:

```
PlayerSpawn                    ← spawn point only; no children, no HUD concept
Player                         ← world-space pawn entity; moves around the scene
└── PlayerAnchor ("Cockpit")   ← viewpoint within the player; owns HUD + mesh children
    ├── Text [HeadLocked]
    ├── Text [BodyLocked]
    └── GltfAsset (body mesh)
```

| Node | Answers | Drives |
|------|---------|--------|
| `PlayerSpawn` | Where does play start? | Pawn initial position |
| `Player` | What moves around the world? | World-space pawn transform |
| `PlayerAnchor` | What viewpoint does the player see through? | HUD reference frame |

### Relationships

- **`PlayerSpawn`** is independent. The runtime places the pawn there at play-start.
  No children. No knowledge of `Player` or `PlayerAnchor`.
- **`Player`** is the world-space root for a playable entity. Its transform is driven by
  the pawn's locomotion system. It owns one or more `PlayerAnchor` children.
- **`PlayerAnchor`** is a local-space viewpoint within a `Player` (or can sit at root
  for Phase 1 standalone use). Text/mesh children authored under it use its local
  coordinate space for offsets.

### Backward compatibility

- A scene with only `PlayerSpawn` works exactly as before.
- A standalone `PlayerAnchor` at root level (Phase 1) keeps working — it still uses the
  camera as reference and authored offsets for HUD children.
- A `Player → PlayerAnchor` hierarchy is the Phase 2 target model.

---

## Problem (original)

All anchor modes (HeadLocked, BodyLocked, ComfortPinned, Cylindrical) are camera-centric at
runtime but authored in world-space in the editor. This creates a permanent mismatch:

- The authored `transform.translation` of a `Text` node is a **world-space scene position**.
- At runtime the anchor system **ignores** that position and uses hardcoded camera-relative
  defaults instead.
- As a result, the edited position is meaningless.

---

## Core Abstraction: `PlayerAnchor`

A **`PlayerAnchor`** marks a node as a *playable entity perspective*. It is the authoritative
reference frame for all anchor-based children.

Key rules:

1. A `Text` node that is a **child of a `PlayerAnchor`** treats its authored
   `transform.translation` as an **anchor-local offset**, not a world position.

2. A `Text` node at **root level** (no `PlayerAnchor` parent) keeps the current fallback
   behaviour (hardcoded defaults, active camera as reference).

3. Only the **active** `PlayerAnchor` (the one the player currently inhabits) runs its
   anchor systems. Switching between anchors is an API call.

4. `PlayerAnchor`'s own world-space transform is its **rest/spawn pose** in the world.

---

## Data Model

### `PlayerSpawn`
```rust
pub struct XrdsScenePlayerSpawn {
    pub locomotion_mode: XrdsPlayerLocomotionMode,
    pub fov_deg: f32,
}
```
Unchanged. Simple spawn-point marker. No parent/child semantics.

### `Player` (Phase 2 — new)
```rust
pub struct XrdsScenePlayer {
    pub label: String,
    pub locomotion_mode: XrdsPlayerLocomotionMode,
    pub fov_deg: f32,
}
```
World-space pawn entity. Parent of one or more `PlayerAnchor` nodes.
Its transform is driven at runtime by the locomotion system.

### `PlayerAnchor`
```rust
pub struct XrdsScenePlayerAnchor {
    pub label: String,
    pub locomotion_mode: XrdsPlayerLocomotionMode,  // used when standalone (no Player parent)
    pub fov_deg: f32,                                // used when standalone
    pub is_initial: bool,
}
```
Viewpoint within a `Player`, or standalone root-level anchor.
`locomotion_mode` and `fov_deg` are used when there is no parent `Player`; when a
`Player` parent exists those fields on the parent take precedence.

---

## Implementation Phases

### Phase 1 — Core wiring ✅ COMPLETE

- [x] `XrdsScenePlayerAnchor` struct and `PlayerAnchor(...)` variant in `XrdsSceneNodePayload`.
- [x] `XrdsPlayerAnchorRoot` marker component in `xrds-runtime`.
- [x] Anchor systems (`head_locked`, `body_locked`, `comfort_pinned`) check parent for
      `XrdsPlayerAnchorRoot` and use authored `local_offset` when present.
- [x] Scene importer tags `PlayerAnchor` entities with `XrdsPlayerAnchorRoot` (full reimport
      and single-node incremental add).
- [x] Palette + icon for `PlayerAnchor` in the editor UI.

**Phase 1 behavior summary:**
- `PlayerAnchor` at root level: HUD children follow camera with authored offsets.
- `PlayerAnchor` is purely a HUD authoring tool; pawn spawning is unaffected.

### Phase 2 — Introduce `Player` node ✅ COMPLETE (this session)

- [x] `XrdsScenePlayer` struct and `Player(...)` variant in `XrdsSceneNodePayload`.
- [x] `XrdsPlayerRoot` marker component in `xrds-runtime`.
- [x] Scene importer tags `Player` entities with `XrdsPlayerRoot`.
- [x] Palette + icon for `Player` in the editor UI.
- [x] `PlayerAnchor` works as child of `Player` — anchor systems already handle this
      correctly (they check the DIRECT parent for `XrdsPlayerAnchorRoot`).

**Phase 2 behavior summary:**
- `Player` is a static scene entity in Phase 2; it does not yet move with the pawn.
- `Player → PlayerAnchor → Text/Mesh` hierarchy is authorable and serializes correctly.
- Actual runtime locomotion driving `Player` transform is Phase 3.

### Phase 3 — Drive `Player` transform from pawn ✅ COMPLETE

- [x] At play-start, find `Player` node first; place pawn at its authored world transform
      (+ EYE_HEIGHT). Falls back to `PlayerSpawn`, then editor camera.
- [x] `sync_player_root_system` (PostUpdate, before TransformPropagate) writes pawn
      position + body yaw to every `XrdsPlayerRoot` entity each frame, so
      `PlayerAnchor` children (and their HUD texts) move with the player.
- [x] Anchor systems unchanged — they already use the camera for head-tracking;
      the Player entity's world position simply keeps children in the right place.
- [x] `PlayerSpawn` remains the fallback when no `Player` node exists.

**Phase 3 behavior summary:**

- `Player → PlayerAnchor → HUD texts` hierarchy follows the pawn in real time.
- Player entity uses body orientation (yaw only); anchor systems still use the
  full 6-DOF camera for head-locked offsets.
- `PlayerSpawn` still works as before for scenes without a `Player` node.

### Phase 4 — Multi-anchor switching ✅ COMPLETE

- [x] `ActivePlayerAnchorEntity` resource in `xrds-runtime`; `None` = all anchors active.
- [x] All four anchor systems (`head_locked`, `body_locked`, `comfort_pinned`, `cylindrical`)
      respect the active anchor via the shared `is_active_anchor()` helper.
- [x] `SetActivePlayerAnchor { id }` command; `EditorState::active_player_anchor_id` updated
      in `toolbar.rs`; reset to `None` on play stop.
- [x] `sync_active_anchor_system` translates node ID → Bevy entity via `XrdsIdIndex` each frame.
- [x] `PlayerPanel` component — list of all PlayerAnchor nodes below the Hierarchy panel.
      Click to activate; click again (or ✕) to clear. Parent Player name shown as context.

**Phase 4 behavior summary:**

- When no anchor is selected: all anchors active (Phase 1/2/3 behaviour unchanged).
- When an anchor is selected: only its children get camera-relative anchor math; other
  anchors' children follow the Player body transform without head-locking.
- Panel hides automatically when there are no PlayerAnchor nodes in the scene.

### Phase 5 — Editor UX ✅ COMPLETE

- [x] Distinct icons already in place (`🎮 Player`, `📍 PlayerAnchor`).
- [x] Inspector shows **"anchor-local offset"** (in blue) instead of "local to parent" when
      the selected node's direct parent is a `PlayerAnchor`.  Populated via `parent_kind`
      field added to `NodeInspectorDto`.
- [x] **Preview from anchor** — 👁 button in each PlayerPanel row.  Sends `PreviewFromAnchor`
      command; `orbit_camera_system` reads `preview_anchor_target`, extracts the anchor's
      authored world position + orientation, and teleports the editor camera there
      (`distance = 0.01`, yaw/pitch from authored rotation).  Works in edit mode so you
      can see HUD layout without pressing Play.

### Phase 6 — Runtime play-mode anchor switching ✅ COMPLETE

- [x] `PlayerAnchorCameraPose` component on every `XrdsPlayerAnchorRoot` entity.
      Initialised at play-start from authored world-space GlobalTransform.
      Updated with the pawn's last pose when switching away from an anchor, so
      switching back resumes from where the player left off.
- [x] `init_anchor_poses_system` — runs once per play session (after pawn spawns)
      to insert `PlayerAnchorCameraPose` on all anchor entities.
- [x] `switch_player_anchor_system` — detects changes to `ActivePlayerAnchorEntity`
      during play mode; saves departing pose, restores arriving pose, teleports pawn.
- [x] `sync_player_root_system` updated — when an anchor is active, only the parent
      `Player` entity of that anchor tracks the pawn; other Player entities stay at
      their authored positions.
- [x] Key bindings (play mode only):
      - **Tab** — cycle to next `PlayerAnchor` in document order.
      - **1–9** — jump directly to anchor N.
- [x] PlayerPanel 👁 button — in play mode sends `SetActivePlayerAnchor` (teleports
      pawn); in edit mode sends both `SetActivePlayerAnchor` and `PreviewFromAnchor`
      (activates anchor filter + moves editor camera).

**Phase 6 behavior summary:**

- During play, pressing **Tab** or clicking the 👁 button / anchor row in the
  Player panel teleports the pawn to the selected anchor's last-visited position.
- The first visit uses the authored world-space position; subsequent visits restore
  the camera orientation and position from the previous stay at that anchor.
- When multiple `Player` nodes exist, only the Player that owns the active anchor
  tracks the pawn — other Players remain at their authored scene positions.

### Phase 7 — Camera-space drag authoring ✅ COMPLETE

- [x] **FOV overlay** — global "FOV" toggle button in the PlayerPanel header. When enabled,
      draws a cyan frustum wireframe from each `PlayerAnchor`'s world position in the editor
      viewport (via Bevy Gizmos). The frustum uses the anchor's authored `fov_deg`, a fixed
      4 m depth, and 16:9 aspect. Selected anchors render brighter. Live-updated as the
      slider changes (via `XrdsAnchorFov` component + `set_anchor_fov_for_node`). FOV slider
      in the inspector is disabled during play mode (FOV applies on the next anchor switch).
- [x] **Drag-to-position for child nodes** — gizmo translate drag now correctly converts the
      world-space drag result to parent-local space before writing to the document. Root nodes
      (no parent) are unaffected (world = local). Child text/mesh nodes under `PlayerAnchor`
      now commit the correct anchor-local offset after drag.
