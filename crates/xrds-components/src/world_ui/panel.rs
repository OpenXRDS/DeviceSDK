use crate::{default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject};

/// A world-space UI panel — a flat, interactive surface anchored at a world transform.
///
/// This is the root container for world-space widgets (buttons, labels, sliders, etc.).
/// Unlike the HUD system, a `XrdsWorldPanel` exists at a fixed position in the 3D scene
/// and is interacted with by pointing an XR controller ray at it (diegetic UI).
///
/// Spawn via `api.spawn(&panel)` or `api.spawn_world_panel(params)`.
/// The runtime automatically attaches [`XrdsWorldSurface`] so the pointer system
/// picks it up without any extra setup.
///
/// [`XrdsWorldSurface`]: crate::world_ui::XrdsWorldSurface
#[derive(Debug, Clone)]
pub struct XrdsWorldPanel {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    /// Panel dimensions in metres [width, height].
    pub size: [f32; 2],
    /// Background RGBA in 0–1 range.
    pub color: [f32; 4],
    /// Corner radius in metres. Reserved for future shader-based rounding; 0.0 = sharp corners.
    pub corner_radius: f32,
    /// Overall opacity multiplier (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
}

impl XrdsWorldPanel {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            size: [0.6, 0.4],
            color: [0.08, 0.08, 0.08, 0.92],
            corner_radius: 0.02,
            opacity: 1.0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

impl Default for XrdsWorldPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl XrdsObject for XrdsWorldPanel {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsWorldPanel {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsWorldPanel {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
