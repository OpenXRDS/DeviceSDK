/// SVG icon loading, rasterization, and caching for the editor UI.
///
/// Icons are loaded from `assets/icons/` relative to `CARGO_MANIFEST_DIR`,
/// rasterized with `resvg`, and cached as egui `TextureHandle`s.

use xrds::editor::egui;

// ── Icon name enum ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconName {
    // Primitives
    Empty, Cube, Sphere, Cylinder, Plane, Tetrahedron,
    // Scene objects
    Camera, PointLight, SpotLight, DirectionalLight, AudioClip, AmbientLight, InteractionZone,
    // Asset kinds
    GltfAsset, Texture, EnvironmentMap,
    // Scene/Hierarchy
    Group,
    // Viewport
    Grid, Orthographic, Perspective, Shaded, Wireframe,
    // Workflow
    NewScene, OpenFile, SaveFile, SaveAs, Export, Import, Settings, Build, Code, Asset,
    // Lights
    AreaLight, Shadow,
    // Shaders
    ShaderGraph,
    // Toolbar
    Undo, Redo, Play, Stop,
    // Misc
    Unknown,
}

impl IconName {
    /// Relative path from `assets/icons/` root.
    pub fn svg_path(&self) -> &'static str {
        match self {
            // Primitives
            IconName::Empty => "scene&hierarchy/empty.svg",
            IconName::Cube => "primitives/cube.svg",
            IconName::Sphere => "primitives/sphere.svg",
            IconName::Cylinder => "primitives/cylinder.svg",
            IconName::Plane => "primitives/plane.svg",
            IconName::Tetrahedron => "primitives/mesh.svg",
            // Scene objects
            IconName::Camera => "viewport&camera/camera.svg",
            IconName::PointLight => "lights/point.svg",
            IconName::SpotLight => "lights/spot.svg",
            IconName::DirectionalLight => "lights/directional.svg",
            IconName::AudioClip => "workflow&assets/asset.svg",
            IconName::AmbientLight => "lights/ambient.svg",
            IconName::InteractionZone => "scene&hierarchy/layers.svg",
            // Asset kinds
            IconName::GltfAsset => "workflow&assets/asset.svg",
            IconName::Texture => "materials/texture.svg",
            IconName::EnvironmentMap => "materials/pbr.svg",
            // Scene/Hierarchy
            IconName::Group => "scene&hierarchy/group.svg",
            // Viewport
            IconName::Grid => "viewport&camera/grid.svg",
            IconName::Orthographic => "viewport&camera/orthographic.svg",
            IconName::Perspective => "viewport&camera/perspective.svg",
            IconName::Shaded => "viewport&camera/shaded.svg",
            IconName::Wireframe => "viewport&camera/wireframe.svg",
            // Workflow
            IconName::NewScene => "workflow&assets/code.svg",
            IconName::OpenFile => "workflow&assets/import.svg",
            IconName::SaveFile => "workflow&assets/asset.svg",
            IconName::SaveAs => "workflow&assets/settings.svg",
            IconName::Export => "workflow&assets/export.svg",
            IconName::Import => "workflow&assets/import.svg",
            IconName::Settings => "workflow&assets/settings.svg",
            IconName::Build => "workflow&assets/build.svg",
            IconName::Code => "workflow&assets/code.svg",
            IconName::Asset => "workflow&assets/asset.svg",
            // Lights
            IconName::AreaLight => "lights/area.svg",
            IconName::Shadow => "lights/shadow.svg",
            // Shaders
            IconName::ShaderGraph => "shaders/shader.graph.svg",
            // Toolbar
            IconName::Undo => "workflow&assets/code.svg",
            IconName::Redo => "workflow&assets/build.svg",
            IconName::Play => "workflow&assets/asset.svg",
            IconName::Stop => "workflow&assets/build.svg",
            // Misc
            IconName::Unknown => "",
        }
    }

    /// Emoji fallback for icons without an SVG icon.
    pub fn emoji_fallback(&self) -> &'static str {
        match self {
            IconName::Empty => "\u{1F4C1}",
            IconName::Cube => "\u{1F532}",
            IconName::Sphere => "\u{26EA}",
            IconName::Cylinder => "\u{1F535}",
            IconName::Plane => "\u{25AD}",
            IconName::Tetrahedron => "\u{25BE}",
            IconName::Camera => "\u{1F4F7}",
            IconName::PointLight => "\u{1F4A1}",
            IconName::SpotLight => "\u{1F526}",
            IconName::DirectionalLight => "\u{1F31E}",
            IconName::AudioClip => "\u{1F50A}",
            IconName::AmbientLight => "\u{2600}",
            IconName::InteractionZone => "\u{2B21}",
            IconName::GltfAsset => "\u{1F4C2}",
            IconName::Texture => "\u{1F5BC}",
            IconName::EnvironmentMap => "\u{1F305}",
            IconName::Group => "\u{1F4C2}",
            IconName::Grid => "\u{1F704}",
            IconName::Orthographic => "\u{25A1}",
            IconName::Perspective => "\u{25C6}",
            IconName::Shaded => "\u{1F7E0}",
            IconName::Wireframe => "\u{1F53A}",
            IconName::NewScene => "\u{1F4C5}",
            IconName::OpenFile => "\u{1F4C2}",
            IconName::SaveFile => "\u{1F4BE}",
            IconName::SaveAs => "\u{1F4BE}",
            IconName::Export => "\u{1F4E4}",
            IconName::Import => "\u{1F4E5}",
            IconName::Settings => "\u{1F527}",
            IconName::Build => "\u{1F528}",
            IconName::Code => "\u{1F4BE}",
            IconName::Asset => "\u{1F4BE}",
            IconName::AreaLight => "\u{1F3A1}",
            IconName::Shadow => "\u{1F305}",
            IconName::ShaderGraph => "\u{1F527}",
            IconName::Undo => "\u{21A9}",
            IconName::Redo => "\u{21AA}",
            IconName::Play => "\u{25B6}",
            IconName::Stop => "\u{25A0}",
            IconName::Unknown => "\u{2753}",
        }
    }
}

// ── Icon cache ────────────────────────────────────────────────────────────────

pub struct SvgIconCache {
    inner: std::sync::Mutex<IconCacheInner>,
}

struct IconCacheInner {
    loaded: std::collections::HashMap<IconName, egui::TextureHandle>,
    /// Tinted icons: (IconName, tint color, threshold) → handle
    tinted: std::collections::HashMap<(IconName, egui::Color32, u8), egui::TextureHandle>,
    /// Tinted icons with explicit target size: (IconName, target_size, tint color, threshold) → handle
    tinted_at: std::collections::HashMap<(IconName, u32, egui::Color32, u8), egui::TextureHandle>,
    /// Large icons: (IconName, logical_size, ppp×100) → handle — keyed on ppp so DPI changes invalidate the cache
    large: std::collections::HashMap<(IconName, u32, u32), egui::TextureHandle>,
    /// Large icons without tint: (IconName, logical_size, ppp×100) → handle
    large_untinted: std::collections::HashMap<(IconName, u32, u32), egui::TextureHandle>,
}

impl SvgIconCache {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(IconCacheInner {
                loaded: std::collections::HashMap::new(),
                tinted: std::collections::HashMap::new(),
                tinted_at: std::collections::HashMap::new(),
                large: std::collections::HashMap::new(),
                large_untinted: std::collections::HashMap::new(),
            }),
        }
    }

    /// Load an icon by name, returning a TextureHandle.
    /// Loads from disk on first call, caches the egui TextureHandle thereafter.
    pub fn load(&self, ctx: &egui::Context, name: IconName) -> egui::TextureHandle {
        let mut inner = self.inner.lock().unwrap();

        if let Some(tex) = inner.loaded.get(&name) {
            return tex.clone();
        }

        let path = Self::resolve_svg_path(name);
        let (w, h, pixels) = match Self::load_svg_pixels(&path) {
            Ok(result) => result,
            Err(_) => {
                let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                    ..Default::default()
                };
                let handle = ctx.load_texture(
                    format!("icon_{name:?}"),
                    empty,
                    options,
                );
                inner.loaded.insert(name, handle.clone());
                return handle;
            }
        };

        let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            ..Default::default()
        };
        let handle = ctx.load_texture(
            format!("icon_{name:?}"),
            color_image,
            options,
        );
        inner.loaded.insert(name, handle.clone());
        handle
    }

    /// Tint a rasterized icon: dark pixels → solid `tint`, light pixels → solid white.
    fn tint_icon(pixels: &[u8], tint: egui::Color32, threshold: u8) -> Vec<u8> {
        let mut out = pixels.to_vec();
        for chunk in out.chunks_exact_mut(4) {
            let a = chunk[3];
            if a == 0 {
                continue; // fully transparent → keep transparent
            }
            let lum = (0.299 * chunk[0] as f32
                     + 0.587 * chunk[1] as f32
                     + 0.114 * chunk[2] as f32) as u8;
            if lum < threshold {
                chunk[0] = tint.r();
                chunk[1] = tint.g();
                chunk[2] = tint.b();
                chunk[3] = 255;
            } else {
                chunk[0] = 255;
                chunk[1] = 255;
                chunk[2] = 255;
                chunk[3] = 255;
            }
        }
        out
    }

    /// Box-filter downsample: every source pixel that maps to a destination pixel
    /// is averaged in.  Unlike bilinear (which samples only 4 points), this
    /// preserves all source detail and produces alias-free results at any ratio.
    fn downsample_box(src: &[u8], src_w: usize, src_h: usize, dst_w: usize, dst_h: usize) -> Vec<u8> {
        let mut dst = vec![0u8; dst_w * dst_h * 4];
        let sx = src_w as f64 / dst_w as f64;
        let sy = src_h as f64 / dst_h as f64;
        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let x0 = (dx as f64 * sx) as usize;
                let y0 = (dy as f64 * sy) as usize;
                let x1 = (((dx + 1) as f64 * sx).ceil() as usize).min(src_w);
                let y1 = (((dy + 1) as f64 * sy).ceil() as usize).min(src_h);
                let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                let mut n = 0u32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        let i = (py * src_w + px) * 4;
                        r += src[i] as u32;
                        g += src[i + 1] as u32;
                        b += src[i + 2] as u32;
                        a += src[i + 3] as u32;
                        n += 1;
                    }
                }
                if n > 0 {
                    let i = (dy * dst_w + dx) * 4;
                    dst[i] = (r / n) as u8;
                    dst[i + 1] = (g / n) as u8;
                    dst[i + 2] = (b / n) as u8;
                    dst[i + 3] = (a / n) as u8;
                }
            }
        }
        dst
    }

    /// Load a tinted version of an icon for use on dark backgrounds.
    /// Dark pixels (luminance < threshold) become solid `tint`, light pixels become white.
    /// Renders at the icon's native SVG resolution.
    pub fn load_tinted(&self, ctx: &egui::Context, name: IconName, tint: egui::Color32, threshold: u8) -> egui::TextureHandle {
        let mut inner = self.inner.lock().unwrap();

        let key = (name, tint, threshold);
        if let Some(tex) = inner.tinted.get(&key) {
            return tex.clone();
        }

        let path = Self::resolve_svg_path(name);
        let (w, h, pixels) = match Self::load_svg_pixels(&path) {
            Ok(result) => result,
            Err(_) => {
                let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                    ..Default::default()
                };
                let handle = ctx.load_texture(format!("tinted_{name:?}_{:?}_{}", tint, threshold), empty, options);
                return handle;
            }
        };

        let tinted = Self::tint_icon(&pixels, tint, threshold);
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &tinted);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            ..Default::default()
        };
        let handle = ctx.load_texture(format!("tinted_{name:?}_{:?}_{}", tint, threshold), color_image, options);
        inner.tinted.insert(key, handle.clone());
        handle
    }

    /// Load a tinted icon rendered at a specific target size.
    /// `oversample` controls how many times larger the SVG render is vs `target_size`.
    /// Higher values = cleaner edges at the cost of texture memory.
    pub fn load_tinted_at(&self, ctx: &egui::Context, name: IconName, target_size: u32, oversample: u32, tint: egui::Color32, threshold: u8) -> egui::TextureHandle {
        let mut inner = self.inner.lock().unwrap();

        let key = (name, target_size, tint, threshold);
        if let Some(tex) = inner.tinted_at.get(&key) {
            return tex.clone();
        }

        let path = Self::resolve_svg_path(name);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                    ..Default::default()
                };
                let handle = ctx.load_texture(format!("tinted_{name:?}_{target_size}_{:?}_{}", tint, threshold), empty, options);
                inner.tinted_at.insert(key, handle.clone());
                return handle;
            }
        };

        let tree = match usvg::Tree::from_data(&bytes, &usvg::Options::default()) {
            Ok(t) => t,
            Err(_) => {
                let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                    ..Default::default()
                };
                let handle = ctx.load_texture(format!("tinted_{name:?}_{target_size}_{:?}_{}", tint, threshold), empty, options);
                inner.tinted_at.insert(key, handle.clone());
                return handle;
            }
        };

        // Render at `oversample`× the target size for cleaner anti-aliasing
        let native_w = tree.size().width() as f64;
        let native_h = tree.size().height() as f64;
        let scale = (target_size as f64) / native_w.max(native_h);
        let big_w = (native_w * scale * oversample as f64) as u32;
        let big_h = (native_h * scale * oversample as f64) as u32;

        let mut pixmap = match resvg::tiny_skia::Pixmap::new(big_w, big_h) {
            Some(p) => p,
            None => {
                let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
                let options = egui::TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::ClampToEdge,
                    ..Default::default()
                };
                let handle = ctx.load_texture(format!("tinted_{name:?}_{target_size}_{:?}_{}", tint, threshold), empty, options);
                inner.tinted_at.insert(key, handle.clone());
                return handle;
            }
        };

        let big_scale = scale as f32 * oversample as f32;
        let transform = usvg::Transform::from_scale(big_scale, big_scale);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // Tint at oversampled resolution and upload directly — let the GPU
        // downsample to the display size, which is far cleaner than any
        // software bilinear filter and works correctly on HiDPI displays.
        let tinted = Self::tint_icon(pixmap.data(), tint, threshold);
        let color_image = egui::ColorImage::from_rgba_unmultiplied([big_w as usize, big_h as usize], &tinted);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            mipmap_mode: Some(egui::TextureFilter::Linear),
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
        };
        let handle = ctx.load_texture(format!("tinted_{name:?}_{target_size}_{:?}_{}", tint, threshold), color_image, options);
        inner.tinted_at.insert(key, handle.clone());
        handle
    }

    /// Load a tinted icon at the given logical size.
    /// Pipeline: render SVG at 4× physical pixels → tint → box-filter downsample
    /// to physical pixels → upload with LINEAR.  The 4× oversample gives resvg
    /// sub-pixel anti-aliasing room; the box filter aggregates every source pixel
    /// so nothing is missed; the final texture is at 1:1 physical size.
    pub fn load_large(&self, ctx: &egui::Context, name: IconName, target_size: u32) -> egui::TextureHandle {
        let ppp = ctx.pixels_per_point();
        let ppp_key = (ppp * 100.0).round() as u32;
        let phys = ((target_size as f32) * ppp).ceil() as u32;

        let mut inner = self.inner.lock().unwrap();
        let key = (name, target_size, ppp_key);
        if let Some(tex) = inner.large.get(&key) {
            return tex.clone();
        }

        let fallback = |ctx: &egui::Context| {
            let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
            ctx.load_texture(format!("large_{name:?}_{target_size}_{ppp_key}"), empty, egui::TextureOptions::LINEAR)
        };

        let path = Self::resolve_svg_path(name);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => { let h = fallback(ctx); inner.large.insert(key, h.clone()); return h; }
        };
        let tree = match usvg::Tree::from_data(&bytes, &usvg::Options::default()) {
            Ok(t) => t,
            Err(_) => { let h = fallback(ctx); inner.large.insert(key, h.clone()); return h; }
        };

        let native_w = tree.size().width() as f64;
        let native_h = tree.size().height() as f64;
        // Physical-pixel dimensions (the final upload size).
        let phys_scale = (phys as f64) / native_w.max(native_h);
        let pw = (native_w * phys_scale).ceil() as u32;
        let ph = (native_h * phys_scale).ceil() as u32;
        // Render at 4× for resvg anti-aliasing detail.
        let render_scale = phys_scale * 4.0;
        let rw = (native_w * render_scale).ceil() as u32;
        let rh = (native_h * render_scale).ceil() as u32;

        let mut pixmap = match resvg::tiny_skia::Pixmap::new(rw, rh) {
            Some(p) => p,
            None => { let h = fallback(ctx); inner.large.insert(key, h.clone()); return h; }
        };
        resvg::render(&tree, usvg::Transform::from_scale(render_scale as f32, render_scale as f32), &mut pixmap.as_mut());

        // Tint at high res first so anti-aliased edge pixels participate in the
        // box filter and produce smooth alpha gradients in the final image.
        let tinted_hi = Self::tint_icon(pixmap.data(), egui::Color32::WHITE, 128);
        let small = Self::downsample_box(&tinted_hi, rw as usize, rh as usize, pw as usize, ph as usize);

        let color_image = egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &small);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        };
        let handle = ctx.load_texture(format!("large_{name:?}_{target_size}_{ppp_key}"), color_image, options);
        inner.large.insert(key, handle.clone());
        handle
    }

    /// Resolve the filesystem path for an SVG icon.
    fn resolve_svg_path(name: IconName) -> std::path::PathBuf {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let icons_root = std::path::Path::new(manifest_dir)
            .join("../../assets/icons")
            .canonicalize()
            .unwrap_or_else(|_| {
                std::path::PathBuf::from(std::path::Path::new(manifest_dir).join("../../assets/icons"))
            });
        let rel = name.svg_path();
        icons_root.join(rel)
    }

    /// Load an SVG file and rasterize it to RGBA pixels.
    fn load_svg_pixels(path: &std::path::Path) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())?;

        let (w, h) = (tree.size().width() as u32, tree.size().height() as u32);
        let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h).ok_or("failed to create pixmap")?;
        resvg::render(&tree, usvg::Transform::default(), &mut pixmap.as_mut());

        Ok((w, h, pixmap.data().to_vec()))
    }

    /// Paint an icon + text button and return the response.
    pub fn paint_button(
        &self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        name: IconName,
        _text: &str,
        selected: bool,
        icon_size: f32,
    ) -> egui::Response {
        let tex = self.load(ctx, name);
        let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(icon_size, icon_size));
        let img = egui::Image::new(sized).max_size(egui::vec2(icon_size, icon_size));
        let btn = egui::Button::new(img).min_size(egui::vec2(icon_size + 24.0, icon_size));
        ui.add(btn.selected(selected))
    }

    /// Paint an icon-only button and return the response.
    pub fn paint_icon_button(
        &self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        name: IconName,
        selected: bool,
        size: egui::Vec2,
    ) -> egui::Response {
        let tex = self.load(ctx, name);
        let sized = egui::load::SizedTexture::new(tex.id(), size);
        let img = egui::Image::new(sized).max_size(size);
        ui.add(egui::Button::new(img).selected(selected))
    }

    /// Load an untinted icon using the same 4× oversample → box-filter pipeline as load_large.
    pub fn load_large_untinted(&self, ctx: &egui::Context, name: IconName, target_size: u32) -> egui::TextureHandle {
        let ppp = ctx.pixels_per_point();
        let ppp_key = (ppp * 100.0).round() as u32;
        let phys = ((target_size as f32) * ppp).ceil() as u32;

        let mut inner = self.inner.lock().unwrap();
        let key = (name, target_size, ppp_key);
        if let Some(tex) = inner.large_untinted.get(&key) {
            return tex.clone();
        }

        let fallback = |ctx: &egui::Context| {
            let empty = egui::ColorImage::new([1, 1], vec![egui::Color32::TRANSPARENT]);
            ctx.load_texture(format!("large_untinted_{name:?}_{target_size}_{ppp_key}"), empty, egui::TextureOptions::LINEAR)
        };

        let path = Self::resolve_svg_path(name);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => { let h = fallback(ctx); inner.large_untinted.insert(key, h.clone()); return h; }
        };
        let tree = match usvg::Tree::from_data(&bytes, &usvg::Options::default()) {
            Ok(t) => t,
            Err(_) => { let h = fallback(ctx); inner.large_untinted.insert(key, h.clone()); return h; }
        };

        let native_w = tree.size().width() as f64;
        let native_h = tree.size().height() as f64;
        let phys_scale = (phys as f64) / native_w.max(native_h);
        let pw = (native_w * phys_scale).ceil() as u32;
        let ph = (native_h * phys_scale).ceil() as u32;
        let render_scale = phys_scale * 4.0;
        let rw = (native_w * render_scale).ceil() as u32;
        let rh = (native_h * render_scale).ceil() as u32;

        let mut pixmap = match resvg::tiny_skia::Pixmap::new(rw, rh) {
            Some(p) => p,
            None => { let h = fallback(ctx); inner.large_untinted.insert(key, h.clone()); return h; }
        };
        resvg::render(&tree, usvg::Transform::from_scale(render_scale as f32, render_scale as f32), &mut pixmap.as_mut());

        let small = Self::downsample_box(pixmap.data(), rw as usize, rh as usize, pw as usize, ph as usize);
        let color_image = egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], &small);
        let options = egui::TextureOptions {
            magnification: egui::TextureFilter::Linear,
            minification: egui::TextureFilter::Linear,
            wrap_mode: egui::TextureWrapMode::ClampToEdge,
            mipmap_mode: None,
        };
        let handle = ctx.load_texture(format!("large_untinted_{name:?}_{target_size}_{ppp_key}"), color_image, options);
        inner.large_untinted.insert(key, handle.clone());
        handle
    }

    /// Paint an icon as a large card icon (used in project assets tab).
    pub fn paint_large_icon(
        &self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        name: IconName,
        center: egui::Pos2,
        size: f32,
        tint: egui::Color32,
    ) {
        let tex = self.load_large_untinted(ctx, name, size as u32);
        let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));
        let uv = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0));
        painter.image(tex.id(), rect, uv, tint);
    }
}

impl Default for SvgIconCache {
    fn default() -> Self {
        Self::new()
    }
}
