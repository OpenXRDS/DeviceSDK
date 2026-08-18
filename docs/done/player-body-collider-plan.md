# Player Body Collider

**Status:** implemented and **device-verified** 2026-08-18 on Quest 3. Walking onto a
marked pad fired a `ZoneEnter` Track and started an effect authored with
`auto_play = false`, so the plume could only have come from the trigger. Zero panics,
zero unresolved actions.

Default is **on** (`Some(XrdsPlayerBody::default())`), per the decision recorded below.

## Why (the state before this landed)

Everything in this section and the next describes the bug as it *was*. Both are kept
because the failure mode — authorable, validated, silently inert — is the kind that
returns if the id registration or the marker plumbing is ever refactored away.

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

## Design (as built)

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

**The editor takes the marker away and gives it back** — `sync_stereo_cameras`
*removes* `XrdsPlayerCamera` when stereo preview turns off (`viewport_camera.rs:315`)
and inserts it on the **same** entity when it turns on (line 326), re-inserting every
frame while enabled. So the attach system has to detach too, via a `RemovedComponents`
reader, or a body outlives its marker and keeps firing zone events.

An earlier draft of this section said the editor moves the marker *between two cameras*.
It does not, and the difference matters: the real sequence is remove/insert on one
entity at frame rate, which is what makes refusing duplicates load-bearing rather than
merely defensive. Verified by test, not by reading.

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

With `Collider::capsule(radius, height - 2*radius)` centred at the body's midpoint,
the collider is offset by `height/2 - eye_height` in local Y — **-0.75** for the default
1.7 m body at a 1.6 m eye height. As built this is computed from the camera's actual
`translation.y`, not from a hardcoded 1.6, so a different eye height still lands the base
on the floor.

### The reserved id

`XrdsIdAllocator` starts at `next: 1`, so **id 0 is unused** — reserve it for the
player and `register` the body entity in `XrdsIdIndex`.

This also gives triggers a stable way to express "the player did this", which
`XrZoneEnterEvent.entity_id` had no vocabulary for: every other id in that field is an
authored node. That prompted an audit of the field's readers — see Work items for the
outcome and the `TriggerSource` semantics it settled.

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

## Decided: the default is on

**`Some(XrdsPlayerBody::default())`.** Reasoning, and the counter-argument that was
weighed against it:

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

- [x] `XrdsPlayerBody` + `RuntimeParameters::player_body`, defaulting to `Some`.
- [x] Attach system (`xrds_api/player_body.rs`): on `XrdsPlayerCamera` added, insert
      `RigidBody::Kinematic` on the camera and a child carrying
      `Collider::capsule` + `CollisionEventsEnabled` at the computed offset.
- [x] Detach on `XrdsPlayerCamera` removal (`RemovedComponents`), ordered *before*
      attach so a marker moving between cameras within one frame is not refused as a
      duplicate.
- [x] Reserved id 0 (`XRDS_PLAYER_ID`) + `XrdsIdIndex::unregister`.
- [x] Tests (8, all passing): floor-level offset, reserved-id registration, observer
      mode attaches nothing, marker-move leaves nothing behind, degenerate shape is
      clamped rather than panicking, the editor's toggle-and-re-insert sequence never
      accumulates bodies, movement is not blocked (with a physics-is-live control), and a
      zone trigger sourced from the player fires.
- [x] Audit readers of `XrZoneEnterEvent.entity_id`. **One consumer exists:**
      `consume_triggers`, which calls `event.source().resolve(&id_index)`. Since
      `XRDS_PLAYER_ID` *is* registered, that resolves to the body collider rather than
      `None`, so nothing silently no-ops. Pinned by
      `a_zone_trigger_sourced_from_the_player_still_fires`.

      **Semantics this settles:** for a zone trigger, `XrdsActionTarget::TriggerSource`
      resolves to the player's **body collider child**, not the camera. Positionally that
      is the body centre — sensible for spawning an effect at the player — but it is not a
      mesh entity, so visibility-style actions aimed at it have nothing to act on. And
      `XrdsActionValue::FromTriggerSource` finds no `XrdsTriggerValue` there, degrading to
      `0.0` with a warning, which is the documented fallback rather than a new failure.

### Implementation notes worth knowing

**The collider is a child entity, not a component on the camera.** The camera transform
sits at eye height while the capsule must stand on the floor, and a child carries that
offset natively; avian3d attributes a child collider to the parent rigid body through
`ColliderOf`. The offset is derived from the camera's actual `translation.y` rather than
assuming 1.6 m.

Consequence: the entity in a collision event — and therefore the one holding
`XRDS_PLAYER_ID` — is the **child**, not the camera.

**Known limitation: the capsule inherits camera rotation.** Because it is a child of the
camera and the desktop fly-camera writes pitch into that transform, looking up or down
tilts the body. In XR, locomotion is yaw-only, so this does not arise there. For zone
overlap the error is small, and the alternative (counter-rotating the child every frame)
buys little for a v1 whose purpose is trigger volumes.

**Duplicate bodies are refused, not stacked.** A second body would double every zone
event, so attach bails while any body still exists.

## Verification

- [x] `cargo check --workspace --all-targets` clean; 217 xrds-runtime / 190
      xrds-scene-graph / 35 xrds-editor passing, no regression.
- [x] Editor marker sequence — and the first version of this item described the wrong
      sequence. `sync_stereo_cameras` does **not** move the marker between two cameras:
      it removes `XrdsPlayerCamera` from *one* entity when stereo preview turns off and
      inserts it back on the **same** entity when it turns on, and that insert runs
      **every frame** while enabled.

      Covered by `toggling_the_marker_on_one_entity_never_accumulates_bodies`, which
      toggles three times and re-inserts an already-present marker repeatedly. This is
      also what makes the duplicate-refusal guard load-bearing rather than defensive:
      without it the editor's per-frame insert is the exact shape that would stack bodies
      and double every zone event.

      Still not exercised through the real GUI, only through the sequence it performs.
- [x] **On device**, per `docs/quest-device-test-recipe.md`. Scene: a bright unlit
      `PAD`, a box `ZONE` covering it (±1.5 m in Y so it cannot be stepped over or
      under), and a `PLUME` with `auto_play = false` bound through Track `on_enter`.
      Walking on produced the plume. 5 entities loaded, 0 `did nothing` warnings,
      0 panics.

      **Caveat on the evidence:** nothing in the runtime logs zone events, so this rests
      on the visual plus the causal chain — `auto_play = false` means the only route to a
      running plume is `PlayEffect`, reachable only from the `ZoneEnter` binding. The
      recipe's Trap 10 implied `zoneenter` could be grepped; it cannot. Grab logs
      `GRABBED`; zones log nothing. Worth adding a log line so a future check can
      separate "event never fired" from "event fired, action failed" without relying on
      someone describing what they saw.
- [x] The player still walks through walls — this feature gives zones something to
      detect, it does not add movement blocking. Covered by
      `the_player_body_does_not_block_movement_through_static_geometry`: a static wall is
      placed across the path and the camera transform is driven through it frame by frame,
      as locomotion does.

      The test carries a **control**: a dynamic probe under gravity must have moved by the
      end. Without it the test would pass equally in a world where physics never steps,
      which would make the real assertion worthless. The control passes, so avian is
      genuinely stepping in the headless harness and the pass-through result means
      something.

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
