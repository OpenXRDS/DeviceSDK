//! Image-based lighting: diffuse irradiance and a prefiltered specular chain.
//!
//! Phase C of `docs/done/editor-task-queue-and-hdr-conversion.md`. Phase B produces a
//! cubemap you can *see*; this produces the two maps that let it *light* a scene.
//! Without them a converted panorama is a backdrop — a metal sphere in front of it
//! reflects nothing.
//!
//! Split-sum approximation, per Karis (SIGGRAPH 2013): the specular integral is
//! separated into a prefiltered environment term (this) and a BRDF term (which
//! Bevy already has as a built-in LUT, so it is not generated here).
//!
//! # Conventions taken from Bevy rather than assumed
//!
//! Both were read out of `bevy_pbr`'s shader, because guessing either produces
//! lighting that looks plausible and is wrong:
//!
//! - **Mip *m* is `perceptual_roughness = m / (levels - 1)`** —
//!   `radiance_level = perceptual_roughness * f32(textureNumLevels(...) - 1u)`.
//!   Note *perceptual* roughness: Bevy squares it to get the GGX alpha, so this
//!   code must square it too rather than feeding the mip fraction in directly.
//! - **The diffuse map is sampled at mip 0 only**, so it needs exactly one level
//!   and there is no reason for it to be large.
//!
//! Handedness needs no adjustment here. `environment_map.wgsl` negates the sample
//! direction's z ("cube maps are left-handed"), and `skybox.wgsl` does exactly the
//! same — so the face convention already verified for the skybox is right for these
//! maps too.

use crate::env_convert::{face_direction, sample_equirect};
use crate::task_queue::TaskContext;
use bevy::math::Vec3;
use half::f16;
use rayon::prelude::*;
use std::f32::consts::PI;

#[derive(Debug, Clone)]
pub struct IblOptions {
    /// Edge length of the diffuse irradiance cube face.
    ///
    /// Irradiance is an extremely low-frequency signal — a cosine-weighted
    /// integral over a whole hemisphere has no detail left to carry — so 32 is the
    /// conventional size and looks identical to far larger ones. The SDK's own
    /// `diffuse.ktx2` is 1024² with a single mip, which is ~50 MB for a signal that
    /// fits in 24 KB. Do not reproduce that.
    pub diffuse_size: u32,
    /// Edge length of the specular chain's base level.
    pub specular_size: u32,
    /// Cosine-weighted samples per diffuse texel.
    pub diffuse_samples: u32,
    /// GGX samples per specular texel, for the rough levels.
    pub specular_samples: u32,
    pub zstd_level: i32,
}

impl Default for IblOptions {
    fn default() -> Self {
        Self {
            diffuse_size: 32,
            // 512 against the 1024 the SDK ships: a full chain is ~8 MB of VRAM
            // rather than ~67 MB, and the difference is only visible in a
            // mirror-smooth surface, which is rare and is what mip 0 serves.
            specular_size: 512,
            diffuse_samples: 2048,
            specular_samples: 256,
            zstd_level: 9,
        }
    }
}

// ---------------------------------------------------------------------------
// Source mip pyramid
// ---------------------------------------------------------------------------

/// The panorama at successively halved resolutions.
///
/// **This is what stops the sun becoming fireflies.** Importance sampling takes a
/// few hundred directions per texel; where the source has a feature far brighter
/// and far smaller than the sample spacing — a sun at 17312 against a mean of 2.6 —
/// whether any sample lands on it is luck, and neighbouring texels get wildly
/// different answers. Sampling instead from a mip level chosen by the sample's
/// solid angle averages that feature in *before* the luck applies.
struct EquirectPyramid {
    levels: Vec<(u32, u32, Vec<f32>)>,
}

impl EquirectPyramid {
    fn build(pano: Vec<f32>, w: u32, h: u32) -> Self {
        let mut levels = vec![(w, h, pano)];
        while {
            let (lw, lh, _) = levels.last().unwrap();
            *lw > 4 && *lh > 2
        } {
            let (pw, ph, prev) = levels.last().unwrap();
            let (nw, nh) = (pw / 2, ph / 2);
            let mut next = vec![0.0f32; (nw * nh * 3) as usize];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..3 {
                        // Box filter of the four parents.
                        let mut sum = 0.0;
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let sx = (x * 2 + dx).min(pw - 1);
                                let sy = (y * 2 + dy).min(ph - 1);
                                sum += prev[((sy * pw + sx) * 3 + c) as usize];
                            }
                        }
                        next[((y * nw + x) * 3 + c) as usize] = sum * 0.25;
                    }
                }
            }
            levels.push((nw, nh, next));
        }
        Self { levels }
    }

    /// Average solid angle covered by one texel of level 0. Used to convert a
    /// sample's solid angle into a mip level.
    fn base_texel_solid_angle(&self) -> f32 {
        let (w, h, _) = &self.levels[0];
        4.0 * PI / (*w as f32 * *h as f32)
    }

    /// Trilinear sample: bilinear within two adjacent levels, blended between.
    fn sample_lod(&self, dir: Vec3, lod: f32) -> [f32; 3] {
        let lon = dir.z.atan2(dir.x);
        let lat = dir.y.clamp(-1.0, 1.0).acos();

        let max = (self.levels.len() - 1) as f32;
        let lod = lod.clamp(0.0, max);
        let lo = lod.floor() as usize;
        let hi = (lo + 1).min(self.levels.len() - 1);
        let t = lod - lo as f32;

        let (w0, h0, d0) = &self.levels[lo];
        let a = sample_equirect(d0, *w0, *h0, lon, lat);
        if lo == hi || t == 0.0 {
            return a;
        }
        let (w1, h1, d1) = &self.levels[hi];
        let b = sample_equirect(d1, *w1, *h1, lon, lat);
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    }
}

// ---------------------------------------------------------------------------
// Sampling
// ---------------------------------------------------------------------------

/// Van der Corput radical inverse — the second dimension of the Hammersley set.
fn radical_inverse_vdc(mut bits: u32) -> f32 {
    bits = (bits << 16) | (bits >> 16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    bits as f32 * 2.328_306_4e-10
}

/// Low-discrepancy 2-D sample. Deterministic, which is why conversions are
/// reproducible and testable — random sampling would make every run differ.
fn hammersley(i: u32, n: u32) -> (f32, f32) {
    (i as f32 / n as f32, radical_inverse_vdc(i))
}

/// Orthonormal basis around `n`. The `up` choice avoids a degenerate cross product
/// when `n` is itself near the pole.
fn tangent_frame(n: Vec3) -> (Vec3, Vec3) {
    let up = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent = up.cross(n).normalize();
    (tangent, n.cross(tangent))
}

/// GGX/Trowbridge-Reitz normal distribution, for the sampling PDF.
fn d_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    a2 / (PI * d * d).max(1e-7)
}

/// A half-vector drawn from the GGX distribution for `alpha`.
fn importance_sample_ggx(xi: (f32, f32), n: Vec3, alpha: f32) -> Vec3 {
    let phi = 2.0 * PI * xi.0;
    let cos_theta = ((1.0 - xi.1) / (1.0 + (alpha * alpha - 1.0) * xi.1)).max(0.0).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();

    let (tangent, bitangent) = tangent_frame(n);
    (tangent * (sin_theta * phi.cos()) + bitangent * (sin_theta * phi.sin()) + n * cos_theta)
        .normalize()
}

/// Mip level whose texels cover roughly one sample's solid angle.
fn lod_for_pdf(pdf: f32, n_samples: u32, base_texel_sa: f32) -> f32 {
    let sa_sample = 1.0 / (n_samples as f32 * pdf).max(1e-4);
    0.5 * (sa_sample / base_texel_sa).max(1e-9).log2()
}

// ---------------------------------------------------------------------------
// Diffuse irradiance
// ---------------------------------------------------------------------------

/// Cosine-weighted irradiance in the direction `n`, divided by π.
///
/// The division is the Khronos glTF-IBL-Sampler convention, and Bevy's: the stored
/// value is multiplied by albedo directly, so it is `∫L·cosθ dω / π`. With
/// cosine-weighted sampling the PDF is `cosθ/π`, which cancels to a plain mean of
/// the sampled radiance — the estimator below is that mean, not a missing π.
fn irradiance(pyr: &EquirectPyramid, n: Vec3, samples: u32, base_texel_sa: f32) -> [f32; 3] {
    let (tangent, bitangent) = tangent_frame(n);
    let mut acc = [0.0f32; 3];

    for i in 0..samples {
        let (u1, u2) = hammersley(i, samples);
        // Malley's method: a cosine-weighted hemisphere direction.
        let r = u1.sqrt();
        let phi = 2.0 * PI * u2;
        let (x, y) = (r * phi.cos(), r * phi.sin());
        let z = (1.0 - u1).max(0.0).sqrt();
        let dir = (tangent * x + bitangent * y + n * z).normalize();

        let pdf = (z / PI).max(1e-6);
        let lod = lod_for_pdf(pdf, samples, base_texel_sa);
        let s = pyr.sample_lod(dir, lod);
        for c in 0..3 {
            acc[c] += s[c];
        }
    }

    let inv = 1.0 / samples as f32;
    [acc[0] * inv, acc[1] * inv, acc[2] * inv]
}

// ---------------------------------------------------------------------------
// Specular prefilter
// ---------------------------------------------------------------------------

/// Prefiltered radiance for `perceptual_roughness` in direction `n`.
///
/// The standard split-sum simplification assumes `N = V = R`, which loses grazing
/// stretch and is what every real-time implementation does.
fn prefiltered_radiance(
    pyr: &EquirectPyramid,
    n: Vec3,
    perceptual_roughness: f32,
    samples: u32,
    base_texel_sa: f32,
) -> [f32; 3] {
    // Mip 0 is a mirror: the integral collapses to a single lookup, and sampling it
    // hundreds of times would only blur a reflection that should stay sharp.
    if perceptual_roughness <= 0.0 {
        return pyr.sample_lod(n, 0.0);
    }

    // Bevy stores *perceptual* roughness; GGX wants alpha = perceptual².
    let alpha = perceptual_roughness * perceptual_roughness;

    let mut acc = [0.0f32; 3];
    let mut weight = 0.0f32;

    for i in 0..samples {
        let xi = hammersley(i, samples);
        let h = importance_sample_ggx(xi, n, alpha);
        // Reflect the view (== n here) about the sampled half-vector.
        let l = (h * (2.0 * n.dot(h)) - n).normalize();

        let n_dot_l = n.dot(l);
        if n_dot_l <= 0.0 {
            continue;
        }
        let n_dot_h = n.dot(h).clamp(0.0, 1.0);
        // With N == V, VdotH == NdotH, so the PDF reduces to D/4.
        let pdf = (d_ggx(n_dot_h, alpha) / 4.0).max(1e-6);
        let lod = lod_for_pdf(pdf, samples, base_texel_sa);

        let s = pyr.sample_lod(l, lod);
        for c in 0..3 {
            acc[c] += s[c] * n_dot_l;
        }
        weight += n_dot_l;
    }

    if weight <= 0.0 {
        return pyr.sample_lod(n, 0.0);
    }
    [acc[0] / weight, acc[1] / weight, acc[2] / weight]
}

// ---------------------------------------------------------------------------
// Face generation
// ---------------------------------------------------------------------------

fn write_texel(row: &mut [u8], x: usize, rgb: [f32; 3]) {
    let o = x * 8;
    for c in 0..3 {
        row[o + c * 2..o + c * 2 + 2].copy_from_slice(&f16::from_f32(rgb[c]).to_le_bytes());
    }
    row[o + 6..o + 8].copy_from_slice(&f16::from_f32(1.0).to_le_bytes());
}

/// Direction through the centre of face-local texel `(x, y)`.
fn texel_direction(face: usize, x: u32, y: u32, size: u32) -> Vec3 {
    let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
    Vec3::from_array(face_direction(face, u, v)).normalize()
}

/// The six faces of the diffuse irradiance map, one mip each.
fn build_diffuse(
    pyr: &EquirectPyramid,
    opts: &IblOptions,
    ctx: &TaskContext,
) -> Result<Vec<Vec<(u32, Vec<u8>)>>, String> {
    let size = opts.diffuse_size;
    let sa = pyr.base_texel_solid_angle();
    ctx.set_detail(format!("diffuse irradiance {size}²"));

    (0..6usize)
        .map(|face| {
            if ctx.is_cancelled() {
                return Err("cancelled".to_string());
            }
            let mut data = vec![0u8; (size as usize) * (size as usize) * 8];
            data.par_chunks_mut(size as usize * 8)
                .enumerate()
                .for_each(|(y, row)| {
                    for x in 0..size {
                        let n = texel_direction(face, x, y as u32, size);
                        write_texel(row, x as usize, irradiance(pyr, n, opts.diffuse_samples, sa));
                    }
                });
            Ok(vec![(size, data)])
        })
        .collect()
}

/// The six faces of the specular chain, each with a full set of roughness mips.
///
/// Level *m* holds `perceptual_roughness = m / (levels - 1)`, which is the mapping
/// Bevy's shader reads back.
fn build_specular(
    pyr: &EquirectPyramid,
    opts: &IblOptions,
    ctx: &TaskContext,
) -> Result<Vec<Vec<(u32, Vec<u8>)>>, String> {
    let base = opts.specular_size;
    let levels = base.trailing_zeros() + 1; // down to 1×1, as Bevy's own maps are
    let sa = pyr.base_texel_solid_angle();

    let mut faces: Vec<Vec<(u32, Vec<u8>)>> = vec![Vec::new(); 6];
    for level in 0..levels {
        let size = base >> level;
        let roughness = level as f32 / (levels - 1) as f32;
        ctx.set_detail(format!(
            "specular mip {level}/{}: {size}² at roughness {roughness:.2}",
            levels - 1
        ));

        for face in 0..6usize {
            if ctx.is_cancelled() {
                return Err("cancelled".to_string());
            }
            let mut data = vec![0u8; (size as usize) * (size as usize) * 8];
            data.par_chunks_mut(size as usize * 8)
                .enumerate()
                .for_each(|(y, row)| {
                    for x in 0..size {
                        let n = texel_direction(face, x, y as u32, size);
                        let rgb =
                            prefiltered_radiance(pyr, n, roughness, opts.specular_samples, sa);
                        write_texel(row, x as usize, rgb);
                    }
                });
            faces[face].push((size, data));
        }
        ctx.set_progress(0.3 + 0.7 * (level + 1) as f32 / levels as f32);
    }
    Ok(faces)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct IblResult {
    pub diffuse_bytes: usize,
    pub specular_bytes: usize,
    pub specular_levels: u32,
}

/// Generate both IBL maps from an equirectangular panorama.
pub fn generate_ibl_maps(
    src: &std::path::Path,
    diffuse_dst: &std::path::Path,
    specular_dst: &std::path::Path,
    opts: &IblOptions,
    ctx: &TaskContext,
) -> Result<IblResult, String> {
    ctx.set_detail("decoding panorama");
    let img = image::open(src).map_err(|e| format!("could not read {}: {e}", src.display()))?;
    let rgb = img.to_rgb32f();
    let (w, h) = (rgb.width(), rgb.height());

    ctx.set_detail("building source mip pyramid");
    let pyr = EquirectPyramid::build(rgb.into_raw(), w, h);
    ctx.log(format!("source {w}×{h}, {} mip levels", pyr.levels.len()));
    ctx.set_progress(0.1);

    let diffuse = build_diffuse(&pyr, opts, ctx)?;
    ctx.set_progress(0.3);
    let specular = build_specular(&pyr, opts, ctx)?;

    ctx.set_detail("encoding KTX2");
    let diffuse_bytes = crate::env_convert::encode_cubemap_ktx2(&diffuse, opts.zstd_level, false)?;
    // `false`: these levels are prefiltered roughness steps. Letting the encoder
    // "complete" the chain by downsampling would silently replace real integrals
    // with something that merely looks similar.
    let specular_bytes =
        crate::env_convert::encode_cubemap_ktx2(&specular, opts.zstd_level, false)?;

    for (path, bytes) in [(diffuse_dst, &diffuse_bytes), (specular_dst, &specular_bytes)] {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::write(path, bytes)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
    }

    ctx.set_progress(1.0);
    Ok(IblResult {
        diffuse_bytes: diffuse_bytes.len(),
        specular_bytes: specular_bytes.len(),
        specular_levels: opts.specular_size.trailing_zeros() + 1,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_convert::tests::with_ctx;

    fn uniform_pyramid(value: f32) -> EquirectPyramid {
        EquirectPyramid::build(vec![value; 64 * 32 * 3], 64, 32)
    }

    /// A uniform environment must integrate back to itself.
    ///
    /// This is the estimator's one exactly-known answer: irradiance over a constant
    /// L is L, and prefiltered radiance at any roughness is L. It catches a missing
    /// or spurious π, a bad PDF, and a normalisation that only happens to look
    /// right on a photograph.
    #[test]
    fn a_uniform_environment_integrates_back_to_itself() {
        let pyr = uniform_pyramid(3.0);
        let sa = pyr.base_texel_solid_angle();

        for n in [Vec3::Y, Vec3::X, Vec3::NEG_Z, Vec3::new(1.0, 1.0, 1.0).normalize()] {
            let e = irradiance(&pyr, n, 512, sa);
            assert!(
                (e[0] - 3.0).abs() < 0.05,
                "irradiance of a constant 3.0 environment should be 3.0, got {e:?}"
            );

            for roughness in [0.0, 0.25, 0.5, 1.0] {
                let r = prefiltered_radiance(&pyr, n, roughness, 256, sa);
                assert!(
                    (r[0] - 3.0).abs() < 0.05,
                    "prefiltered radiance at roughness {roughness} should be 3.0, got {r:?}"
                );
            }
        }
    }

    /// Roughness must actually blur. A prefilter that ignored it would still pass
    /// the uniform test above, and would light every material identically.
    #[test]
    fn higher_roughness_blurs_more() {
        // Bright above, dark below — a strong directional signal to smear.
        let (w, h) = (64u32, 32u32);
        let mut pano = vec![0.0f32; (w * h * 3) as usize];
        for y in 0..h {
            let v = if y < h / 2 { 10.0 } else { 0.0 };
            for x in 0..w {
                for c in 0..3 {
                    pano[((y * w + x) * 3 + c) as usize] = v;
                }
            }
        }
        let pyr = EquirectPyramid::build(pano, w, h);
        let sa = pyr.base_texel_solid_angle();

        // Sampled just above the horizon, not straight up.
        //
        // Straight up shows almost no blurring however rough the surface, and that
        // is correct rather than a bug: the lobe is weighted by `n·l`, so it stays
        // within the hemisphere around N — which here is entirely bright. Blurring
        // is only visible where the lobe straddles a boundary, so the test has to
        // stand where the boundary is.
        let n = Vec3::new(1.0, 0.27, 0.0).normalize();
        let sharp = prefiltered_radiance(&pyr, n, 0.0, 256, sa)[0];
        let mid = prefiltered_radiance(&pyr, n, 0.5, 256, sa)[0];
        let rough = prefiltered_radiance(&pyr, n, 1.0, 512, sa)[0];

        assert!(sharp > 9.0, "roughness 0 should see the bright half, got {sharp}");
        assert!(sharp > mid, "roughness 0 should be sharpest: {sharp} vs {mid}");
        assert!(mid > rough, "roughness 0.5 should exceed 1.0: {mid} vs {rough}");
        // Stated as a fraction rather than a magic number: the point is that a
        // rough surface pulls in the dark half, not the exact amount.
        assert!(
            rough < sharp * 0.8,
            "a fully rough lobe must reach across the horizon: {rough} vs {sharp}"
        );
    }

    /// Mip 0 is a mirror and must stay sharp — sampling it through the GGX loop
    /// would blur a reflection that should be exact.
    #[test]
    fn mip_zero_is_an_exact_mirror_lookup() {
        let (w, h) = (64u32, 32u32);
        let mut pano = vec![1.0f32; (w * h * 3) as usize];
        // A distinct band the mirror lookup must reproduce exactly.
        for x in 0..w {
            for c in 0..3 {
                pano[((0 * w + x) * 3 + c) as usize] = 42.0;
            }
        }
        let pyr = EquirectPyramid::build(pano, w, h);
        let sa = pyr.base_texel_solid_angle();

        let straight_up = prefiltered_radiance(&pyr, Vec3::Y, 0.0, 256, sa);
        let direct = pyr.sample_lod(Vec3::Y, 0.0);
        assert_eq!(straight_up, direct, "roughness 0 must be a plain lookup");
    }

    /// The mapping Bevy's shader reads back: `radiance_level = perceptual_roughness
    /// * (numLevels - 1)`. Getting the level count wrong shifts every roughness.
    #[test]
    fn the_specular_chain_runs_from_mirror_to_fully_rough() {
        let pyr = uniform_pyramid(1.0);
        let opts = IblOptions { specular_size: 16, specular_samples: 16, ..Default::default() };
        let faces = with_ctx(move |ctx| build_specular(&pyr, &opts, ctx)).unwrap();

        assert_eq!(faces.len(), 6);
        // 16 -> 8 -> 4 -> 2 -> 1 is five levels, and 16.trailing_zeros() + 1 == 5.
        assert_eq!(faces[0].len(), 5);
        let sizes: Vec<u32> = faces[0].iter().map(|(s, _)| *s).collect();
        assert_eq!(sizes, vec![16, 8, 4, 2, 1]);
        // First level is roughness 0 and last is 1.0, so the shader's
        // `perceptual_roughness * (levels - 1)` addresses the whole range.
        for face in &faces {
            for (size, data) in face {
                assert_eq!(data.len(), (*size as usize) * (*size as usize) * 8);
            }
        }
    }

    #[test]
    fn the_diffuse_map_has_exactly_one_level() {
        let pyr = uniform_pyramid(1.0);
        let opts = IblOptions { diffuse_size: 8, diffuse_samples: 32, ..Default::default() };
        let faces = with_ctx(move |ctx| build_diffuse(&pyr, &opts, ctx)).unwrap();

        assert_eq!(faces.len(), 6);
        for face in &faces {
            // Bevy samples the diffuse map at mip 0 and nowhere else; extra levels
            // would be bytes nothing reads.
            assert_eq!(face.len(), 1);
            assert_eq!(face[0].0, 8);
        }
    }

    /// The pyramid must reach a level small enough that a whole-hemisphere sample
    /// is averaging rather than picking, or rough levels turn to fireflies.
    #[test]
    fn the_source_pyramid_halves_down_to_a_few_texels() {
        let pyr = EquirectPyramid::build(vec![1.0; 256 * 128 * 3], 256, 128);
        let sizes: Vec<(u32, u32)> = pyr.levels.iter().map(|(w, h, _)| (*w, *h)).collect();
        assert_eq!(sizes.first(), Some(&(256, 128)));
        assert!(sizes.len() >= 6, "expected a real pyramid, got {sizes:?}");
        let (lw, lh) = *sizes.last().unwrap();
        assert!(lw <= 8 && lh <= 4, "pyramid stopped too early at {lw}×{lh}");
    }

    /// A brighter mip level must be selected for a wider sample cone. Without this
    /// the pyramid exists but is never used, and the sun aliases exactly as before.
    #[test]
    fn wider_sample_cones_select_coarser_mips() {
        let sa = 4.0 * PI / (4096.0 * 2048.0);
        // A tight lobe (large pdf) stays near the base level.
        let sharp = lod_for_pdf(10_000.0, 256, sa);
        // A broad one (small pdf) must reach much further up the pyramid.
        let broad = lod_for_pdf(0.5, 256, sa);
        assert!(broad > sharp, "broad cone {broad} should exceed sharp {sharp}");
        assert!(broad > 4.0, "a near-uniform lobe should be well up the pyramid: {broad}");
    }

    #[test]
    fn cancellation_stops_both_passes() {
        let pyr = uniform_pyramid(1.0);
        let opts = IblOptions { diffuse_size: 8, specular_size: 8, ..Default::default() };

        let mut q = crate::task_queue::TaskQueue::default();
        let out = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&out);
        let id = q.spawn("ibl", crate::task_queue::TaskLane::Convert, move |ctx| {
            while !ctx.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            *sink.lock().unwrap() = Some((
                build_diffuse(&pyr, &opts, &ctx).is_err(),
                build_specular(&pyr, &opts, &ctx).is_err(),
            ));
            Ok(String::new())
        });
        q.pump();
        q.cancel(id);
        for _ in 0..2000 {
            q.pump();
            if q.get(id).unwrap().state.is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(out.lock().unwrap().take(), Some((true, true)));
    }
}

#[cfg(test)]
mod real_file_check {
    use super::*;
    use crate::env_convert::tests::with_ctx;

    /// Runs only when XRDS_TEST_PANORAMA points at a real `.exr` — for example
    /// `assets/environment_maps/DayEnvironmentHDRI043_4K_HDR.exr`.
    ///
    /// The synthetic fixtures cannot exercise a sun four orders of magnitude above
    /// the mean, which is exactly the input that makes prefiltering hard.
    #[test]
    fn xxx_prefilter_a_real_panorama() {
        let Ok(src) = std::env::var("XRDS_TEST_PANORAMA") else { return };
        let dir = std::env::var("XRDS_TEST_PANORAMA_OUT_DIR").unwrap_or_else(|_| ".".into());
        let d = std::path::PathBuf::from(&dir).join("real_diffuse.ktx2");
        let s = std::path::PathBuf::from(&dir).join("real_specular.ktx2");

        let t = std::time::Instant::now();
        let (src, dd, ss) = (std::path::PathBuf::from(src), d.clone(), s.clone());
        let r = with_ctx(move |ctx| {
            generate_ibl_maps(&src, &dd, &ss, &IblOptions::default(), ctx)
        })
        .expect("real panorama should prefilter");

        println!(
            "[real-ibl] diffuse {:.2} MB, specular {:.2} MB across {} levels, {:?}",
            r.diffuse_bytes as f64 / 1e6,
            r.specular_bytes as f64 / 1e6,
            r.specular_levels,
            t.elapsed()
        );
    }
}
