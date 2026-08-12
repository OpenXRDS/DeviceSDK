# Player Body Collider — plan

**Status:** planned, not started. Independent of the VFX/particle work; shares no
files with it.

## Why

`XrdsInteractionZone` spawns a working avian3d sensor
(`Collider + Sensor + CollisionEventsEnabled`, `spawn.rs:1157`), and
`zone_collision_system` faithfully translates collisions into
`XrZoneEnterEvent`/`XrZoneExitEvent`. But **nothing gives the player a collider**,
so the player is absent from avian's broad-phase entirely and can never appear in a
collision pair. Walking into a zone does nothing at all.

Confirmed on a Quest 3 during the VFX device pass: a marked, correctly-placed zone
produced zero zone events. `grep -rn Collider crates/xrds-openxr/src` returns
nothing.

"Walk into a trigger volume" is a fundamental XR interaction, and today an author
can wire it up in the editor, see it validate, and get silence on device. That
failure mode — authorable but inert — is the actual cost here.

## Two blockers, not one

This is the part to keep in mind, because fixing only the first would look correct,
compile, pass review, and change nothing.

**1. No collider.** Covered above.

**2. No `XrdsId` on the player.** From `zone.rs:21`:

```rust
if let (Some(zone_id), Some(entity_id)) = (id_index.id_of(e1), id_index.id_of(e2)) {
    enter.write(XrZoneEnterEvent { zone_id, entity_id });
}
```

`XrZoneEnterEvent` carries **both** ids and the write sits inside the `if let`.
`XrdsIdIndex::register` is only called on the authored-node spawn path, and the
player camera is spawned by host-app code — so `id_of(player)` is `None`, the
pattern fails, and no event is written even with a collider present.

## Design

### Opt-in, because "player without physics" is a real mode

Per the observer/spectator case: the body is something you *add*, not a property the
camera always carries. That matches every engine — Unreal ships `ACharacter`
(capsule) and `ASpectatorPawn` (collision off) as separate classes; Unity's
`CharacterController` is a component you attach; Godot splits `CharacterBody3D` from
a plain `Camera3D`.

### Attach to the marker, don't spawn the player

**The SDK does not own the player entity.** `spawn_app_camera`
(`apps/xrds-app/src/main.rs:342`) spawns it, and `apps/xrds-editor` has its own
(`viewport_camera.rs:326`). What they share is the `XrdsPlayerCamera` marker.

So this must be a runtime system that *attaches* a body to whatever carries
`XrdsPlayerCamera`, not a spawn-path change. Putting the config in `xrds-app`'s
`SpawnConfig` would leave the editor out; putting it in `RuntimeParameters` keeps it
SDK-level where zones already live.

```rust
// RuntimeParameters
pub player_body: Option<XrdsPlayerBody>,   // None = observer, no physics

pub struct XrdsPlayerBody {
    pub height: f32,   // 1.7
    pub radius: f32,   // 0.25
}
```

**The editor moves the marker between entities** — `viewport_camera.rs:315` *removes*
`XrdsPlayerCamera` before line 326 inserts it elsewhere. The attach system therefore
has to detach too, or a stale capsule is left behind on the previous camera, still
firing zone events from wherever that camera sits. A `RemovedComponents` reader, or
`Added`/`Changed` plus a cleanup pass.

### Kinematic — and be explicit about what that does not buy

`RigidBody::Kinematic` + `Collider::capsule`. Locomotion writes the root `Transform`
directly, so kinematic gives exactly the requested semantics: the player **pushes**
dynamic props and is **not stopped** by static geometry. `Dynamic` would fight the
transform writes and jitter.

**This is not wall collision.** The player still walks through walls. Blocking
movement needs locomotion to shapecast before it moves, which is a separate feature
(see Out of scope). Worth stating loudly, because "the player has a collider now" is
very naturally misread as "the player collides with things".

### Capsule placement

The `XrdsPlayerCamera` transform is at **eye height** (`(0, 1.6, 8)` by default), not
at the feet. A 1.7m capsule centred on the camera would sink half a body into the
floor and its top would stand above the head.

With `Collider::capsule(radius, height - 2*radius)` centred at the body's midpoint:
offset the collider **-0.75** in local Y (body centre `0.85` − eye `1.6`), so the
base lands on the floor. If eye height ever becomes configurable this must derive
from it rather than hardcode `1.6`.

### The reserved id

`XrdsIdAllocator` starts at `next: 1`, so **id 0 is unused** — reserve it for the
player and `register` the body entity in `XrdsIdIndex`.

This also gives triggers a stable way to express "the player did this", which
`XrZoneEnterEvent.entity_id` currently has no vocabulary for: today every id in that
field is an authored node. Anything reading `entity_id` should be checked for how it
handles an id that resolves to no document node.

`XrdsIdIndex::register` is `pub(super)`, so this has to live inside `xrds_api`.

## Grab: no conflict, but the reason is incidental

An earlier read of this flagged grab interference as a risk. It is not, in this
codebase: `grab.rs:147` uses `raycast_world_meshes`, which queries
`(Entity, &Aabb, &GlobalTransform)` — Bevy's **render** `Aabb`, present only on mesh
entities. A bare physics capsule has no `Mesh3d`, therefore no `Aabb`, and would fail
the `XrGrabbable` filter regardless. Two independent reasons it cannot be hit.

**But that immunity is accidental, not designed.** If grab ever moves to physics
raycasts — the natural upgrade, since mesh-Aabb raycasting is coarse — the player
capsule becomes a hit candidate immediately, and the fix is the standard engine
idiom: exclude the ray's own originator (`TraceParams.AddIgnoredActor(this)` in
Unreal, `RayCast3D.add_exception(self)` in Godot, `layerMask` in Unity). We have
**no** layer machinery today (`grep CollisionLayers` → nothing), so that work belongs
with the grab change, not here.

## Open decision: the default

Recommendation: **`Some(XrdsPlayerBody::default())`** — on.

An author who drops an `InteractionZone` expects walking into it to fire; silence is
the worst outcome and is exactly what happened on device. Observer mode opts out
explicitly, as Unreal's spectator does.

The usual "don't change default behaviour" objection is weak here: because the body
is kinematic, enabling it changes nothing about how the player moves. The only new
behaviour is zone events firing and dynamic props getting shoved.

Counter-argument worth weighing: props being shoved by an invisible body *is* a
visible behaviour change in existing scenes, and it arrives without the author asking
for it.

## Work items

- [ ] `XrdsPlayerBody` + `RuntimeParameters::player_body`, defaulted per the decision
      above.
- [ ] Attach system in `xrds_api`: on `XrdsPlayerCamera` added, insert
      `Collider::capsule` + `RigidBody::Kinematic` at the computed offset; register
      the reserved id.
- [ ] Detach on `XrdsPlayerCamera` removal (`RemovedComponents`) — the editor
      relies on this.
- [ ] Reserve id 0 in `XrdsIdAllocator` and document it as the player.
- [ ] Audit readers of `XrZoneEnterEvent.entity_id` for an id with no backing
      document node.
- [ ] Tests: zone enter/exit fires for the player; `player_body: None` attaches
      nothing and fires nothing; moving the marker detaches the old body; the capsule
      base sits at floor level, not at eye level.

## Verification

- [ ] `cargo check --workspace --all-targets` clean.
- [ ] `cargo test -p xrds-runtime -p xrds-scene-graph -p xrds-editor` no regression.
- [ ] Editor: enter a zone in play mode, confirm the Track fires; toggle play/edit
      mode repeatedly and confirm no duplicate or stale body (the detach path).
- [ ] **On device**, per `docs/quest-device-test-recipe.md` — this is the case that
      was broken, so desktop-only sign-off would be missing the point. Mark the zone
      visibly (Trap 6) and grep for the event (Trap 10).
- [ ] Confirm the player still walks through walls, i.e. that this did not
      accidentally grow into movement blocking.

## Out of scope

- **Wall/terrain collision and blocked movement.** Needs a locomotion shapecast;
  much larger, and touches comfort/motion-sickness behaviour.
- **Crouch/height tracking.** The body is a fixed capsule at the root, per the
  decision above; a body that follows real head height is a separate feature with its
  own zone-triggering consequences.
- **Collision layers.** Nothing needs them until grab becomes physics-based.
- **Hand/controller colliders.** Separate; grab is mesh-raycast today.
- **Networked player bodies.** Blocked on the same authority-model decision as
  `XrdsAction::SendNetworkMessage` (`docs/xrds-trigger-action-backlog.md`).
