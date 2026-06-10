pub mod hierarchy;
pub mod inspector;
pub mod menubar;
pub mod palette;
pub mod template_picker;
pub mod toolbar;
pub mod viewport;

pub use hierarchy::hierarchy_panel;
pub use inspector::inspector_panel;
pub use menubar::menubar_panel;
pub use palette::palette_panel;
pub use template_picker::template_picker_panel;
pub(crate) use toolbar::{start_play, stop_play};
pub use toolbar::toolbar_panel;
pub use viewport::viewport_panel;

// These are plain functions (not Bevy systems).  The single `editor_ui` system
// in main.rs obtains the egui context once and passes it to each panel function
// in the correct order: TopBottomPanel → left SidePanel → right SidePanel.
