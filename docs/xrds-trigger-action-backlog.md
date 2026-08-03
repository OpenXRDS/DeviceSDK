# `XrdsAction` backlog — candidate variants beyond the v1 draft

Companion to [`xrds-scenegraph-trigger-action-sequencing.md`](xrds-scenegraph-trigger-action-sequencing.md)
(the design doc), [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md)
(what shipped) and [`xrds-trigger-action-implementation-plan.md`](xrds-trigger-action-implementation-plan.md)
(what is still ahead). **Not scheduled** — this is a holding area for
candidate `XrdsAction` variants beyond the seven in the v1 draft
(`PlayGltfAnimation`, `StopGltfAnimation`, `SetVisible`, `Teleport`,
`ModifyHealth`, `Wait`, `FireCustomEvent`). None of these block Phase 1-5
of the implementation plan; pull an entry out of here only when a real
use case needs it, per the "don't design for hypothetical future
requirements" principle — this list exists so the *idea* isn't lost, not
as a commitment to build all of it.

Every entry below wraps an imperative capability that **already exists**
in `xrds-runtime`/`xrds-net` today — same relationship `PlayGltfAnimation`
has to `play_gltf_animation_in_world`. None of these require new engine
capability; they only require an `Action` wrapper + an `XrdsActionRunner`
match arm.

## Audio

- **Sketch:** `XrdsAction::PlayAudio { clip: XrdsAudioHandle, .. }`,
  `StopAudio`.
- **Wraps:** the existing document-level audio workflow
  (`examples/xrds_first/scene_document_audio_workflow.rs`).
- **Use case:** trigger a sound on zone-enter/hit/interaction, the most
  common "small trigger effect" in most interactive scenes.

## Material / appearance

- **Sketch:** `XrdsAction::SetMaterialProperty { target, property, value }`.
- **Wraps:** the existing material-editing path
  (`examples/xrds_first/edit_material.rs`).
- **Use case:** flash/tint an object on hit, fade an object in/out via
  base color alpha.

## Morph targets

- **Sketch:** `XrdsAction::SetMorphWeights { target, weights }`.
- **Wraps:** `gltf_morph_target_weights`/`gltf_morph_targets` (already in
  `xrds-runtime/src/xrds_api/api.rs`).
- **Use case:** facial expressions, soft-body-style effects driven by a
  trigger rather than a baked animation clip.

## Lifecycle, generalized beyond `SetVisible`

- **Sketch:** `XrdsAction::SetActive(bool)`, `XrdsAction::Spawn { node }`,
  `XrdsAction::Despawn`.
- **Wraps:** existing spawn-zone payload concept in the scene graph.
- **Use case:** "trigger fires → spawn this node" (e.g. spawn a pickup,
  an effect, an enemy) without a dedicated spawn system per case.

## Camera / anchor properties

- **Sketch:** `XrdsAction::SetCameraFov { fov }`, exposure changes.
- **Wraps:** `apply_anchor_fov_system`/`apply_anchor_exposure_system`
  (`crates/xrds-runtime/src/xrds_api/anchor.rs`).
- **Use case:** a camera "punch" on a hit reaction, or an exposure shift
  when entering a zone — cutscene-adjacent effects without a scripting
  system.

## UI / HUD

- **Sketch:** `XrdsAction::SetUiVisible { target, visible }`,
  `XrdsAction::SetUiValue { target, value }`.
- **Wraps:** the existing UI widget modules
  (`world_ui_button`/`world_ui_slider`/`world_ui_toggle`, referenced in
  `xrds_api.rs`).
- **Use case:** show a HUD prompt on zone-enter, update a health-bar
  widget's value when `ModifyHealth` fires — pairs naturally with the
  health example already used throughout the design doc.

## Physics

- **Sketch:** `XrdsAction::ApplyImpulse { target, impulse }`,
  `XrdsAction::SetPhysicsEnabled { target, enabled }`.
- **Wraps:** `avian3d`, already installed (`crates/xrds-runtime/src/xrds_api/install.rs`).
- **Use case:** knockback on hit, disabling a body when a trigger fires
  (e.g. a platform that stops moving once stepped on).

## Networking

- **Sketch:** `XrdsAction::SendNetworkMessage { topic, payload }`, routed
  through `xrds-net`.
- **Wraps:** `XrdsNet::dispatch`/the WebRTC data-channel send path — both
  hardened and real-network-verified this same work cycle (see
  `docs/done/xrds-net-release-readiness.md`).
- **Use case:** the multiplayer-sync case that motivated the whole
  bullet-hits-player stress test — "reduce HP locally *and* notify other
  clients" in one sequence step. Worth flagging as the least speculative
  entry on this list, given how much of this session's other work
  (`xrds-net` hardening, real-network WebRTC verification) already
  supports it directly.
- **Caveat, before this one gets built:** needs an authority model
  decided first. If every client independently simulates the same
  trigger locally (e.g. each client's own copy of a bullet hitting a
  player) and each fires `SendNetworkMessage` on its own, that's
  duplicate/diverging state across clients, not sync — the classic
  client-authority-vs-server-authority problem. `XrdsAction` shouldn't
  grow this variant until that's decided; it's a networking-architecture
  question, not a sequencer-schema one.

## The permanent escape hatch

`XrdsAction::FireCustomEvent { name: String }` already covers "anything
not yet modeled" and isn't going anywhere — every entry above is a
candidate to *promote* out of "fire a custom event and let expert-layer
Rust handle it" into a first-class, editor-authorable variant, not a
replacement for the escape hatch itself.
