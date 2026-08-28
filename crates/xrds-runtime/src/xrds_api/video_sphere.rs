//! The inside-facing equirectangular sphere a 360 video is projected onto.
//!
//! Ported from `F:\workspace\rust\HMDViewer`, which ships this on a Quest 3 — see
//! `docs/video-360-plan.md`. The geometry is generated inside-facing rather than
//! drawn with front-face culling, which is what that implementation does and is
//! the better of the two: winding handles the interior and the UV formula handles
//! the horizontal mirror, so neither needs a render-state flag or a texture
//! transform to correct it afterwards.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;

/// Longitudinal divisions. 64 × 32 is smooth enough that the horizon of a 4K
/// equirect frame shows no faceting at the centre of view, and small enough that
/// the mesh is irrelevant next to the per-frame video upload.
const SEGMENTS: u32 = 64;
/// Latitudinal divisions.
const RINGS: u32 = 32;

/// Build the sphere a 360 video is mapped onto.
///
/// **Orientation matters and is copied deliberately**: `u = 0.5` faces **−Z**, the
/// direction a Bevy camera looks by default, and `v = 0` is the top of the video.
/// So the centre of the frame is straight ahead and the seam falls behind the
/// viewer. Getting this wrong is not a crash, it is a video that starts facing the
/// wrong way, which is the kind of thing that gets rediscovered by eye on a
/// headset instead of read off a comment.
pub fn inside_facing_equirect_sphere(radius: f32) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();

    for ring in 0..=RINGS {
        let v = ring as f32 / RINGS as f32;
        let theta = v * std::f32::consts::PI; // 0 at the top
        for seg in 0..=SEGMENTS {
            let u = seg as f32 / SEGMENTS as f32;
            let phi = (u - 0.5) * std::f32::consts::TAU; // 0 at -Z
            let x = radius * theta.sin() * phi.sin();
            let y = radius * theta.cos();
            let z = -radius * theta.sin() * phi.cos();
            positions.push([x, y, z]);
            uvs.push([u, v]);
            // Inward, because the surface is seen from the centre. The material is
            // unlit so nothing samples these, but a mesh whose normals disagree
            // with its winding is a trap for whatever reads it next.
            let inv = radius.max(f32::EPSILON);
            normals.push([-x / inv, -y / inv, -z / inv]);
        }
    }

    // Generated in the ordinary outward winding, then reversed. Stating it that
    // way — rather than hand-ordering the indices — means the intent survives:
    // the reversal *is* the inside-facing part, and it is one line to check.
    let row = SEGMENTS + 1;
    let mut indices: Vec<u32> = Vec::new();
    for ring in 0..RINGS {
        for seg in 0..SEGMENTS {
            let a = ring * row + seg;
            let b = a + row;
            for tri in [[a, a + 1, b], [a + 1, b + 1, b]] {
                indices.extend_from_slice(&[tri[2], tri[1], tri[0]]);
            }
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn positions(mesh: &Mesh) -> Vec<Vec3> {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(v) => v.iter().map(|p| Vec3::from(*p)).collect(),
            other => panic!("unexpected position format: {other:?}"),
        }
    }

    fn uvs(mesh: &Mesh) -> Vec<Vec2> {
        match mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            VertexAttributeValues::Float32x2(v) => v.iter().map(|p| Vec2::from(*p)).collect(),
            other => panic!("unexpected uv format: {other:?}"),
        }
    }

    /// Every triangle faces the centre.
    ///
    /// **This is the test the whole module exists for.** An outward-facing sphere
    /// is not subtly wrong from the inside — it is completely invisible, because
    /// every face is culled. And it looks identical to a missing texture, an
    /// unplayed video or a mis-sized radius, so getting it wrong costs a debugging
    /// session on a headset rather than a failing assertion here.
    #[test]
    fn every_face_points_inward() {
        let mesh = inside_facing_equirect_sphere(10.0);
        let pos = positions(&mesh);
        let Some(Indices::U32(idx)) = mesh.indices() else {
            panic!("expected u32 indices");
        };

        for tri in idx.chunks_exact(3) {
            let (a, b, c) = (
                pos[tri[0] as usize],
                pos[tri[1] as usize],
                pos[tri[2] as usize],
            );
            // Counter-clockwise winding is the front face, so this is the outward
            // normal of the triangle as the rasteriser sees it.
            let face = (b - a).cross(c - a);
            let centroid = (a + b + c) / 3.0;
            // Degenerate triangles at the poles have a near-zero cross product and
            // no meaningful facing; they are skipped rather than asserted on.
            if face.length_squared() < 1e-6 {
                continue;
            }
            assert!(
                face.dot(centroid) < 0.0,
                "a face at {centroid:?} points away from the centre — the sphere \
                 would be invisible from inside it"
            );
        }
    }

    /// The centre of the video is straight ahead, not off to one side.
    #[test]
    fn u_half_faces_negative_z() {
        let mesh = inside_facing_equirect_sphere(1.0);
        let pos = positions(&mesh);
        let uv = uvs(&mesh);

        // The vertex on the equator closest to u = 0.5.
        let (best, _) = uv
            .iter()
            .enumerate()
            .filter(|(i, t)| (t.y - 0.5).abs() < 1e-6 && pos[*i].length() > 0.5)
            .min_by(|a, b| {
                (a.1.x - 0.5)
                    .abs()
                    .partial_cmp(&(b.1.x - 0.5).abs())
                    .unwrap()
            })
            .expect("an equator vertex should exist");

        let dir = pos[best].normalize();
        assert!(
            dir.z < -0.99,
            "u=0.5 must face -Z, the default camera forward; it faces {dir:?}"
        );
    }

    /// The seam is behind the viewer, and it closes.
    ///
    /// `u = 0` and `u = 1` are separate vertices at the same place — that
    /// duplication is why the texture does not wrap incorrectly across the seam —
    /// so the check is that they coincide in space while differing in UV.
    #[test]
    fn the_uv_seam_closes_behind_the_viewer() {
        let mesh = inside_facing_equirect_sphere(1.0);
        let pos = positions(&mesh);
        let uv = uvs(&mesh);

        let row = (SEGMENTS + 1) as usize;
        let equator_ring = (RINGS / 2) as usize;
        let first = equator_ring * row;
        let last = first + SEGMENTS as usize;

        assert!((uv[first].x - 0.0).abs() < 1e-6, "row should start at u=0");
        assert!((uv[last].x - 1.0).abs() < 1e-6, "row should end at u=1");
        assert!(
            pos[first].distance(pos[last]) < 1e-5,
            "the two seam vertices must sit at the same point"
        );
        assert!(
            pos[first].normalize().z > 0.99,
            "the seam belongs behind the viewer, at +Z"
        );
    }

    /// Radius reaches the geometry, since a 360 sphere that is not large enough to
    /// contain the viewer is a sphere they are standing outside of.
    #[test]
    fn radius_scales_the_sphere() {
        for radius in [1.0f32, 50.0, 500.0] {
            let mesh = inside_facing_equirect_sphere(radius);
            for p in positions(&mesh) {
                assert!(
                    (p.length() - radius).abs() < radius * 1e-4,
                    "every vertex should sit on the sphere of radius {radius}"
                );
            }
        }
    }
}

#[cfg(test)]
mod attribute_tests {
    use super::*;

    /// The mesh carries normals.
    ///
    /// Bevy's `pbr_fragment.wgsl` reads `in.world_normal` and a mesh without a
    /// normal attribute fails pipeline specialisation with "invalid field accessor
    /// `world_normal`" — a shader error at draw time, not a build error, so nothing
    /// but a device or a test like this catches it.
    #[test]
    fn the_mesh_has_the_attributes_the_pbr_shader_requires() {
        let mesh = inside_facing_equirect_sphere(10.0);
        for (name, attr) in [
            ("position", Mesh::ATTRIBUTE_POSITION),
            ("normal", Mesh::ATTRIBUTE_NORMAL),
            ("uv", Mesh::ATTRIBUTE_UV_0),
        ] {
            assert!(
                mesh.attribute(attr).is_some(),
                "the mesh must carry {name}, or the PBR pipeline fails to compile"
            );
        }
        let count = mesh.count_vertices();
        for (name, attr) in [
            ("normal", Mesh::ATTRIBUTE_NORMAL),
            ("uv", Mesh::ATTRIBUTE_UV_0),
        ] {
            assert_eq!(
                mesh.attribute(attr).unwrap().len(),
                count,
                "{name} must have one entry per vertex"
            );
        }
    }
}
