/// One text item inside a [`XrdsHudTemplate`].
#[derive(Debug, Clone, PartialEq)]
pub struct XrdsHudItemDef {
    pub id: u64,
    /// Key used at runtime via `set_hud_item`.
    pub name: String,
    /// Canvas-local position: X right, Y up (metres).
    pub position: [f32; 2],
    pub text: String,
    pub font_size: f32,
    /// RGBA in 0-1 range.
    pub color: [f32; 4],
}

/// Authored HUD layout template. Linked to a `PlayerAnchor`; at runtime the
/// system instantiates one copy of this template per active anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct XrdsHudTemplate {
    pub id: u64,
    pub name: String,
    /// Camera-space depth in metres (positive = in front of viewer).
    pub depth: f32,
    pub items: Vec<XrdsHudItemDef>,
}
