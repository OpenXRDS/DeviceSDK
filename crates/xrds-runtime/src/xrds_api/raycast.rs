use std::collections::HashSet;
use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use xrds_components::{XrRayhit, XrdsId};
use super::state::XrdsIdIndex;

/// Cast a ray against all XRDS scene entities and return hits sorted nearest-first.
///
/// Uses world-space AABB intersection (conservative — slightly enlarged for rotated
/// meshes). One hit per XRDS entity: GLTF submesh children all resolve to their
/// common XRDS ancestor so only the closest face registers.
pub(super) fn raycast_world(
    world: &mut World,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Vec<XrRayhit> {
    let dir = direction.normalize_or_zero();
    if dir == Vec3::ZERO {
        return vec![];
    }

    // Build QueryState and collect hits — the &mut borrow ends after this block.
    let candidates: Vec<(Entity, f32)> = {
        let mut q = world.query::<(Entity, &Aabb, &GlobalTransform)>();
        q.iter(world)
            .filter_map(|(entity, aabb, gt)| {
                ray_vs_world_aabb(origin, dir, aabb, gt)
                    .filter(|&t| t <= max_distance)
                    .map(|t| (entity, t))
            })
            .collect()
    };

    // Resolve each hit entity to its nearest XrdsId ancestor.
    // Both `world.resource()` and `world.get()` are shared borrows — fine to interleave.
    let mut hits: Vec<XrRayhit> = candidates
        .into_iter()
        .filter_map(|(entity, t)| {
            find_xrds_ancestor(world, entity).map(|id| XrRayhit {
                id,
                distance: t,
                point: origin + dir * t,
            })
        })
        .collect();

    // Sort nearest-first, then keep only the closest hit per unique XRDS entity.
    hits.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = HashSet::new();
    hits.retain(|h| seen.insert(h.id));
    hits
}

/// Ray vs. world-space AABB. Returns the entry distance (`>= 0`) or `None` on miss.
///
/// Converts the local-space AABB to a world-space one by multiplying the local
/// half-extents by the absolute values of the GlobalTransform rotation columns
/// (exact for axis-aligned transforms, conservative for rotated ones).
fn ray_vs_world_aabb(
    origin: Vec3,
    dir: Vec3,
    aabb: &Aabb,
    gt: &GlobalTransform,
) -> Option<f32> {
    let local_center = Vec3::from(aabb.center);
    let local_half   = aabb.half_extents; // Vec3A

    let world_center = gt.transform_point(local_center);
    let m = gt.affine().matrix3; // Mat3A — rotation+scale columns

    // World half-extents: |R| * local_half (each world axis gets the projected radius)
    let world_half = Vec3::new(
        m.x_axis.abs().dot(local_half),
        m.y_axis.abs().dot(local_half),
        m.z_axis.abs().dot(local_half),
    );

    let world_min = world_center - world_half;
    let world_max = world_center + world_half;

    // Slab method — clamp near-zero denominator to avoid NaN
    let inv = Vec3::new(safe_recip(dir.x), safe_recip(dir.y), safe_recip(dir.z));
    let t1 = (world_min - origin) * inv;
    let t2 = (world_max - origin) * inv;

    let t_enter = t1.min(t2).max_element();
    let t_exit  = t1.max(t2).min_element();

    if t_exit >= t_enter.max(0.0) {
        Some(t_enter.max(0.0))
    } else {
        None
    }
}

/// Walk the `ChildOf` chain from `entity` upward until an entry in `XrdsIdIndex` is found.
fn find_xrds_ancestor(world: &World, mut entity: Entity) -> Option<XrdsId> {
    let id_index = world.resource::<XrdsIdIndex>();
    loop {
        if let Some(id) = id_index.id_of(entity) {
            return Some(id);
        }
        entity = world.get::<ChildOf>(entity)?.0;
    }
}

#[inline]
fn safe_recip(v: f32) -> f32 {
    if v.abs() > 1e-10 { 1.0 / v } else { f32::INFINITY }
}
