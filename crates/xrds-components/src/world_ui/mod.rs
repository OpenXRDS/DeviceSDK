/// World-space UI components.
///
/// A *world-space UI* (also called *diegetic UI* in XR design) is an interactive panel
/// anchored at a fixed position in the 3D scene. Unlike the HUD system — which follows
/// the player's view — a world panel stays in place and the player physically points at it
/// with an XR controller ray to interact.
///
/// # Core types
///
/// - [`XrdsWorldPanel`] — the root panel surface (flat quad mesh in the scene)
/// - [`XrdsWorldSurface`] — Bevy component that marks an entity as a pointer target
/// - [`XrdsWorldPointerState`] — per-frame hit state resource, one entry per hand
/// - [`XrWorldHoverEnterEvent`] / [`XrWorldHoverExitEvent`] — fired on hover transitions
///
/// # Widgets (Phase 2)
///
/// - [`XrdsWorldLabel`] — static or updatable text line inside a panel
/// - [`XrdsWorldButton`] — pressable quad with hover/pressed colour states
/// - [`XrdsWorldImage`] — textured quad inside a panel
///
/// # Compound Widgets (Phase 3)
///
/// - [`XrdsWorldSlider`] — drag-to-scrub value control
/// - [`XrdsWorldToggle`] — binary on/off flip control
///
/// # Layout (Phase 4)
///
/// - [`XrdsWorldLayout`] — attach to a panel to auto-position its child widgets
pub mod button;
pub mod image;
pub mod label;
pub mod layout;
pub mod panel;
pub mod pointer;
pub mod slider;
pub mod surface;
pub mod toggle;

pub use button::{
    XrdsWorldButton, XrdsWorldButtonParams, XrdsWorldButtonState,
    XrWorldButtonPressEvent, XrWorldButtonReleaseEvent,
};
pub use image::{XrdsWorldImage, XrdsWorldImageParams};
pub use label::{XrdsWorldLabel, XrdsWorldLabelParams};
pub use panel::XrdsWorldPanel;
pub use pointer::{
    XrdsWorldPointerCursors, XrdsWorldPointerHit, XrdsWorldPointerState,
    XrWorldHoverEnterEvent, XrWorldHoverExitEvent,
};
pub use layout::XrdsWorldLayout;
pub use slider::{XrdsWorldSlider, XrdsWorldSliderParams, XrWorldSliderChangeEvent};
pub use surface::XrdsWorldSurface;
pub use toggle::{XrdsWorldToggle, XrdsWorldToggleParams, XrWorldToggleEvent};
