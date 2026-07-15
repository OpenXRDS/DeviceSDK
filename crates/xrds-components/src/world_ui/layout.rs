use bevy::prelude::Component;

/// Layout policy attached to an [`super::panel::XrdsWorldPanel`] entity.
///
/// When present (and not [`XrdsWorldLayout::None`]), the layout system runs every frame and
/// repositions the panel's direct widget children — overwriting their `local_position` fields
/// and `Transform` components. Use [`XrdsWorldLayout::None`] (or omit the component entirely)
/// to keep fully manual per-widget positioning.
///
/// # Coordinate convention
/// All positions are in **panel-local metres** with the panel centre at the origin.
/// X increases rightward; Y increases upward.
#[derive(Component, Debug, Clone)]
pub enum XrdsWorldLayout {
    /// No automatic layout — widgets keep the `local_position` set at spawn time.
    None,
    /// Stack widgets **top-to-bottom**, horizontally centred.
    VStack { gap: f32 },
    /// Stack widgets **left-to-right**, vertically centred.
    HStack { gap: f32 },
    /// Arrange widgets in a `cols`-wide grid.
    /// `gap` is `[x_gap, y_gap]` in metres.
    Grid { cols: usize, gap: [f32; 2] },
}

impl Default for XrdsWorldLayout {
    fn default() -> Self {
        Self::None
    }
}

impl XrdsWorldLayout {
    pub fn vstack(gap: f32) -> Self {
        Self::VStack { gap }
    }
    pub fn hstack(gap: f32) -> Self {
        Self::HStack { gap }
    }
    pub fn grid(cols: usize, gap: f32) -> Self {
        Self::Grid { cols, gap: [gap, gap] }
    }
}
