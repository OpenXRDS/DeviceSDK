mod bevy_bridge;
mod bevy_scene;
mod bridge;
mod editor_state;
mod environment;
mod hierarchy;
mod hud_library;
mod inspector;
mod io;
mod keyboard_shortcuts;
mod palette;
mod toolbar;
mod viewport_camera;
mod viewport_gizmo;
mod viewport_gizmo_interaction;
mod viewport_player;
mod viewport_selection;
mod wry_overlay;

use std::sync::Arc;
use bridge::EditorBridge;
use bevy_scene::run_bevy_viewport;

pub fn run() {
    let bridge = Arc::new(EditorBridge::new());
    run_bevy_viewport(bridge);
    // WebView2 COM teardown deadlocks if the winit message loop is already gone.
    // Exiting immediately after Bevy's event loop ends avoids the hang.
    std::process::exit(0);
}
