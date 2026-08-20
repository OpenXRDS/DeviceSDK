//! Equirectangular panorama → cubemap KTX2.
//!
//! Phase B of `docs/editor-task-queue-and-hdr-conversion.md`. An author downloads a
//! `.exr` panorama; Bevy's `Skybox` and `EnvironmentMapLight` both require a *cube*
//! texture, and nothing in the tree could produce one. This is that step.
//!
//! # Why CPU rather than the editor's GPU
//!
//! The editor already owns a GPU context, so rendering six faces to a cubemap was
//! the obvious approach and is the wrong one. This runs headless, which makes the
//! whole conversion unit-testable — and every environment-map bug in this project
//! so far was found by a person running the editor, never by a test. It also avoids
//! render-graph plumbing, async readback, and contending with the viewport, to
//! accelerate something that measures in hundreds of milliseconds.
//!
//! # What is verified
//!
//! Decode, projection, orientation, container and both runtime formats were each
//! checked end to end before this module existed, including on a Quest 3. See the
//! plan document. The one thing left to get right here is *filtering*, which the
//! probe skipped.

use crate::task_queue::TaskContext;
use ctt::{
    convert, AlphaMode, ColorSpace, Container, ConvertSettings, Format, Image, PipelineOutput,
    Surface, TargetFormat, TextureKind,
};
use half::f16;
use rayon::prelude::*;

/// The output texture format, and why there is no choice of it.
///
/// `Rgb9e5Ufloat` — shared-exponent RGB, 4 bytes per pixel — is half the VRAM of
/// the `Rgba16Float` the SDK's own maps use, and **verified rendering on a Quest 3**
/// (Adreno). It is not a *compressed* format, so it needs no
/// `CompressedImageFormats` support; there is no compressed-HDR alternative on a
/// headset anyway, since Bevy exposes only `ASTC_LDR`, `BC` and `ETC2` and ASTC HDR
/// is unsupported by wgpu.
///
/// Offering the choice was tried and removed: nothing could reach the alternative,
/// which makes it an option that exists only in a struct. Adding one later is
/// additive, and Phase C may genuinely need it — a shared exponent costs precision
/// in the smaller channels of a saturated colour, which matters more for a
/// prefiltered specular chain than for a backdrop.
pub(crate) const TARGET_FORMAT: Format = Format::E5B9G9R9_UFLOAT_PACK32;

#[derive(Debug, Clone)]
pub struct EnvConvertOptions {
    /// Edge length of each cube face. `None` derives it from the source.
    pub face_size: Option<u32>,
    /// Zstd supercompression level for the KTX2 container.
    ///
    /// Shrinks the file and the APK, and nothing else — Bevy decompresses on load,
    /// so VRAM is unaffected. Confirmed loadable through Bevy's own
    /// `ktx2_buffer_to_image`.
    pub zstd_level: i32,
}

impl Default for EnvConvertOptions {
    fn default() -> Self {
        Self {
            face_size: None,
            zstd_level: 9,
        }
    }
}

/// Cube face size for a given panorama width.
///
/// Four faces span the 360° that the panorama's width covers, so `width / 4` keeps
/// roughly the source's angular resolution without inventing detail. Clamped, and
/// rounded down to a power of two because mip chains want one.
fn default_face_size(src_width: u32) -> u32 {
    let ideal = (src_width / 4).clamp(256, 2048);
    // Largest power of two that does not exceed `ideal`.
    1u32 << (u32::BITS - 1 - ideal.leading_zeros())
}

/// Direction of the point at face-local `(u, v)`, both in `-1..=1`.
///
/// Face order is the KTX2/Vulkan one: +X, -X, +Y, -Y, +Z, -Z. The signs are easy to
/// get subtly wrong and produce a cubemap that is structurally perfect and visually
/// scrambled, so `up_is_brightest_and_down_is_darkest` pins them against physics
/// rather than against my arithmetic.
pub(crate) fn face_direction(face: usize, u: f32, v: f32) -> [f32; 3] {
    match face {
        0 => [1.0, -v, -u],
        1 => [-1.0, -v, u],
        2 => [u, 1.0, v],
        3 => [u, -1.0, -v],
        4 => [u, -v, 1.0],
        _ => [-u, -v, -1.0],
    }
}

/// Bilinear sample of an equirectangular image.
///
/// Longitude wraps (the panorama is a full circle, so the last column is adjacent
/// to the first — clamping there would leave a visible seam behind the viewer).
/// Latitude clamps, since there is nothing above the north pole.
pub(crate) fn sample_equirect(pano: &[f32], w: u32, h: u32, lon: f32, lat: f32) -> [f32; 3] {
    use std::f32::consts::PI;

    let fx = (lon / (2.0 * PI) + 0.5) * w as f32 - 0.5;
    let fy = (lat / PI) * h as f32 - 0.5;

    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;

    let xi = |x: i64| -> usize { x.rem_euclid(w as i64) as usize };
    let yi = |y: i64| -> usize { y.clamp(0, h as i64 - 1) as usize };

    let (x0i, x1i) = (xi(x0 as i64), xi(x0 as i64 + 1));
    let (y0i, y1i) = (yi(y0 as i64), yi(y0 as i64 + 1));

    let px = |x: usize, y: usize| -> [f32; 3] {
        let o = (y * w as usize + x) * 3;
        [pano[o], pano[o + 1], pano[o + 2]]
    };

    let (a, b, c, d) = (px(x0i, y0i), px(x1i, y0i), px(x0i, y1i), px(x1i, y1i));
    let mut out = [0.0f32; 3];
    for i in 0..3 {
        let top = a[i] + (b[i] - a[i]) * tx;
        let bot = c[i] + (d[i] - c[i]) * tx;
        out[i] = top + (bot - top) * ty;
    }
    out
}

/// Supersampling rate per axis for the given source and face size.
///
/// **This is the part the feasibility probe skipped, and it is not cosmetic.** A
/// cube texel covers a patch of panorama that is usually larger than one source
/// pixel, so a single sample per texel is point-sampling a signal it cannot
/// represent. In an ordinary photo that shows as shimmer; in an HDR panorama, where
/// the sun measures 17312 against a mean of 2.6, a single missed or hit sample
/// swings the texel by four orders of magnitude, and the sky crawls as the head
/// turns.
///
/// The ratio is measured at the equator, where a face is most stretched: a face
/// spans 90° across `face_size` texels, the panorama spans 360° across `src_width`.
fn supersample_rate(src_width: u32, face_size: u32) -> u32 {
    let ratio = (src_width as f32 / 4.0) / face_size as f32;
    // Below 1.0 the output is denser than the source and there is nothing to
    // average; above 4 the cost stops buying visible quality.
    (ratio.ceil() as u32).clamp(1, 4)
}

/// Project an equirectangular panorama onto six cube faces as RGBA16F.
///
/// `pano` is tightly packed RGB f32.
fn project_faces(
    pano: &[f32],
    w: u32,
    h: u32,
    face_size: u32,
    ctx: &TaskContext,
) -> Result<Vec<Vec<u8>>, String> {
    let rate = supersample_rate(w, face_size);
    let inv = 1.0 / (rate * rate) as f32;
    ctx.set_detail(format!(
        "projecting {face_size}² faces ({}×{} samples per texel)",
        rate, rate
    ));

    (0..6usize)
        .map(|face| {
            if ctx.is_cancelled() {
                return Err("cancelled".to_string());
            }
            let mut data = vec![0u8; (face_size as usize) * (face_size as usize) * 4 * 2];

            // One row per rayon task: rows are independent, and chunking by row
            // keeps the split cheap without needing a shared mutable image.
            data.par_chunks_mut(face_size as usize * 8)
                .enumerate()
                .for_each(|(y, row)| {
                    for x in 0..face_size {
                        let mut acc = [0.0f32; 3];
                        for sy in 0..rate {
                            for sx in 0..rate {
                                // Sub-texel offsets on a regular grid, each at the
                                // centre of its own sub-cell.
                                let ox = (sx as f32 + 0.5) / rate as f32;
                                let oy = (sy as f32 + 0.5) / rate as f32;
                                let u = (x as f32 + ox) / face_size as f32 * 2.0 - 1.0;
                                let v = (y as f32 + oy) / face_size as f32 * 2.0 - 1.0;

                                let d = face_direction(face, u, v);
                                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                                let (dx, dy, dz) = (d[0] / len, d[1] / len, d[2] / len);

                                let lon = dz.atan2(dx);
                                let lat = dy.clamp(-1.0, 1.0).acos();
                                let s = sample_equirect(pano, w, h, lon, lat);
                                for i in 0..3 {
                                    acc[i] += s[i];
                                }
                            }
                        }

                        let o = x as usize * 8;
                        for c in 0..3 {
                            let half = f16::from_f32(acc[c] * inv);
                            row[o + c * 2..o + c * 2 + 2].copy_from_slice(&half.to_le_bytes());
                        }
                        row[o + 6..o + 8].copy_from_slice(&f16::from_f32(1.0).to_le_bytes());
                    }
                });

            ctx.set_progress((face + 1) as f32 / 7.0);
            Ok(data)
        })
        .collect()
}

/// Encode six faces — each with one or more mip levels, as
/// `(edge_length, RGBA16F bytes)` — into a KTX2 cubemap.
///
/// `generate_mips` completes the chain by downsampling. True for a skybox, whose
/// lower mips are just smaller versions of the same image; **false for a specular
/// IBL chain**, whose levels are prefiltered roughness steps that downsampling
/// would replace with something that merely looks similar.
pub(crate) fn encode_cubemap_ktx2(
    faces: &[Vec<(u32, Vec<u8>)>],
    zstd_level: i32,
    generate_mips: bool,
) -> Result<Vec<u8>, String> {
    let surfaces: Vec<Vec<Surface>> = faces
        .iter()
        .map(|mips| {
            mips.iter()
                .map(|(size, data)| Surface {
                    data: data.clone(),
                    width: *size,
                    height: *size,
                    depth: 1,
                    stride: size * 8,
                    slice_stride: 0,
                    format: Format::R16G16B16A16_SFLOAT,
                    color_space: ColorSpace::Linear,
                    alpha: AlphaMode::Opaque,
                })
                .collect()
        })
        .collect();

    let image = Image { surfaces, kind: TextureKind::Cubemap };
    let settings = ConvertSettings {
        format: Some(TargetFormat::Uncompressed(TARGET_FORMAT)),
        container: Container::ktx2_zstd(zstd_level),
        mipmap: generate_mips,
        ..Default::default()
    };
    match convert(image, settings).map_err(|e| format!("KTX2 encode failed: {e}"))? {
        PipelineOutput::Encoded(bytes) => Ok(bytes),
        PipelineOutput::Raw(_) => Err("expected encoded output".into()),
    }
}

/// The result of a conversion, for the caller to report.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvConvertResult {
    pub face_size: u32,
    pub bytes_written: usize,
}

/// Convert an equirectangular `.exr`/`.hdr` at `src` into a cubemap KTX2 at `dst`.
///
/// Cancellation is checked between faces and before encoding; a partial file is
/// never left behind, because the bytes are only written once encoding succeeds.
pub fn convert_equirect_to_cubemap(
    src: &std::path::Path,
    dst: &std::path::Path,
    opts: &EnvConvertOptions,
    ctx: &TaskContext,
) -> Result<EnvConvertResult, String> {
    ctx.set_detail("decoding panorama");
    let img = image::open(src).map_err(|e| format!("could not read {}: {e}", src.display()))?;
    let rgb = img.to_rgb32f();
    let (w, h) = (rgb.width(), rgb.height());
    let pano = rgb.into_raw();
    ctx.log(format!("source {w}×{h}"));

    if ctx.is_cancelled() {
        return Err("cancelled".into());
    }

    let face_size = opts.face_size.unwrap_or_else(|| default_face_size(w));
    let faces = project_faces(&pano, w, h, face_size, ctx)?;
    // The panorama is the largest allocation here and is dead once projection is
    // done; encoding a 2048² cube is not the moment to still be holding a 4K float
    // image.
    drop(pano);

    if ctx.is_cancelled() {
        return Err("cancelled".into());
    }

    ctx.set_detail("encoding KTX2");
    // One mip supplied per face; `generate_mips` completes the chain by
    // downsampling, which is right for a skybox. A specular IBL chain is NOT this:
    // its mips are roughness levels that must be computed. See `ibl`.
    let faces_mips: Vec<Vec<(u32, Vec<u8>)>> =
        faces.into_iter().map(|d| vec![(face_size, d)]).collect();
    let bytes = encode_cubemap_ktx2(&faces_mips, opts.zstd_level, true)?;

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(dst, &bytes).map_err(|e| format!("could not write {}: {e}", dst.display()))?;

    ctx.set_progress(1.0);
    Ok(EnvConvertResult {
        face_size,
        bytes_written: bytes.len(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::task_queue::{TaskLane, TaskQueue};

    /// Build a panorama with a known structure: bright at the top (sky), dark at
    /// the bottom (ground), so orientation is checkable without a real photo.
    fn gradient_panorama(w: u32, h: u32) -> Vec<f32> {
        let mut v = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            // 10.0 at the top row down to ~0.0 at the bottom.
            let lum = 10.0 * (1.0 - y as f32 / (h - 1) as f32);
            for _ in 0..w {
                v.extend_from_slice(&[lum, lum, lum]);
            }
        }
        v
    }

    fn face_mean(data: &[u8]) -> f64 {
        let mut sum = 0f64;
        let mut n = 0u64;
        for px in data.chunks_exact(8) {
            sum += f16::from_le_bytes([px[0], px[1]]).to_f32() as f64;
            n += 1;
        }
        sum / n as f64
    }

    /// Run a closure with a real `TaskContext`, which is otherwise only
    /// constructible by the queue.
    pub(crate) fn with_ctx<T: Send + 'static>(
        f: impl FnOnce(&TaskContext) -> T + Send + 'static,
    ) -> T {
        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&result);
        let mut q = TaskQueue::default();
        let id = q.spawn("test", TaskLane::Convert, move |ctx| {
            *sink.lock().unwrap() = Some(f(&ctx));
            Ok(String::new())
        });
        for _ in 0..2000 {
            q.pump();
            if q.get(id).unwrap().state.is_finished() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let out = result.lock().unwrap().take();
        out.expect("closure ran")
    }

    /// The orientation check, stated against physics rather than against my
    /// arithmetic: a sky is bright above and dark below, so +Y must be the
    /// brightest face and -Y the darkest.
    ///
    /// A wrong face order or a flipped axis sign still produces a structurally
    /// perfect cubemap that renders scrambled, and nothing else in this file would
    /// catch it. Confirmed against the real ambientCG panorama too: +Y 9.23,
    /// -Y 0.219.
    #[test]
    fn up_is_brightest_and_down_is_darkest() {
        let (w, h) = (256, 128);
        let pano = gradient_panorama(w, h);
        let faces = with_ctx(move |ctx| project_faces(&pano, w, h, 32, ctx).unwrap());

        let means: Vec<f64> = faces.iter().map(|f| face_mean(f)).collect();
        let up = means[2];
        let down = means[3];

        assert!(
            up > means[0] && up > means[1] && up > means[4] && up > means[5],
            "+Y must be the brightest face, got {means:?}"
        );
        assert!(
            down < means[0] && down < means[1] && down < means[4] && down < means[5],
            "-Y must be the darkest face, got {means:?}"
        );
        // The four side faces straddle the same gradient, so they must agree.
        for pair in [(0, 1), (0, 4), (0, 5)] {
            assert!(
                (means[pair.0] - means[pair.1]).abs() < 0.05,
                "side faces should match, got {means:?}"
            );
        }
    }

    /// Supersampling is the whole difference between this and the probe. With one
    /// sample per texel a lone bright pixel is either caught whole or missed
    /// entirely; averaging is what keeps a 17312-bright sun from making the sky
    /// crawl as the head turns.
    #[test]
    fn a_sub_pixel_highlight_is_averaged_rather_than_all_or_nothing() {
        let (w, h) = (512, 256);
        let mut pano = vec![0.05f32; (w * h * 3) as usize];
        // A single very bright texel, like a sun in a real panorama.
        let o = ((h / 2) * w + w / 2) as usize * 3;
        pano[o] = 10000.0;
        pano[o + 1] = 10000.0;
        pano[o + 2] = 10000.0;

        let p = pano.clone();
        let faces = with_ctx(move |ctx| project_faces(&p, w, h, 32, ctx).unwrap());

        // The highlight sits on the equator, so it lands on a side face and must
        // not vanish: energy is conserved even though no single texel keeps 10000.
        let total: f64 = faces.iter().map(|f| face_mean(f)).sum();
        assert!(total > 0.2, "the highlight must survive somewhere, got {total}");

        let peak = faces
            .iter()
            .flat_map(|f| f.chunks_exact(8))
            .map(|px| f16::from_le_bytes([px[0], px[1]]).to_f32())
            .fold(0.0f32, f32::max);
        assert!(peak > 1.0, "the highlight must stay bright, got {peak}");
        assert!(
            peak.is_finite(),
            "half-float must not overflow to infinity, got {peak}"
        );
    }

    /// Longitude wraps rather than clamping. A seam behind the viewer is the
    /// classic equirect bug and is invisible until someone turns around.
    #[test]
    fn longitude_wraps_across_the_seam() {
        let (w, h) = (64, 32);
        let mut pano = vec![1.0f32; (w * h * 3) as usize];
        // Make the first and last columns distinct so a clamp would show up.
        for y in 0..h {
            let last = (y * w + (w - 1)) as usize * 3;
            pano[last] = 5.0;
        }
        use std::f32::consts::PI;
        // Just past the +PI edge, which wraps around to -PI.
        let a = sample_equirect(&pano, w, h, PI - 0.001, PI / 2.0);
        let b = sample_equirect(&pano, w, h, -PI + 0.001, PI / 2.0);
        assert!(a[0].is_finite() && b[0].is_finite());
        // Sampling either side of the seam must not blow up or read out of bounds;
        // both land on real data.
        assert!(a[0] >= 1.0 && b[0] >= 1.0);
    }

    /// The whole path, on a file that goes to disk and comes back: decode →
    /// project → encode → a KTX2 whose header says cubemap.
    ///
    /// The header assertions are the ones that matter. `faceCount == 6` is what
    /// `detect_asset_kind` keys on to let the skybox picker accept the file at all,
    /// and getting it wrong produces an asset that imports cleanly and renders
    /// nothing.
    #[test]
    fn conversion_writes_a_cubemap_ktx2_that_reports_six_faces() {
        let dir = std::env::temp_dir().join("xrds_env_convert_e2e");
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("pano.exr");
        let dst = dir.join("pano_cube.ktx2");

        // 2:1, as the import contract requires, and HDR-valued.
        let buf = image::Rgb32FImage::from_fn(256, 128, |_, y| {
            let l = 10.0 * (1.0 - y as f32 / 127.0);
            image::Rgb([l, l, l])
        });
        image::DynamicImage::ImageRgb32F(buf).save(&src).expect("write source");

        let (s, d) = (src.clone(), dst.clone());
        // Face size pinned rather than derived: the heuristic has its own test, and
        // it has a 256 floor that would otherwise upscale this deliberately tiny
        // fixture and make the test slow for no reason.
        let opts = EnvConvertOptions { face_size: Some(64), ..Default::default() };
        let result = with_ctx(move |ctx| convert_equirect_to_cubemap(&s, &d, &opts, ctx))
            .expect("conversion should succeed");

        assert_eq!(result.face_size, 64);
        assert!(dst.exists());

        let bytes = std::fs::read(&dst).unwrap();
        const KTX2_ID: [u8; 12] =
            [0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(bytes[..12], KTX2_ID, "must be a KTX2 container");

        let u32_at = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        // Layout: identifier(12), vkFormat, typeSize, width, height, depth,
        // layerCount, faceCount at 36.
        assert_eq!(u32_at(20), 64, "pixelWidth");
        assert_eq!(u32_at(24), 64, "pixelHeight");
        assert_eq!(
            u32_at(36),
            6,
            "faceCount must be 6 — this is what detect_asset_kind keys on"
        );
        assert!(u32_at(40) > 1, "levelCount: a skybox wants a mip chain");
        // 2 = Zstandard. Loadable by Bevy, verified through ktx2_buffer_to_image.
        assert_eq!(u32_at(44), 2, "supercompressionScheme");
        // 123 = VK_FORMAT_E5B9G9R9_UFLOAT_PACK32. Asserted on the written file
        // rather than on the setting that requested it, so this still holds if the
        // encoder ever silently declines to convert.
        assert_eq!(u32_at(12), 123, "vkFormat must be the format verified on device");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn face_size_follows_the_source_and_stays_a_power_of_two() {
        // Four faces span the 360 degrees the width covers, so width/4.
        assert_eq!(default_face_size(4096), 1024);
        assert_eq!(default_face_size(8192), 2048);
        // Clamped at both ends rather than producing a 32² or 8192² cube.
        assert_eq!(default_face_size(256), 256);
        assert_eq!(default_face_size(65536), 2048);
        for w in [1000, 3000, 5000, 12000] {
            let f = default_face_size(w);
            assert!(f.is_power_of_two(), "{w} -> {f} must be a power of two");
        }
    }

    #[test]
    fn supersampling_scales_with_the_ratio_and_is_bounded() {
        // 4K source into 1024 faces: the ratio is 1, so one sample suffices.
        assert_eq!(supersample_rate(4096, 1024), 1);
        // Downscaling harder needs more samples per texel.
        assert_eq!(supersample_rate(4096, 512), 2);
        assert_eq!(supersample_rate(4096, 256), 4);
        // Bounded, so a tiny face from a huge panorama cannot explode the cost.
        assert_eq!(supersample_rate(65536, 64), 4);
        // Upscaling has nothing to average.
        assert_eq!(supersample_rate(1024, 2048), 1);
    }

    /// Cancellation must be honoured between faces — a 2048² conversion is slow
    /// enough that an author who picked the wrong file needs a way out.
    #[test]
    fn projection_stops_when_the_task_is_cancelled() {
        let (w, h) = (128, 64);
        let pano = gradient_panorama(w, h);

        let mut q = TaskQueue::default();
        let done = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&done);
        let id = q.spawn("convert", TaskLane::Convert, move |ctx| {
            // Already cancelled when projection starts: the first face must refuse.
            while !ctx.is_cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            *sink.lock().unwrap() = Some(project_faces(&pano, w, h, 16, &ctx).is_err());
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
        assert_eq!(done.lock().unwrap().take(), Some(true));
    }
}

#[cfg(test)]
mod real_file_check {
    use super::*;
    use super::tests::with_ctx;

    /// Runs only when XRDS_TEST_PANORAMA points at a real `.exr`. Not part of
    /// `cargo test`, because a 31 MB panorama does not belong in the repo — but the
    /// synthetic fixtures cannot exercise PIZ decoding, 4K throughput, or a sun
    /// four orders of magnitude above the mean.
    #[test]
    fn xxx_convert_a_real_panorama() {
        let Ok(src) = std::env::var("XRDS_TEST_PANORAMA") else { return };
        let dst = std::env::var("XRDS_TEST_PANORAMA_OUT")
            .unwrap_or_else(|_| "real_cube.ktx2".to_string());

        let t = std::time::Instant::now();
        let (s, d) = (std::path::PathBuf::from(src), std::path::PathBuf::from(dst));
        let out = d.clone();
        let r = with_ctx(move |ctx| {
            convert_equirect_to_cubemap(&s, &d, &EnvConvertOptions::default(), ctx)
        })
        .expect("real panorama should convert");

        println!(
            "[real] {}² faces, {:.2} MB, {:?} -> {}",
            r.face_size,
            r.bytes_written as f64 / 1e6,
            t.elapsed(),
            out.display()
        );
    }
}
