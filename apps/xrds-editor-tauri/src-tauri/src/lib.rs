mod bevy_bridge;
mod bevy_scene;
mod bridge;
mod commands;
mod editor_state;
mod environment;
mod hierarchy;
mod inspector;
mod io;
mod palette;
mod toolbar;
mod viewport_camera;
mod viewport_gizmo;
mod viewport_gizmo_interaction;
mod viewport_player;
mod viewport_selection;

use std::sync::Arc;
use tauri::Manager;
use bridge::EditorBridge;
use bevy_scene::run_bevy_viewport;
use bevy_bridge::spawn_snapshot_emitter;
use commands::{bridge_queue_depth, load_dialog_paths, send_editor_command, show_export_app_dialog, show_export_glb_dialog, show_import_asset_dialog, show_open_dialog, show_save_dialog, SharedDialogPaths};
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let bridge = Arc::new(EditorBridge::new());
    let bevy_bridge = Arc::clone(&bridge);

    // Bevy's winit event loop runs on a background thread (run_on_any_thread=true).
    // Spawned before Tauri so Bevy's LogPlugin initializes the global logger first.
    std::thread::spawn(move || run_bevy_viewport(bevy_bridge));

    tauri::Builder::default()
        .manage(bridge)
        // SharedDialogPaths is initialised in setup() once AppHandle is available.
        .manage(SharedDialogPaths::new(Mutex::new(Default::default())))
        .invoke_handler(tauri::generate_handler![
            send_editor_command,
            bridge_queue_depth,
            show_open_dialog,
            show_save_dialog,
            show_import_asset_dialog,
            show_export_glb_dialog,
            show_export_app_dialog,
        ])
        .setup(|app| {
            // Open devtools automatically in debug builds.
            #[cfg(debug_assertions)]
            if let Some(win) = app.get_webview_window("main") {
                win.open_devtools();
            }

            // Load persisted dialog paths and store them in managed state.
            let loaded = load_dialog_paths(app.handle());
            *app.state::<SharedDialogPaths>().lock().unwrap() = loaded;

            // Start the async task that drains Bevy's outbound channel and
            // emits "editor_state" events to the webview.
            let bridge = app.state::<Arc<EditorBridge>>().inner().clone();
            spawn_snapshot_emitter(app.handle().clone(), bridge);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
