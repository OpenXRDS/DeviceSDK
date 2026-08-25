# LOD: deferred, and what an attempt taught

**Status: deferred.** Built through three phases, device-verified, then removed
unbuilt — deliberately, on 2026-08-24. Nothing in the tree implements LOD.

This document exists so the next attempt starts from what was learned rather than
from the same first idea.

## Why it was deferred

The thing worth having is **Unreal's model**: levels of detail living *inside a mesh
asset*, generated automatically by decimation. That needs a mesh simplification pass
in the import pipeline — its own project, not a phase of this one. What was built
instead was a hierarchy-based approximation, and the approximation is where all the
trouble came from.

Building it and deleting it was not waste. It answered questions that reading could
not, and every one of those answers is below.

## What Bevy actually provides

`bevy_camera::visibility::VisibilityRange` — a component giving one entity a distance
band. `VisibilityRangePlugin` is registered by `bevy_camera`'s own plugin group, so
**it is already running in every XRDS app**; there is no plugin to add and no
selection loop to write.

```rust
pub struct VisibilityRange {
    pub start_margin: Range<f32>,  // fades IN across this band
    pub end_margin: Range<f32>,    // fades OUT across this band
    pub use_aabb: bool,
}
```

The per-frame pass is `check_visibility_ranges`, in `PostUpdate`, before
`check_visibility`:

```rust
let model_position = entity_transform.translation_vec3a();
if visibility_range.is_visible_at_all((view_position - model_position).length()) {
    visibility |= 1 << view_index;
}
```

Facts worth keeping:

- **Plain Euclidean distance, camera to entity origin.** No frustum, no screen-space
  size, no hysteresis.
- **Measured per view**, as a `u32` bitmask, capped at 32 views. `check_visibility`
  reads it and returns early.
- **The band test is half-open**: `distance >= start_margin.start && distance <
  end_margin.end`. So `abrupt(0,20)` and `abrupt(20,70)` are exactly contiguous — no
  gap, no double-draw.
- **The range cull runs *before* the `no_frustum_culling` branch**, and independently
  of it. This is why it works on XRDS primitives at all, since we put
  `NoFrustumCulling` on them.
- **Nothing propagates to children.** A "level" must be a single renderable node; a
  level with children would leave those children drawing at every distance.
- **Crossfade is a 4×4 ordered dither** (`VISIBILITY_RANGE_DITHER`), applied by a
  `visibility_range_dither(...)` call inside `pbr.wgsl`'s fragment stage. Our material
  extension declares **its own `@fragment`**, so a fade may silently not reach XRDS
  materials. `abrupt` emits no dither at all, which is why abrupt-first was the safe
  order.

## What the device measurement showed

Quest 3, 2026-08-24. 600 objects in a 30×20 grid, shadows off. Baseline draws every
object at full detail; the LOD build switches Sphere → Cube → Tetrahedron at 15 m and
40 m, culling past 120 m. Frame time from the `handle_events tick frame=N` counter,
first 10 windows dropped as warm-up:

| | median | p25 | p75 | fps |
| --- | --- | --- | --- | --- |
| Baseline, 600 spheres | 17.00 ms | 16.00 | 17.69 | 59 |
| With LOD | 13.26 ms | 10.31 | 15.97 | 75 |

**3.74 ms, 22%** — baseline over the 13.9 ms budget for 72 Hz, LOD build under it.
The spread is itself evidence: the baseline is flat because it draws everything from
everywhere; the LOD build varies with where you stand.

**Read that number with its caveat.** The levels were Sphere → Cube → Tetrahedron,
three orders of magnitude apart in triangles. *A sphere is not a level of detail of a
tetrahedron* — that was a synthetic best case. A real scene's levels differ by perhaps
4×, and the win would be correspondingly smaller.

Two measurement mistakes worth not repeating:

- **The first scene was too small to prove anything.** At 150 objects the baseline
  measured 12.57 ms, indistinguishable from a ground-plus-one-cube scene's 13.0 ms
  during the atmosphere spike — both at the display cap. An A/B against a baseline
  that is not over budget cannot show a saving however well the feature works.
- **The first LOD reading said LOD was 2.1 ms *worse*.** Startup contamination: 2402
  entities spawn far slower than 602, and only three warm-up windows were dropped from
  a 29-window capture. A scene with 4× the entities needs proportionally more warm-up
  discarded.

Also learned: the LOD scene carried **4× the entities and was still faster**, so the
per-entity overhead of holding every level in the world is real but smaller than the
geometry it saves — *at that ratio*.

## Why the hierarchy model failed

An `XrdsLodGroup` node whose children are the levels. It worked in generated scenes
and fell apart the moment someone built one by hand, for one reason:

**Bevy measures each level's distance from that level's own position, and levels in a
hierarchy have their own transforms.**

Reported as three separate symptoms from one cause: the authored distance did not
match the ring drawn for it; switching responded mainly to movement along one axis;
and there was a band where the object vanished entirely.

The scene that made it unarguable had three levels laid out in a row:

| Level | World x | Its band |
| --- | --- | --- |
| Cube | 0.13 | [0, 9) |
| Sphere | 10.8 | [9, 18) |
| Cylinder | 18.7 | [18, ∞) |

From behind the cube, all three are within their own bands — **all three visible**.
Standing in front of it, the sphere is 4.95 m away and its band starts at 9 —
**culled**. It behaves like three proximity zones because, laid out that way, that is
exactly what it is.

**And spreading them out is the reasonable thing to do**, because three coincident
meshes are an unusable tangle in the editor. The model demanded an arrangement the
authoring experience made impossible to work with. That is a design fault, not a user
error.

### Two failed attempts to enforce coincidence

Both mutated authored transforms from a per-edit pass, and both fought the author:

1. **Snapping levels to the group's origin** teleported objects that were already
   placed, stacking them wherever the group happened to spawn.
2. **Moving the group to its first level** fixed that and introduced an accumulator:
   the pass runs on *every* edit, so each time a level was moved its offset was added
   to the group and zeroed on the level — the object jumped, and jumped further next
   time. Unrelated edits, such as adding a Player, flattened level positions as a side
   effect.

**The lesson generalises past LOD:** a pass that runs on every edit cannot tell "just
dropped in" from "being moved right now", so it must not mutate authored values.
Report and offer a fix; do not apply one silently.

### One thing the hierarchy model got right

Deriving contiguous bands from a single ordered list of switch distances, rather than
exposing Bevy's paired margins per node. The invariant that adjacent margins must
match exactly is not something to leave to hand-maintenance. Whatever replaces this,
keep that.

## What the next attempt should look like

**Levels belong to an asset, not to a hierarchy.** Unreal puts them inside the mesh; a
single node is trivially coincident with itself, and every failure above becomes
unrepresentable. The shape is `lod_meshes` on a `GltfAsset` node — a property, not a
parent-child relationship.

**Which needs LOD generation**, and that is the real blocker: mesh decimation (quadric
error metrics, or the `meshopt` crate's simplifier) run at import. Authors do not have
hand-modelled low-poly variants, and expecting them to is why the hierarchy model had
nothing to swap to. Godot's users rarely think about LOD precisely because generation
is automatic.

**Distances belong in named profiles, not on nodes.** Proposed while reconsidering
this, and worth keeping: a document-level registry beside `assets`, `tracks` and
`panels` —

```rust
pub struct XrdsLodProfile {
    pub name: String,      // "Props", "Buildings", "Foliage"
    pub radii: Vec<f32>,   // shells around the camera
    pub cull_at: Option<f32>,
}
```

Per-node distances do not scale: the device scene had 600 objects, so tuning meant
editing 600 nodes or none — and LOD exists *for* performance, so the knob has to be
reachable at the scale where it matters. Unreal ships exactly this as named LOD Group
presets (SmallProp, LargeProp, Foliage, Vista). Multiple profiles is what lets a
boulder and a pebble differ without going back to per-object numbers.

**Screen size beats distance as the authored metric.** Unity switches on
screen-relative height, Unreal on projected screen size, Godot on screen-space error.
All three fold in object size and FOV automatically; distance does not, which is why a
boulder and a pebble at 40 m get identical treatment. It needs no renderer change —
`distance ≈ radius / (target_fraction · tan(fov/2))` converts a screen fraction to the
distance `VisibilityRange` wants. Approximate for non-spherical objects, and tied to
the FOV at authoring time.

**An editor level picker is required, not optional.** Unity's LOD bar forces a chosen
level in the scene view. Without it, coincident levels cannot be inspected, which is
what pushed levels apart here in the first place.

## A smaller thing that is separable

**Distance culling alone** — no levels, no meshes, no generation. Any node gets a cull
radius from a profile, and the runtime applies `VisibilityRange::abrupt(0, r)`. A
single node is coincident with itself, so none of the failures above apply.

It is a fraction of the work and would buy real frame time on a Quest, but it is not
LOD and should not be called that. Recorded here because it is the one piece of this
that could ship without the generation project, if it is ever wanted.
