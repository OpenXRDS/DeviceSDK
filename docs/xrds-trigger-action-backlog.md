# `XrdsAction` backlog — candidate variants beyond the v1 draft

Companion to [`done/xrds-scenegraph-trigger-action-sequencing.md`](done/xrds-scenegraph-trigger-action-sequencing.md)
(the design doc) and [`done/xrds-trigger-action-v1.md`](done/xrds-trigger-action-v1.md)
(what shipped — the whole system, schema through the `xrds-editor` UI, is
done; nothing is "still ahead" anymore). **Not scheduled** — this is a
holding area for candidate `XrdsAction` variants beyond the ones already shipped.
Pull an entry out of here only when a real use case needs it, per the "don't
design for hypothetical future requirements" principle — this list exists so the
*idea* isn't lost, not as a commitment to build all of it.

**Shipped as of 2026-08-28**, since this list is only useful against an accurate
one: `SetTransform`, `SetVisible`, `SetMaterial`, `SetLight`, `ModifyHealth`,
`PlayGltfAnimation`, `StopGltfAnimation`, `PlayEffect`, `StopEffect`, `PlayAudio`,
`StopAudio`, `PlayVideo`, `StopVideo`, `SetElementText`, `SetElementValue`,
`SetElementEnabled`. The four this paragraph used to list alongside them —
`Teleport`, `Wait`, `FireCustomEvent`, `Run` — **were deleted**, not shipped:
`Teleport` turned out to be a zero-duration `SetTransform`, and the other three
went when the Track model replaced sequences. See the closing section for what
their removal cost.

Every entry below wraps an imperative capability that **already exists**
in `xrds-runtime`/`xrds-net` today — same relationship `PlayGltfAnimation`
has to `play_gltf_animation_in_world`. None of these require new engine
capability; they only require an `Action` wrapper + an `XrdsActionRunner`
match arm.

## Audio — **shipped**

Shipped as `PlayAudio` and `StopAudio`, both offered only on an `AudioClip` row.
`StopAudio` rewinds rather than stopping one-way, which is what makes it safe to
use on looping ambience.

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

- **Sketch:** `XrdsAction::SetCameraFov { fov }`, and the **per-anchor exposure
  override** — `XrdsAnchorExposureOverride`, not the scene-wide value.
- **Wraps:** `apply_anchor_fov_system`/`apply_anchor_exposure_system`
  (`crates/xrds-runtime/src/xrds_api/anchor.rs`).
- **Use case:** a camera "punch" on a hit reaction, or an exposure shift when
  entering a zone — cutscene-adjacent effects without a scripting system.
- **Which exposure this is.** The word covers two things in this codebase and
  only one of them belongs here. `XrdsSceneExposureEnvironment` is the scene-wide
  EV100 and lives on the environment; `XrdsAnchorExposureOverride` is a component
  on an anchor and means "this anchor instead", with `None` falling back to the
  scene value. Only the second has a node to hang an action on, which is exactly
  why it is a candidate and the scene-wide one is not — see below.

## Scene-wide environment settings are **not** getting an action

**Decided 2026-08-28: no.** Fog, scene exposure and colour grading stay authored
on the scene environment and are not addressable from a Track.

An earlier entry here proposed them, on the grounds that they are numeric values
with runtime setters like any other. The reason that was wrong is worth keeping,
because the same argument will be made again for the next global setting:

Every `XrdsActionTarget` is a node or a panel element — `SelfNode`, `Node(id)`,
`TriggerSource`, `Element { panel, name }`. A Track addresses *things in the
scene*, and it holds asset locks on them so two Tracks cannot fight over one
thing. A scene-wide setting has nothing to address and nothing to lock: giving it
a target means either a global lock, where one Track fading the fog blocks every
other Track in the scene, or an exception to the policy that keeps Tracks from
interfering. Neither buys enough to pay for it.

**The per-anchor exposure override is the exception that proves the rule** — it is
a component on a node, so it needs no new target kind and no new locking. If a
scene genuinely needs "the light changes when the player enters here", that is the
shape it should take.

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

## The escape hatch — **there isn't one any more**

This section used to say `XrdsAction::FireCustomEvent { name: String }` "already
covers anything not yet modeled and isn't going anywhere", and that every entry
above was a candidate to *promote* out of it rather than a replacement for it.

**`FireCustomEvent` no longer exists.** It was deleted along with `Wait` and `Run`
when the Track model replaced sequences — `XrdsAction`'s own doc comment says so.
So the framing above is inverted: the entries on this list have no fallback to be
promoted out of. A Track can do exactly what `XrdsAction` has variants for.

The two directions are not symmetrical, and only one survived:

- **App code → Track** still works. `XrdsTriggerKind::Custom` matches a named
  trigger an application fires, so app code can start a Track. Untouched.
- **Track → app code** is gone. Nothing lets a Track call out to expert-layer
  Rust at a moment in its timeline.

Whether that gap matters is deferred — a far-future question, not a v1.0 one.

**Where the custom trigger belongs, decided 2026-08-28.** `XrdsTriggerKind::Custom`
is an *expert-layer* concept: application Rust fires it, through the SDK or Bevy
directly. A scene authored and run from the editor alone never sees it. It stays
offered in the editor's trigger picker, because binding it in the document and
firing it from code is a real workflow and the document is where that binding
belongs — but it now says `fired from your own code, never by the editor` beside
the option, which is what a GUI-only author needed and did not have.

`RunawayDetected` was offered next to it and is a different case entirely: nothing
fires it at all. It guarded runaway `Run` chains against `MAX_RUN_CHAIN_DEPTH`, and
`Run` went with the sequence model — `fire_runaway_detected_in_world` is called
from nowhere and the depth constant is compared against nothing. It is no longer
offered, and the test that used to assert it always was now asserts the opposite.
