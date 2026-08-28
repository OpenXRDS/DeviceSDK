# `XrdsAction` backlog — candidate variants beyond the v1 draft

Companion to [`done/xrds-scenegraph-trigger-action-sequencing.md`](done/xrds-scenegraph-trigger-action-sequencing.md)
(the design doc) and [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md)
(what shipped — the whole system, schema through the `xrds-editor` UI, is
done; nothing is "still ahead" anymore). **Not scheduled** — this is a
holding area for candidate `XrdsAction` variants beyond the ones already
shipped (`PlayGltfAnimation`, `StopGltfAnimation`, `SetVisible`, `Teleport`,
`ModifyHealth`, `Wait`, `FireCustomEvent`, `Run`). Pull an entry out of here
only when a real use case needs it, per the "don't design for hypothetical
future requirements" principle — this list exists so the *idea* isn't
lost, not as a commitment to build all of it.

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

## Material / appearance — **shipped**

Shipped as `XrdsAction::SetMaterial`, and as of 2026-08-28 it interpolates:
`duration_secs` + `ease`, so "fade an object out via base colour alpha" — the use
case this entry named — is now literally what it does rather than something an
author would have had to approximate with a burst of instant events. See
[`done/animatable-properties.md`](done/animatable-properties.md).

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

## Scene environment — fog, exposure, colour grading

- **Sketch:** `XrdsAction::SetFog { visibility, .. }`, `SetExposure { ev100 }`,
  animated the same way `SetLight` and `SetMaterial` now are.
- **Wraps:** the managed-marker environment sync in
  `crates/xrds-runtime/src/xrds_api/environment.rs`, which already applies these
  to every `Camera3d`.
- **Blocked on a decision, not on work.** `done/animatable-properties.md` claimed
  these were "a match arm and a tween component". They are not: **every**
  `XrdsActionTarget` is a node or a panel element — `SelfNode`, `Node(id)`,
  `TriggerSource`, `Element { panel, name }` — and the scene environment is
  neither. An action for it needs a new target kind, which is a decision about
  what a Track may address.
- **The question that decides it:** a Track holds *asset locks* so two Tracks
  cannot fight over the same thing, under the reject-the-newcomer policy. The
  environment is the one thing every Track would contend for, so giving it a
  target means either a global lock — where one Track fading the fog blocks every
  other Track in the scene — or an exception to the policy. Neither is obviously
  right, which is why this is not started rather than half-built.

## UI / HUD — **shipped**

Shipped as `SetElementText`, `SetElementValue` and `SetElementEnabled`, addressed
by `XrdsActionTarget::Element { panel, name }` rather than by the `target` field
this entry sketched. `SetElementValue` interpolates as of 2026-08-28 — a health
bar filling over time is one event — though only on a Slider: a Toggle has no
halfway between checked and unchecked. Visibility is `SetVisible` on the panel
node; there is no separate UI variant of it.

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
