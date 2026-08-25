# XRDS API Reference — Outline

**Status: outline only — but the rustdoc half is now done.**

Pass of 2026-08-24, before writing any of the narrative below:

- **Per-item coverage was already good** and is not where the gap was: `XrdsAPI`
  105/117 documented (89%), `XrdsUpdateContext` 98/111 (88%). About 25 public
  functions still lack a doc comment, listed by running the coverage check in that
  session; a worthwhile small job, not a blocker.
- **The landing pages were empty**, which is what a reader actually meets first.
  `xrds` — the crate that *is* the SDK — had zero lines of crate-level docs, as did
  `xrds-runtime`, `xrds-components`, `xrds-openxr` and `xrds-net`. `cargo doc --open`
  opened on a blank page. All but `xrds-net` now have one, and `xrds`'s carries a
  runnable example that is checked by `cargo test --doc`.
- **24 intra-doc links were broken** and are now zero. Most pointed from
  `xrds-components` at `XrdsAPI`, which lives in `xrds-runtime` — a link that could
  never resolve, because the dependency runs the other way. Those are code spans
  now. `cargo doc --workspace --no-deps` is clean, so a new break is visible.

What remains here is the narrative layer described below — task-oriented topics
that rustdoc's per-item view cannot give.

**Original status:** This is the table of contents and section scope for
a manual covering both the SDK layer (`XrdsApp`/`XrdsAPI`/`XrdsUpdateContext`)
and the expert layer (`RuntimeHandler`, direct Bevy). Per the project's own
two-layer model, these are one audience for documentation purposes — a reader
of one is a plausible reader of the other, and the boundary between them is
itself a topic this doc has to cover, not a reason to split it.

Audience: a Rust developer building an XR app against DeviceSDK, or
extending the SDK itself. Not the GUI editor's audience — see
`gui-user-manual-outline.md` for that.

## Relationship to rustdoc

This manual is the **narrative/topic layer** — task-oriented, cross-cutting,
explains *why* and *when*. `cargo doc --open` is the **exhaustive signature
layer** — every method, every field, generated and never stale. Each section
below should link to the relevant rustdoc module/type rather than
re-transcribing signatures. `XrdsAPI` alone has 112 public methods,
`XrdsUpdateContext` has 105 — hand-written exhaustive coverage of that surface
would drift out of sync within a week; grouping by task is what a
hand-written manual is actually good for.

## Sources already available to draw from

- `README.md` — the two-layer model statement, the "which type do I use"
  decision rule, the document-first/runtime-first split, build instructions.
  Largely reusable as-is for §0–§2, §6, §12.
- `ARCHITECTURE.md` — crate dependency diagram and rationale; source for §2's
  crate map.
- `docs/done/*.md` — design records for features that shipped. Each relevant
  one is cited in its section below; this manual should **link** to them for
  depth rather than re-deriving the design reasoning.
- `examples/` (60+ files across `xrds_first/`, `expert/`, `rendering/`,
  `networking/`, `webrtc/`, `extensions/`) — the closest thing to a worked
  tutorial already in the repo. §13 turns this into a real index; earlier
  sections should link the specific example that demonstrates each topic
  rather than inventing new code samples where a working one already exists.

## 0. Front Matter

- Who this is for; how to read this doc vs. rustdoc vs. `docs/done/*`.
- The two-layer model, restated from `README.md`: `XrdsApp`/`XrdsAPI`/
  `XrdsUpdateContext` (default) vs. `RuntimeHandler`/direct Bevy (expert).
- The strict-layering rule: the `xrds` crate does not re-export Bevy —
  dropping to the expert layer means importing `bevy` explicitly. State this
  once, prominently, since it is a common first-confusion point.

## 1. Getting Started

- Minimal working app: `XrdsApp` impl + `Runtime::new()` +
  `run_xrds_app`. Base this on `examples/xrds_first/simple_api.rs` and
  `simple_update.rs` rather than writing a new sample.
- `RuntimeParameters` basics: `app_name`, `enable_xr`, `asset_path` (and why
  an editor sub-crate needs to override it — see the doc comment on
  `RuntimeParameters::asset_path`).
- Where assets live; the `assets/` directory convention.

## 2. Core Concepts

- `XrdsApp::setup` vs `XrdsApp::update` — when each runs, what's in scope in
  each (`XrdsAPI` vs `XrdsUpdateContext`).
- `Handle<T>` — why `spawn()` returns one, what it's a typed wrapper around,
  how it differs from a Bevy `Entity`.
- The central decision rule from `README.md`, expanded into a table:
  runtime-facing types (`XrdsCube`, `XrdsCamera`, ...) vs. scene-document
  types (`XrdsSceneNode`, `XrdsSceneDocument`) — when each is the right one.
- `XrdsId` — the stable-across-reimport identity that both layers key off of;
  relevant the moment a reader needs to bridge from document data to a live
  entity (or from the editor, to the expert layer).
- Crate map (from `ARCHITECTURE.md`): `xrds-runtime`, `xrds-scene-graph`,
  `xrds-components`, `xrds-openxr`, `xrds-net`, `xrds-media`, and how they
  depend on each other. (`xrds-audio` is deprecated and excluded from the
  workspace — do not document it as part of the SDK.)

## 3. Spawning & Managing Objects

Grouped by the actual method clusters in `xrds_api.rs` (`spawn_*`, the 32
`set_*` setters, `register_*`, `queue_update`), not by an invented taxonomy:

- Primitives: `XrdsCube`/`Sphere`/`Cylinder`/`Capsule`/`Plane3D`/`Tetrahedron`
  — spawn, `set_*_geometry`, and the shared `physics_body`/`gravity_scale`/
  `mass` fields every primitive carries.
- Cameras, the four light types, `AudioClip`, `InteractionZone`,
  `PlayerSpawn`/`PlayerSpawnZone`, `Player`/`PlayerAnchor`.
- Text: `XrdsText` and `XrdsExtrudedText` (both render as real 3D geometry via
  `bevy_rich_text3d`/`bevy_fontmesh` — no `Text2d`/`Camera2d` dependency,
  worth stating plainly since `OVERALL_PROGRESS.md` had to correct a stale
  claim to the contrary).
- GLTF assets: import, the 6 `gltf_*` animation/morph-target methods.
- Hierarchy: `parent_*`/`child_*`/`children_of`, `duplicate_*`, `delete_*`,
  `rename_*`.

## 4. In-World UI (Panels)

- What a panel template is; the unified model (one template, two attachment
  points — world-placed vs. head-locked).
- The five widget kinds (Label/Button/Image/Slider/Toggle) and per-instance
  trigger bindings.
- The grab-handle model (`XrGrabHandle`/`XrGrabHandleOnly`) — a panel's face
  stays clickable, a dedicated bar moves it.
- Link to `docs/done/xrds-widget-template-plan.md` for the full design
  history; this section is the "how do I use this from Rust" companion, not
  a restatement of that doc.

## 5. Materials & Textures

- `XrdsMaterialParams`, the PBR field set, texture slots
  (`material_textures()`/`set_material_texture_slot()`).
- Texture UV transform modes — the `Raw` vs. center-based `rotation_deg`
  escape hatch already documented in `README.md`; expand with the actual
  method signatures.

## 6. Physics

- `XrdsPhysicsBody` (`None`/`Static`/`Dynamic`), how each primitive's
  collider is derived from its own dimensions (and the naming gotcha:
  avian3d's `Collider::cylinder`/`Collider::capsule` argument conventions
  differ — worth a callout given this was a live bug, fixed alongside adding
  `XrdsCapsule`).
- Live geometry/physics-param updates without reimport
  (`set_gravity_scale_for_node`, `set_mass_for_node`, `set_*_geometry`).
- Grab/throw, interaction zones (`XrZoneEnterEvent`/`XrZoneExitEvent`),
  raycasting.

## 7. Scene Environment

- IBL, skybox, manual exposure, procedural atmosphere, fog (linear / exponential /
  exponential-squared).
- The document-first vs. runtime-first split, from `README.md`:
  save/load-then-import vs. `merge_scene_assets`/`set_scene_environment`/
  `clear_scene_environment` directly on `XrdsAPI`.

## 8. Trigger / Action System (Tracks)

- What a Track is; the trigger → binding → action model.
- The shipped `XrdsAction` variants (`PlayGltfAnimation`, `StopGltfAnimation`,
  `SetVisible`, `Teleport`, `ModifyHealth`, `Wait`, `FireCustomEvent`, `Run`)
  and `FireCustomEvent` as the permanent escape hatch for anything not yet
  modeled.
- Link to `docs/done/xrds-trigger-action-v1.md` (what shipped) and
  `docs/xrds-trigger-action-backlog.md` (candidate variants, explicitly not
  scheduled — pull from there only when a real use case needs it).

## 9. Scene Document Model (`xrds-scene-graph`)

- `XrdsSceneDocument`/`XrdsSceneNode`, save/load, import/export.
- `XrdsSceneDocumentSession` — the undo/redo session model.
- When to use this layer at all vs. staying purely runtime-facing — the
  README's rule again, applied to document-authoring code rather than app code.

## 10. Networking (`xrds-net`)

- What's available: WebRTC data channels, capture/streaming.
- Release-readiness status — the transport layer is hardened and
  real-network-verified for an internal milestone
  (`docs/done/xrds-net-release-readiness.md`); state this plainly since
  `OVERALL_PROGRESS.md` previously undersold it as "deferred, low priority."
- Where it stops: no game-level multiplayer-sync action exists yet, and it
  needs an authority model decided before one should be added — see
  `docs/xrds-trigger-action-backlog.md`'s Networking entry.

## 11. Export

- Export as Application: bundling, `cargo build --release`, output layout.
- Scene GLB export is **retired** — state this once, plainly, here, so it
  isn't rediscovered as a bug report. glTF *import* is unaffected.

## 12. The Expert Layer

- `RuntimeHandler`'s lifecycle hooks (`on_construct`/`on_begin`/`on_resumed`/
  `on_suspended`/`on_end`/`on_update`/`on_deconstruct`) — note the
  Android/mobile-lifecycle shape of these names, since that's not obvious
  from the trait alone.
- `api.add_startup_system`/`api.add_update_system` — the escape hatch for
  direct Bevy systems without leaving `XrdsApp`.
- Walkthrough based on `examples/expert/direct_bevy.rs` and
  `sequential_actions_spike.rs`.
- What NOT to do when mixing layers — e.g., fighting the reimport system by
  hand-mutating entities the document layer also owns. (Needs a real
  example pulled from a past incident, not an invented one — check whether
  any `docs/done/*` postmortem already describes one before writing this.)

## 13. Platform Notes

- Android XR / Quest bringup — summarize and link
  `docs/android-xr-quest-bringup.md`.
- Desktop/Linux build notes — the `apt install` list and Linux-editor notes
  already in `README.md`; move here rather than duplicate.

## 14. Appendix

- **Example index**: a table of every file under `examples/`, one line each
  on what it demonstrates. Build this by actually reading each file's intent,
  not by filename guess — several (e.g. `descriptor_gen.rs`,
  `active_compo_control.rs`) don't self-describe from the name alone.
- Where generated docs live (`cargo doc --open`) and how to regenerate them.
- Glossary of XRDS-prefixed type names, since the prefix convention itself
  (`Xrds*` for document/component types, `Xr*` for a handful of
  runtime-only markers like `XrGrabbable`/`XrGrabHandle`) is not written
  down anywhere and is a real point of confusion the moment someone greps
  for a type and gets both.
