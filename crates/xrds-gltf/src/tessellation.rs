/// Pure-math mesh tessellation for XRDS primitives.
///
/// Each function returns `(positions, normals, uvs, indices)` where positions
/// and normals are flat `[x,y,z, x,y,z, ...]` f32 slices and indices are u32.
use std::f32::consts::PI;

// ── Sphere ────────────────────────────────────────────────────────────────────

pub fn sphere(radius: f32, lon_segs: u32, lat_segs: u32) -> MeshData {
    let mut pos: Vec<f32> = Vec::new();
    let mut nor: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for lat in 0..=lat_segs {
        let theta = lat as f32 * PI / lat_segs as f32;
        let sin_t = theta.sin();
        let cos_t = theta.cos();

        for lon in 0..=lon_segs {
            let phi = lon as f32 * 2.0 * PI / lon_segs as f32;
            let sin_p = phi.sin();
            let cos_p = phi.cos();

            let nx = sin_t * cos_p;
            let ny = cos_t;
            let nz = sin_t * sin_p;
            pos.extend_from_slice(&[nx * radius, ny * radius, nz * radius]);
            nor.extend_from_slice(&[nx, ny, nz]);
            uvs.extend_from_slice(&[lon as f32 / lon_segs as f32, lat as f32 / lat_segs as f32]);
        }
    }

    for lat in 0..lat_segs {
        for lon in 0..lon_segs {
            let a = lat * (lon_segs + 1) + lon;
            let b = a + lon_segs + 1;
            idx.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }

    MeshData { pos, nor, uvs, idx }
}

// ── Cuboid ────────────────────────────────────────────────────────────────────
// 24 vertices (4 per face, hard normals), 36 indices.

pub fn cuboid(w: f32, h: f32, d: f32) -> MeshData {
    let (hx, hy, hz) = (w * 0.5, h * 0.5, d * 0.5);

    // Each face: 4 positions, normal, 4 UVs, 2 triangles.
    // Order: +X, -X, +Y, -Y, +Z, -Z
    #[rustfmt::skip]
    let faces: &[([f32;3], [f32;3], [[f32;2];4])] = &[
        // +X
        ([hx,hy,hz],[1.,0.,0.], [[1.,0.],[1.,1.],[0.,1.],[0.,0.]]),
        ([hx,-hy,-hz],[1.,0.,0.], [[0.,0.],[1.,0.],[1.,1.],[0.,1.]]),
        ([hx,-hy,hz],[1.,0.,0.], [[0.,0.],[0.,1.],[1.,1.],[1.,0.]]),
        ([hx,hy,-hz],[1.,0.,0.], [[1.,1.],[0.,1.],[0.,0.],[1.,0.]]),
        // (rewind to make faces; we emit tris below)
        // --- simpler: just emit each face as two triangles directly ---
    ];
    let _ = drop(faces);

    let face_data: &[([[f32; 3]; 4], [f32; 3])] = &[
        // +X  verts in CCW when viewed from outside
        (
            [[hx, -hy, -hz], [hx, hy, -hz], [hx, hy, hz], [hx, -hy, hz]],
            [1., 0., 0.],
        ),
        // -X
        (
            [
                [-hx, -hy, hz],
                [-hx, hy, hz],
                [-hx, hy, -hz],
                [-hx, -hy, -hz],
            ],
            [-1., 0., 0.],
        ),
        // +Y
        (
            [[-hx, hy, hz], [hx, hy, hz], [hx, hy, -hz], [-hx, hy, -hz]],
            [0., 1., 0.],
        ),
        // -Y
        (
            [
                [-hx, -hy, -hz],
                [hx, -hy, -hz],
                [hx, -hy, hz],
                [-hx, -hy, hz],
            ],
            [0., -1., 0.],
        ),
        // +Z
        (
            [[hx, -hy, hz], [hx, hy, hz], [-hx, hy, hz], [-hx, -hy, hz]],
            [0., 0., 1.],
        ),
        // -Z
        (
            [
                [-hx, -hy, -hz],
                [-hx, hy, -hz],
                [hx, hy, -hz],
                [hx, -hy, -hz],
            ],
            [0., 0., -1.],
        ),
    ];

    let face_uvs: [[f32; 2]; 4] = [[0., 1.], [0., 0.], [1., 0.], [1., 1.]];

    let mut pos: Vec<f32> = Vec::new();
    let mut nor: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for (verts, n) in face_data {
        let base = (pos.len() / 3) as u32;
        for (i, v) in verts.iter().enumerate() {
            pos.extend_from_slice(v);
            nor.extend_from_slice(n);
            uvs.extend_from_slice(&face_uvs[i]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    MeshData { pos, nor, uvs, idx }
}

// ── Cylinder ──────────────────────────────────────────────────────────────────

pub fn cylinder(radius: f32, height: f32, segments: u32) -> MeshData {
    let mut pos: Vec<f32> = Vec::new();
    let mut nor: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    let hy = height * 0.5;

    // Side vertices (2 rings, top and bottom, with one extra for seam)
    let ring_base = 0u32;
    for i in 0..=segments {
        let phi = i as f32 * 2.0 * PI / segments as f32;
        let (s, c) = (phi.sin(), phi.cos());
        let u = i as f32 / segments as f32;
        // bottom vertex
        pos.extend_from_slice(&[c * radius, -hy, s * radius]);
        nor.extend_from_slice(&[c, 0., s]);
        uvs.extend_from_slice(&[u, 1.]);
        // top vertex
        pos.extend_from_slice(&[c * radius, hy, s * radius]);
        nor.extend_from_slice(&[c, 0., s]);
        uvs.extend_from_slice(&[u, 0.]);
    }
    // Side quads
    for i in 0..segments {
        let b = ring_base + i * 2;
        idx.extend_from_slice(&[b, b + 2, b + 1, b + 1, b + 2, b + 3]);
    }

    // Top cap
    let top_center = (pos.len() / 3) as u32;
    pos.extend_from_slice(&[0., hy, 0.]);
    nor.extend_from_slice(&[0., 1., 0.]);
    uvs.extend_from_slice(&[0.5, 0.5]);
    let top_ring_start = (pos.len() / 3) as u32;
    for i in 0..segments {
        let phi = i as f32 * 2.0 * PI / segments as f32;
        let (s, c) = (phi.sin(), phi.cos());
        pos.extend_from_slice(&[c * radius, hy, s * radius]);
        nor.extend_from_slice(&[0., 1., 0.]);
        uvs.extend_from_slice(&[c * 0.5 + 0.5, s * 0.5 + 0.5]);
    }
    for i in 0..segments {
        let a = top_ring_start + i;
        let b = top_ring_start + (i + 1) % segments;
        idx.extend_from_slice(&[top_center, a, b]);
    }

    // Bottom cap
    let bot_center = (pos.len() / 3) as u32;
    pos.extend_from_slice(&[0., -hy, 0.]);
    nor.extend_from_slice(&[0., -1., 0.]);
    uvs.extend_from_slice(&[0.5, 0.5]);
    let bot_ring_start = (pos.len() / 3) as u32;
    for i in 0..segments {
        let phi = i as f32 * 2.0 * PI / segments as f32;
        let (s, c) = (phi.sin(), phi.cos());
        pos.extend_from_slice(&[c * radius, -hy, s * radius]);
        nor.extend_from_slice(&[0., -1., 0.]);
        uvs.extend_from_slice(&[c * 0.5 + 0.5, s * 0.5 + 0.5]);
    }
    for i in 0..segments {
        let a = bot_ring_start + i;
        let b = bot_ring_start + (i + 1) % segments;
        idx.extend_from_slice(&[bot_center, b, a]); // reversed winding
    }

    MeshData { pos, nor, uvs, idx }
}

// ── Plane ─────────────────────────────────────────────────────────────────────
// Lies in the XZ plane, normal pointing +Y.

pub fn plane(w: f32, d: f32) -> MeshData {
    let (hw, hd) = (w * 0.5, d * 0.5);
    MeshData {
        pos: vec![-hw, 0., -hd, hw, 0., -hd, hw, 0., hd, -hw, 0., hd],
        nor: vec![0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1., 0.],
        uvs: vec![0., 0., 1., 0., 1., 1., 0., 1.],
        idx: vec![0, 1, 2, 0, 2, 3],
    }
}

// ── Tetrahedron ───────────────────────────────────────────────────────────────

pub fn tetrahedron(radius: f32) -> MeshData {
    let a = radius;
    // 4 vertices of a regular tetrahedron inscribed in a sphere of the given radius.
    let v0 = [0.0_f32, a, 0.0];
    let v1 = [2.0 * a * (2.0_f32 / 3.0).sqrt(), -a / 3.0, 0.0];
    let v2 = [
        -(a * (2.0_f32 / 3.0).sqrt()),
        -a / 3.0,
        a * (2.0_f32 / 3.0).sqrt(),
    ];
    let v3 = [
        -(a * (2.0_f32 / 3.0).sqrt()),
        -a / 3.0,
        -a * (2.0_f32 / 3.0).sqrt(),
    ];

    let faces = [[v0, v1, v2], [v0, v2, v3], [v0, v3, v1], [v1, v3, v2]];
    let mut pos: Vec<f32> = Vec::new();
    let mut nor: Vec<f32> = Vec::new();
    let mut uvs: Vec<f32> = Vec::new();
    let mut idx: Vec<u32> = Vec::new();

    for face in &faces {
        let base = (pos.len() / 3) as u32;
        let [a, b, c] = face;
        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = normalize(cross(ab, ac));
        for v in face {
            pos.extend_from_slice(v);
            nor.extend_from_slice(&n);
        }
        uvs.extend_from_slice(&[0.5, 1., 0., 0., 1., 0.]);
        idx.extend_from_slice(&[base, base + 1, base + 2]);
    }

    MeshData { pos, nor, uvs, idx }
}

// ── Output type and helpers ───────────────────────────────────────────────────

pub struct MeshData {
    pub pos: Vec<f32>, // flat [x,y,z, ...]
    pub nor: Vec<f32>, // flat [x,y,z, ...]
    pub uvs: Vec<f32>, // flat [u,v, ...]
    pub idx: Vec<u32>,
}

impl MeshData {
    pub fn vertex_count(&self) -> usize {
        self.pos.len() / 3
    }

    pub fn position_min_max(&self) -> ([f32; 3], [f32; 3]) {
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for chunk in self.pos.chunks_exact(3) {
            for i in 0..3 {
                mn[i] = mn[i].min(chunk[i]);
                mx[i] = mx[i].max(chunk[i]);
            }
        }
        (mn, mx)
    }
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-8 {
        return [0., 1., 0.];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}
