//! Scene file save / load operations.
//!
//! The XRDS scene format is JSON with a `.xrds` extension.
//! `XrdsSceneDocumentSession` already handles serialisation; this module adds
//! the native file-dialog layer and wires up the session replacement logic.

use std::path::{Path, PathBuf};
use xrds_gltf;

use xrds::scene_graph::{
    XrdsSceneDocument, XrdsSceneDocumentSession, XrdsSceneAssetKind, XrdsSceneNodePayload,
    XRDS_SCENE_DOCUMENT_VERSION,
};

use crate::state::{EditorSession, EditorState, ExportAppPending, PendingFileDialog, PendingFileOpKind};

// Baked in at compile time: <sdk>/apps/xrds-editor
const EDITOR_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

const XRDS_EXTENSION: &str = "xrds";
const XRDS_FILTER_NAME: &str = "XRDS Scene";

// ── Helper: spawn a file-dialog thread and store a PendingFileDialog ─────────

pub(crate) fn spawn_file_dialog(
    state: &mut EditorState,
    op: PendingFileOpKind,
    builder: impl FnOnce() -> Option<PathBuf> + Send + 'static,
) {
    if state.pending_file_dialog.is_some() {
        return; // another dialog already in flight
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || { let _ = tx.send(builder()); });
    state.pending_file_dialog = Some(PendingFileDialog {
        rx: std::sync::Mutex::new(rx),
        op,
    });
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Save to the current path if known, otherwise open a Save As dialog.
pub fn save(session: &mut EditorSession, state: &mut EditorState) {
    if session.session.save_path().is_some() {
        match session.session.save() {
            Ok(_) => state.status_message = Some("Saved.".into()),
            Err(e) => state.status_message = Some(format!("Save failed: {e:?}")),
        }
    } else {
        save_as(session, state);
    }
}

/// Queue a Save As dialog; actual save happens in `XrdsEditorApp::update` when
/// the path arrives through the channel.
pub fn save_as(session: &mut EditorSession, state: &mut EditorState) {
    let dir = session.session.save_path()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf());
    let filename = session.session.save_path()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            let name = &session.document().metadata.name;
            if name.is_empty() { "untitled.xrds".to_string() } else { format!("{name}.xrds") }
        });

    spawn_file_dialog(state, PendingFileOpKind::SaveSceneAs, move || {
        let mut dialog = rfd::FileDialog::new()
            .add_filter(XRDS_FILTER_NAME, &[XRDS_EXTENSION])
            .set_title("Save Scene As")
            .set_file_name(&filename);
        if let Some(d) = dir { dialog = dialog.set_directory(d); }
        let mut path = dialog.save_file()?;
        if path.extension().is_none() { path.set_extension(XRDS_EXTENSION); }
        Some(path)
    });
}

/// Queue an Open dialog; session replacement happens in `XrdsEditorApp::update`.
pub fn load(_session: &mut EditorSession, state: &mut EditorState) {
    spawn_file_dialog(state, PendingFileOpKind::OpenScene, || {
        rfd::FileDialog::new()
            .add_filter(XRDS_FILTER_NAME, &[XRDS_EXTENSION])
            .set_title("Open Scene")
            .pick_file()
    });
}

/// Open the template picker dialog — actual scene creation happens in `apply_template`.
pub fn new_scene(_session: &mut EditorSession, state: &mut EditorState) {
    state.template_picker_selection = "empty";
    state.show_template_picker = true;
}

/// Create a new session from `doc` and reset all editor state.
pub fn apply_template(session: &mut EditorSession, state: &mut EditorState, doc: XrdsSceneDocument) {
    match XrdsSceneDocumentSession::new(doc) {
        Ok(new_session) => {
            session.session = new_session;
            state.selection.clear();
            state.editing_name = None;
            state.clear_pending_translations();
            state.clear_pending_rotations();
            state.pending_scale = None;
            state.pending_material = None;
            state.pending_visible = None;
            state.gizmo_drag = None;
            state.needs_runtime_sync = true;
            state.needs_full_reimport = true;
            state.status_message = Some("New scene.".into());
        }
        Err(e) => {
            state.status_message = Some(format!("New scene failed: {e:?}"));
        }
    }
}

// ── Asset import ─────────────────────────────────────────────────────────────

/// Queue an asset-import file picker; actual registration happens in
/// `XrdsEditorApp::update` when the path arrives through the channel.
pub fn import_asset(_session: &mut EditorSession, state: &mut EditorState) {
    spawn_file_dialog(state, PendingFileOpKind::ImportAsset, || {
        rfd::FileDialog::new()
            .add_filter("All Supported", &[
                "gltf","glb",
                "png","jpg","jpeg","ktx2","dds",
                "exr","hdr",
                "mp3","ogg","wav","flac",
            ])
            .add_filter("glTF / GLB",       &["gltf","glb"])
            .add_filter("Textures",         &["png","jpg","jpeg","ktx2","dds"])
            .add_filter("Environment Maps", &["exr","hdr","ktx2","dds"])
            .add_filter("Audio",            &["mp3","ogg","wav","flac"])
            .set_title("Import Asset")
            .pick_file()
    });
}

/// Detect asset kind from a file extension (pub(crate) for use in update()).
pub(crate) fn detect_kind(ext: &str) -> XrdsSceneAssetKind {
    match ext {
        "gltf" | "glb"                   => XrdsSceneAssetKind::Gltf,
        "exr"  | "hdr"                   => XrdsSceneAssetKind::EnvironmentMap,
        "mp3"  | "ogg" | "wav" | "flac"  => XrdsSceneAssetKind::Audio,
        _                                => XrdsSceneAssetKind::Texture,
    }
}

/// Make `target` relative to `scene_file`'s parent directory if possible.
/// Returns an absolute path string otherwise.
pub(crate) fn scene_relative_uri(target: &Path, scene_file: Option<&Path>) -> String {
    let Some(scene_dir) = scene_file.and_then(|p| p.parent()) else {
        // No save path — absolute URI.  Normalize to forward slashes so that
        // Bevy's AssetServer can load it on Windows (backslashes break loads).
        return target.to_string_lossy().into_owned().replace('\\', "/");
    };

    // Try to compute a relative path.
    if let Some(rel) = relative_from(target, scene_dir) {
        return rel.to_string_lossy().replace('\\', "/");
    }

    target.to_string_lossy().into_owned()
}

/// Compute the path of `target` relative to `base`, without external crates.
fn relative_from(target: &Path, base: &Path) -> Option<PathBuf> {
    let target = target.canonicalize().ok()?;
    let base   = base.canonicalize().ok()?;

    let mut target_parts = target.components().peekable();
    let mut base_parts   = base.components().peekable();

    // Skip the common prefix.
    while target_parts.peek() == base_parts.peek() {
        if target_parts.peek().is_none() { break; }
        target_parts.next();
        base_parts.next();
    }

    // Climb out of the remaining base directories.
    let mut rel = PathBuf::new();
    for _ in base_parts { rel.push(".."); }
    for part in target_parts { rel.push(part); }

    if rel.as_os_str().is_empty() { None } else { Some(rel) }
}

/// Queue an export-selection dialog; processing happens in `XrdsEditorApp::update`.
///
/// For `GltfAsset` nodes the original file is copied directly (preserving all
/// geometry and textures); for primitive/light/camera nodes the xrds-gltf
/// exporter builds a GLB from the subtree document.
pub fn export_glb_selection(session: &EditorSession, state: &mut EditorState) {
    let Some(selected_id) = state.selection.primary() else {
        state.status_message = Some("Nothing selected — select a node first.".into());
        return;
    };
    if state.pending_file_dialog.is_some() {
        return;
    }

    let doc = session.document();
    let Some(node) = doc.node(selected_id) else {
        state.status_message = Some("Selected node not found in document.".into());
        return;
    };

    if let XrdsSceneNodePayload::GltfAsset(gltf) = &node.payload {
        let source = resolve_asset_uri(&gltf.asset_uri, session.session.save_path());
        let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("glb").to_owned();
        let default_name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.{ext}", node.name));
        let op = PendingFileOpKind::ExportGlbSelectionCopy { source };
        spawn_file_dialog(state, op, move || {
            rfd::FileDialog::new()
                .add_filter("glTF / GLB", &["gltf", "glb"])
                .set_title("Export glTF Asset")
                .set_file_name(default_name)
                .save_file()
        });
        return;
    }

    let node_name = node.name.clone();
    let Some(_subtree) = doc.subtree_document(selected_id) else {
        state.status_message = Some("Could not build subtree for export.".into());
        return;
    };
    let op = PendingFileOpKind::ExportGlbSelectionExport { node_id: selected_id };
    spawn_file_dialog(state, op, move || {
        rfd::FileDialog::new()
            .add_filter("Binary glTF", &["glb"])
            .set_title("Export Selected as GLB")
            .set_file_name(format!("{node_name}.glb"))
            .save_file()
    });
}

/// Resolve an asset URI to an absolute `PathBuf`.
/// Relative URIs are resolved against the scene file's directory.
fn resolve_asset_uri(uri: &str, save_path: Option<&Path>) -> PathBuf {
    let p = Path::new(uri);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    if let Some(scene_dir) = save_path.and_then(|sp| sp.parent()) {
        return scene_dir.join(p);
    }
    p.to_path_buf()
}

/// Queue an export-GLB save dialog; processing happens in `XrdsEditorApp::update`.
pub fn export_glb(session: &EditorSession, state: &mut EditorState) {
    let name = session.document().metadata.name.clone();
    let default_filename = if name.is_empty() { "scene.glb".to_string() } else { format!("{name}.glb") };
    spawn_file_dialog(state, PendingFileOpKind::ExportGlb, move || {
        rfd::FileDialog::new()
            .add_filter("Binary glTF", &["glb"])
            .set_title("Export GLB")
            .set_file_name(default_filename)
            .save_file()
    });
}

// ── Export as Application ─────────────────────────────────────────────────────

/// Export the current scene as a standalone Rust application project.
///
/// Generates:
///   <out_dir>/
///     Cargo.toml       — runner crate with path dep → SDK
///     src/main.rs      — calls import_scene_document_json at startup
///     assets/scene.xrds — scene document with relativised asset URIs
///     assets/...       — all referenced asset files copied in-place
///
/// After export the user runs `cargo build --release` inside the output folder.
///
/// The folder picker runs on a background thread via a channel to avoid a
/// `dispatch_sync` deadlock on macOS when called from a Bevy ECS system that
/// runs on a thread-pool thread (not the main thread).
pub fn export_app(session: &EditorSession, state: &mut EditorState) {
    if state.export_app_pending.is_some() {
        return; // already waiting for a folder pick
    }

    let doc = session.document().clone();
    let save_path = session.session.save_path().map(|p| p.to_path_buf());

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = rfd::FileDialog::new()
            .set_title("Export as Application — Choose Output Folder")
            .pick_folder();
        let _ = tx.send(result);
    });

    state.export_app_pending = Some(ExportAppPending { rx: std::sync::Mutex::new(rx), doc, save_path });
    state.status_message = Some("Choose an output folder…".into());
}

pub(crate) fn do_export_app(
    doc: &XrdsSceneDocument,
    save_path: Option<&Path>,
    out_dir: &Path,
) -> Result<(), String> {
    let assets_dir = out_dir.join("assets");
    let src_dir = out_dir.join("src");

    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("Cannot create assets/: {e}"))?;
    std::fs::create_dir_all(&src_dir)
        .map_err(|e| format!("Cannot create src/: {e}"))?;

    // Copy every catalog asset into assets/ and rewrite URIs to be relative
    // to the assets/ directory (Bevy's AssetServer root).
    let mut export_doc = doc.clone();
    // Older scenes may have been deserialized with version=0 (missing field).
    // Stamp the current version so save_json doesn't reject the document.
    export_doc.version = XRDS_SCENE_DOCUMENT_VERSION;
    for asset in &mut export_doc.assets {
        let abs = resolve_asset_uri(&asset.uri, save_path);

        let new_uri = if !Path::new(&asset.uri).is_absolute() {
            // Preserve existing subdirectory structure inside assets/.
            asset.uri.replace('\\', "/")
        } else {
            // Absolute path: flatten to bare filename.
            abs.file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "asset".to_string())
        };

        let dest = assets_dir.join(new_uri.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create asset subdir: {e}"))?;
        }

        if abs.exists() {
            std::fs::copy(&abs, &dest)
                .map_err(|e| format!("Cannot copy '{}': {e}", abs.display()))?;
        }

        asset.uri = new_uri;
    }

    // Rewrite node payload asset URIs to match the relativised catalog.
    //
    // GltfAsset payloads store a fallback `asset_uri` alongside the catalog
    // `asset_id`. A save on the authoring machine leaves the machine-local
    // absolute path there; if it leaked into the export, any consumer that hits
    // the fallback path (missing/renamed catalog entry) would resolve a path
    // that only exists on the authoring PC. Point the fallback at the same
    // relative URI as the catalog entry, or flatten it like catalog URIs when
    // no catalog entry matches.
    let catalog: std::collections::HashMap<String, String> = export_doc
        .assets
        .iter()
        .map(|a| (a.id.clone(), a.uri.clone()))
        .collect();
    for node in &mut export_doc.nodes {
        if let XrdsSceneNodePayload::GltfAsset(gltf) = &mut node.payload {
            if let Some(uri) = gltf.asset_id.as_ref().and_then(|id| catalog.get(id)) {
                gltf.asset_uri = uri.clone();
            } else if Path::new(&gltf.asset_uri).is_absolute() {
                let abs = resolve_asset_uri(&gltf.asset_uri, save_path);
                let new_uri = abs
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "asset".to_string());
                let dest = assets_dir.join(&new_uri);
                if abs.exists() && !dest.exists() {
                    std::fs::copy(&abs, &dest)
                        .map_err(|e| format!("Cannot copy '{}': {e}", abs.display()))?;
                }
                gltf.asset_uri = new_uri;
            }
        }
    }

    // Write the re-patched scene document.
    export_doc
        .save_json(assets_dir.join("scene.xrds"))
        .map_err(|e| format!("Cannot write scene.xrds: {e:?}"))?;

    // Derive SDK root from compile-time manifest dir of this editor crate.
    // EDITOR_MANIFEST_DIR = <sdk>/apps/xrds-editor → two parents up = <sdk>
    let sdk_root = Path::new(EDITOR_MANIFEST_DIR)
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("."));
    let sdk_root_str = sdk_root.to_string_lossy().replace('\\', "/");

    // Copy bundled SDK fonts — required by bevy_rich_text3d at runtime.
    let src_fonts = sdk_root.join("assets").join("fonts");
    let dst_fonts = assets_dir.join("fonts");
    if src_fonts.is_dir() {
        std::fs::create_dir_all(&dst_fonts)
            .map_err(|e| format!("Cannot create assets/fonts/: {e}"))?;
        for entry in std::fs::read_dir(&src_fonts)
            .map_err(|e| format!("Cannot read fonts dir: {e}"))?
        {
            let entry = entry.map_err(|e| format!("Font dir entry error: {e}"))?;
            let dest = dst_fonts.join(entry.file_name());
            std::fs::copy(entry.path(), &dest)
                .map_err(|e| format!("Cannot copy font '{}': {e}", entry.path().display()))?;
        }
    }

    // Sanitise scene name into a valid Cargo package name.
    let app_name = sanitize_package_name(&doc.metadata.name).unwrap_or_else(|| {
        out_dir
            .file_name()
            .map(|n| sanitize_package_name(&n.to_string_lossy()).unwrap_or_else(|| "xrds-app".into()))
            .unwrap_or_else(|| "xrds-app".into())
    });

    let display_name = if doc.metadata.name.is_empty() {
        "XRDS App".to_string()
    } else {
        doc.metadata.name.clone()
    };

    // --- Cargo.toml ---
    let cargo_toml = format!(
        r#"[package]
name = "{app_name}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{app_name}"
path = "src/main.rs"

[dependencies]
xrds = {{ path = "{sdk_root_str}" }}
bevy = {{ version = "0.17.2", features = ["jpeg", "mp3", "wav"] }}
"#
    );
    std::fs::write(out_dir.join("Cargo.toml"), cargo_toml)
        .map_err(|e| format!("Cannot write Cargo.toml: {e}"))?;

    // --- src/main.rs ---
    let main_rs = format!(
        r#"use xrds::{{Runtime, RuntimeParameters}};
use xrds::viewer::XrdsSceneViewer;

fn main() {{
    // Use assets/ next to the exe (distribution layout).
    // Fall back to assets/ in the working directory (cargo run from project root).
    let exe_assets = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("assets")));

    let assets_dir = exe_assets
        .filter(|p| p.exists())
        .unwrap_or_else(|| std::path::PathBuf::from("assets"));

    let asset_path = Some(assets_dir.to_string_lossy().into_owned());
    let scene_path = assets_dir.join("scene.xrds").to_string_lossy().into_owned();

    Runtime::new(RuntimeParameters {{
        app_name: "{display_name}".to_owned(),
        enable_xr: false, // set to true to enable OpenXR / HMD output
        asset_path,
        ..Default::default()
    }})
    .run_xrds(XrdsSceneViewer::new(scene_path))
    .expect("Could not run application");
}}
"#
    );
    std::fs::write(src_dir.join("main.rs"), main_rs)
        .map_err(|e| format!("Cannot write main.rs: {e}"))?;

    Ok(())
}

/// Sanitise a string into a valid Cargo package name (lowercase, hyphens only).
/// Returns `None` if the result is empty after stripping invalid characters.
fn sanitize_package_name(s: &str) -> Option<String> {
    let name: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    let name = name.trim_matches('-').to_string();
    // Collapse consecutive hyphens.
    let name = name
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if name.is_empty() { None } else { Some(name) }
}

/// Recursively copy every file from `src` into `dst`, creating directories as needed.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("Cannot create '{}': {e}", dst.display()))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("Cannot read '{}': {e}", src.display()))?
    {
        let entry = entry.map_err(|e| format!("Read dir entry: {e}"))?;
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type().map_err(|e| format!("File type: {e}"))?;
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)
                .map_err(|e| format!("Cannot copy '{}': {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Open the given directory in the OS file explorer (best-effort).
pub fn reveal_in_explorer(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    std::process::Command::new("explorer").arg(path).spawn()?;
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(path).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(path).spawn()?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Helper: create a temp directory tree and return its path.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xrds_io_test_{name}"));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn scene_relative_uri_returns_forward_slash_relative_path() {
        let base = temp_dir("rel_uri");
        let scene_file = base.join("scenes").join("my_scene.xrds");
        let asset = base.join("assets").join("models").join("cube.glb");

        // Create dirs so canonicalize succeeds.
        fs::create_dir_all(scene_file.parent().unwrap()).unwrap();
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&asset, b"fake").unwrap();

        let uri = scene_relative_uri(&asset, Some(&scene_file));
        // Should be relative, using only forward slashes.
        assert!(!uri.contains('\\'), "URI must not contain backslashes: {uri}");
        assert!(!Path::new(&uri).is_absolute(), "URI should be relative: {uri}");
        assert!(uri.contains("cube.glb"), "URI should contain the filename: {uri}");
    }

    #[test]
    fn scene_relative_uri_without_save_path_returns_absolute_with_forward_slashes() {
        let base = temp_dir("abs_uri");
        let asset = base.join("some").join("model.glb");
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&asset, b"fake").unwrap();

        let uri = scene_relative_uri(&asset, None);
        assert!(!uri.contains('\\'), "Absolute URI must use forward slashes: {uri}");
        assert!(uri.contains("model.glb"));
    }

    #[test]
    fn resolve_asset_uri_resolves_relative_against_scene_dir() {
        let base = temp_dir("resolve_rel");
        let scene_file = base.join("scene.xrds");

        let resolved = resolve_asset_uri("assets/audio.ogg", Some(&scene_file));
        assert_eq!(resolved, base.join("assets").join("audio.ogg"));
    }

    #[test]
    fn resolve_asset_uri_passes_through_absolute_path() {
        let abs = PathBuf::from("/tmp/xrds_test/texture.png");
        let resolved = resolve_asset_uri("/tmp/xrds_test/texture.png", None);
        assert_eq!(resolved, abs);
    }

    #[test]
    fn do_export_app_copies_relative_assets_and_rewrites_uris() {
        let base = temp_dir("export_app");
        let scene_dir = base.join("project");
        let out_dir = base.join("exported");
        fs::create_dir_all(&scene_dir).unwrap();

        // Create a fake asset next to the scene file.
        let asset_path = scene_dir.join("models").join("cube.glb");
        fs::create_dir_all(asset_path.parent().unwrap()).unwrap();
        fs::write(&asset_path, b"GLB_DATA").unwrap();

        let scene_file = scene_dir.join("scene.xrds");
        let relative_uri = "models/cube.glb";

        let doc = xrds::scene_graph::XrdsSceneDocument {
            assets: vec![xrds::scene_graph::XrdsSceneAsset {
                id: "asset:cube".to_string(),
                uri: relative_uri.to_string(),
                kind: xrds::scene_graph::XrdsSceneAssetKind::Gltf,
            }],
            ..Default::default()
        };

        do_export_app(&doc, Some(&scene_file), &out_dir)
            .expect("app export with relative asset URI should succeed");

        // Asset should be copied.
        let copied = out_dir.join("assets").join("models").join("cube.glb");
        assert!(copied.exists(), "asset should be copied to output: {copied:?}");

        // scene.xrds should exist.
        assert!(out_dir.join("assets").join("scene.xrds").exists());
    }

    #[test]
    fn do_export_app_flattens_absolute_asset_uris_to_filename() {
        let base = temp_dir("export_abs");
        let out_dir = base.join("out");

        let asset_path = base.join("somewhere_else").join("music.ogg");
        fs::create_dir_all(asset_path.parent().unwrap()).unwrap();
        fs::write(&asset_path, b"OGG_DATA").unwrap();

        let doc = xrds::scene_graph::XrdsSceneDocument {
            assets: vec![xrds::scene_graph::XrdsSceneAsset {
                id: "asset:music".to_string(),
                uri: asset_path.to_string_lossy().into_owned(),
                kind: xrds::scene_graph::XrdsSceneAssetKind::Audio,
            }],
            ..Default::default()
        };

        do_export_app(&doc, None, &out_dir)
            .expect("app export with absolute asset URI should succeed");

        // Absolute URI must be flattened to filename (no subdirectory).
        let copied = out_dir.join("assets").join("music.ogg");
        assert!(copied.exists(), "absolute asset should be flattened to filename: {copied:?}");
    }
}
