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
    // Initialize GTK before wgpu creates its X11 surface.
    // If GTK is initialized later (e.g. inside the Bevy event loop), it changes X11
    // display properties that invalidate the already-created wgpu swap chain surface.
    #[cfg(target_os = "linux")]
    gtk::init().expect("gtk::init failed — cannot start editor on Linux");

    let bridge = Arc::new(EditorBridge::new());
    run_bevy_viewport(bridge);
    // After the Bevy event loop ends: bypass all destructors to prevent webkit2gtk/GDK
    // from segfaulting during teardown (Linux) or WebView2 COM from deadlocking (Windows).
    #[cfg(target_os = "linux")]
    // SAFETY: intentional immediate termination; OS reclaims all resources.
    unsafe { libc::_exit(0); }
    #[cfg(not(target_os = "linux"))]
    std::process::exit(0);
}
