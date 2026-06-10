use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use crate::bridge::{EditorBridge, EditorCommand};

// ---------------------------------------------------------------------------
// Per-dialog path memory
// ---------------------------------------------------------------------------

/// Remembers the last-used directory for each dialog type so each dialog
/// reopens where the user left it, independently of other dialogs.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DialogPaths(HashMap<String, String>);

impl DialogPaths {
    fn get(&self, key: &str) -> Option<PathBuf> {
        self.0.get(key).map(PathBuf::from)
    }
    /// Remember the directory containing `file`.
    fn remember_file(&mut self, key: &str, file: &Path) {
        if let Some(dir) = file.parent() {
            self.0.insert(key.to_string(), dir.to_string_lossy().into_owned());
        }
    }
    /// Remember `dir` directly (for folder-picker dialogs).
    fn remember_dir(&mut self, key: &str, dir: &Path) {
        self.0.insert(key.to_string(), dir.to_string_lossy().into_owned());
    }
}

pub type SharedDialogPaths = Arc<Mutex<DialogPaths>>;

// ---------------------------------------------------------------------------
// Persistence helpers (called from lib.rs setup + each dialog command)
// ---------------------------------------------------------------------------

fn paths_file(handle: &tauri::AppHandle) -> Option<PathBuf> {
    handle.path().app_data_dir().ok().map(|d| d.join("dialog_paths.json"))
}

pub fn load_dialog_paths(handle: &tauri::AppHandle) -> DialogPaths {
    paths_file(handle)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_dialog_paths(handle: &tauri::AppHandle, paths: &DialogPaths) {
    if let Some(file) = paths_file(handle) {
        if let Some(dir) = file.parent() { let _ = std::fs::create_dir_all(dir); }
        if let Ok(json) = serde_json::to_string(paths) {
            let _ = std::fs::write(file, json);
        }
    }
}

// ---------------------------------------------------------------------------
// Bridge command
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn send_editor_command(
    state: tauri::State<Arc<EditorBridge>>,
    command: EditorCommand,
) {
    state.inbound.lock().unwrap().push_back(command);
}

#[tauri::command]
pub fn bridge_queue_depth(state: tauri::State<Arc<EditorBridge>>) -> usize {
    state.inbound.lock().unwrap().len()
}

// ---------------------------------------------------------------------------
// File dialogs — each remembers its own last-used path
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn show_open_dialog(
    app: tauri::AppHandle,
    dialog_paths: tauri::State<'_, SharedDialogPaths>,
) -> Result<Option<String>, String> {
    let start_dir = dialog_paths.lock().unwrap().get("open_scene");
    let mut dlg = rfd::AsyncFileDialog::new()
        .set_title("Open Scene")
        .add_filter("XRDS Scene", &["json"])
        .add_filter("All Files", &["*"]);
    if let Some(dir) = start_dir { dlg = dlg.set_directory(dir); }
    let Some(file) = dlg.pick_file().await else { return Ok(None); };
    let path = file.path().to_path_buf();
    { let mut p = dialog_paths.lock().unwrap(); p.remember_file("open_scene", &path); save_dialog_paths(&app, &p); }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn show_import_asset_dialog(
    app: tauri::AppHandle,
    dialog_paths: tauri::State<'_, SharedDialogPaths>,
) -> Result<Option<String>, String> {
    let start_dir = dialog_paths.lock().unwrap().get("import_asset");
    let mut dlg = rfd::AsyncFileDialog::new()
        .set_title("Import Asset")
        .add_filter("All Assets",  &["glb","gltf","png","jpg","jpeg","webp","ktx2","mp3","wav","ogg","flac","hdr"])
        .add_filter("3D Models",   &["glb","gltf"])
        .add_filter("Textures",    &["png","jpg","jpeg","webp","ktx2"])
        .add_filter("Audio",       &["mp3","wav","ogg","flac"])
        .add_filter("Environment", &["hdr","ktx2"])
        .add_filter("All Files",   &["*"]);
    if let Some(dir) = start_dir { dlg = dlg.set_directory(dir); }
    let Some(file) = dlg.pick_file().await else { return Ok(None); };
    let path = file.path().to_path_buf();
    { let mut p = dialog_paths.lock().unwrap(); p.remember_file("import_asset", &path); save_dialog_paths(&app, &p); }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn show_export_app_dialog(
    app: tauri::AppHandle,
    dialog_paths: tauri::State<'_, SharedDialogPaths>,
) -> Result<Option<String>, String> {
    let start_dir = dialog_paths.lock().unwrap().get("export_app");
    let mut dlg = rfd::AsyncFileDialog::new()
        .set_title("Export Application — choose output folder");
    if let Some(dir) = start_dir { dlg = dlg.set_directory(dir); }
    let Some(folder) = dlg.pick_folder().await else { return Ok(None); };
    let path = folder.path().to_path_buf();
    { let mut p = dialog_paths.lock().unwrap(); p.remember_dir("export_app", &path); save_dialog_paths(&app, &p); }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn show_export_glb_dialog(
    app: tauri::AppHandle,
    dialog_paths: tauri::State<'_, SharedDialogPaths>,
    scene_name: String,
) -> Result<Option<String>, String> {
    let default_name = if scene_name.is_empty() || scene_name == "Untitled Scene" {
        "scene.glb".to_string()
    } else {
        format!("{}.glb", scene_name.replace(' ', "_"))
    };
    let start_dir = dialog_paths.lock().unwrap().get("export_glb");
    let mut dlg = rfd::AsyncFileDialog::new()
        .set_title("Export GLB")
        .set_file_name(&default_name)
        .add_filter("GLB", &["glb"]);
    if let Some(dir) = start_dir { dlg = dlg.set_directory(dir); }
    let Some(file) = dlg.save_file().await else { return Ok(None); };
    let path = file.path().to_path_buf();
    { let mut p = dialog_paths.lock().unwrap(); p.remember_file("export_glb", &path); save_dialog_paths(&app, &p); }
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn show_save_dialog(
    app: tauri::AppHandle,
    dialog_paths: tauri::State<'_, SharedDialogPaths>,
    current_name: String,
) -> Result<Option<String>, String> {
    let start_dir = dialog_paths.lock().unwrap().get("save_scene");
    let mut dlg = rfd::AsyncFileDialog::new()
        .set_title("Save Scene As")
        .set_file_name(&current_name)
        .add_filter("XRDS Scene", &["json"]);
    if let Some(dir) = start_dir { dlg = dlg.set_directory(dir); }
    let Some(file) = dlg.save_file().await else { return Ok(None); };
    let path = file.path().to_path_buf();
    { let mut p = dialog_paths.lock().unwrap(); p.remember_file("save_scene", &path); save_dialog_paths(&app, &p); }
    Ok(Some(path.to_string_lossy().into_owned()))
}
