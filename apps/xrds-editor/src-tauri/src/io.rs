use bevy::log::{error, info};
use std::path::Path;
use xrds_gltf;
use crate::editor_state::ExportJob;
use xrds_scene_graph::{
    XrdsSceneDocument, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneDocumentSession,
    XrdsSceneNodePayload, XrdsSceneAssetKind,
};
use crate::bevy_scene::build_default_document;
use crate::bridge::EditorCommand;
use crate::editor_state::{EditorSession, EditorState};

/// Apply a file I/O or edit-history EditorCommand.
/// Returns true if a full reimport is needed.
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn detect_asset_kind(path: &str) -> Option<XrdsSceneAssetKind> {
    let ext = std::path::Path::new(path)
        .extension().and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    match ext.as_deref() {
        Some("glb" | "gltf")             => Some(XrdsSceneAssetKind::Gltf),
        Some("png" | "jpg" | "jpeg" | "webp" | "ktx2") => Some(XrdsSceneAssetKind::Texture),
        Some("mp3" | "wav" | "ogg" | "flac")            => Some(XrdsSceneAssetKind::Audio),
        Some("hdr")                       => Some(XrdsSceneAssetKind::EnvironmentMap),
        _                                 => None,
    }
}

fn node_references_asset(node: &XrdsSceneNode, asset_id: &str) -> bool {
    match &node.payload {
        XrdsSceneNodePayload::GltfAsset(a) =>
            a.asset_id.as_deref() == Some(asset_id),
        XrdsSceneNodePayload::AudioClip(a) =>
            a.asset_id == asset_id,
        _ => false,
    }
}

fn collect_subtree(doc: &XrdsSceneDocument, root: XrdsSceneNodeId) -> Vec<XrdsSceneNodeId> {
    let mut result = vec![root];
    let mut i = 0;
    while i < result.len() {
        let cur = result[i];
        for n in doc.nodes.iter().filter(|n| n.parent_id == Some(cur)) {
            result.push(n.id);
        }
        i += 1;
    }
    result
}

pub fn apply_io_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        // ── Undo / Redo ──────────────────────────────────────────────────────
        EditorCommand::Undo => {
            if session.0.undo() {
                state.selection.clear();
                state.pending_translations.clear();
                state.pending_rotations.clear();
                info!("[io] undo — {} undo steps remaining", session.0.undo_count());
                true
            } else {
                false
            }
        }
        EditorCommand::Redo => {
            if session.0.redo() {
                state.selection.clear();
                state.pending_translations.clear();
                state.pending_rotations.clear();
                info!("[io] redo — {} redo steps remaining", session.0.redo_count());
                true
            } else {
                false
            }
        }

        // ── Scene lifecycle ──────────────────────────────────────────────────
        EditorCommand::NewScene => {
            let doc = build_default_document();
            match XrdsSceneDocumentSession::new(doc) {
                Ok(new_session) => {
                    session.0 = new_session;
                    state.selection.clear();
                    state.pending_translations.clear();
                    state.pending_rotations.clear();
                    state.active_camera_id = None;
                    info!("[io] new scene");
                    true
                }
                Err(e) => {
                    error!("[io] new scene validation error: {:?}", e);
                    false
                }
            }
        }

        EditorCommand::OpenScene { path } => {
            match XrdsSceneDocumentSession::load_json(path) {
                Ok(new_session) => {
                    session.0 = new_session;
                    state.selection.clear();
                    state.pending_translations.clear();
                    state.pending_rotations.clear();
                    state.active_camera_id = None;
                    info!("[io] opened scene: {}", path);
                    true
                }
                Err(e) => {
                    error!("[io] failed to open '{}': {:?}", path, e);
                    false
                }
            }
        }

        EditorCommand::SaveScene => {
            if session.0.save_path().is_some() {
                match session.0.save() {
                    Ok(_) => info!("[io] saved"),
                    Err(e) => error!("[io] save failed: {:?}", e),
                }
            }
            // No path set: the JS layer should have shown a dialog and sent SaveSceneAs.
            false
        }

        EditorCommand::RemoveAsset { asset_id } => {
            let asset_id = asset_id.clone();
            match session.0.edit(|doc| {
                // Collect all scene nodes that reference this asset.
                let to_remove: Vec<_> = doc.nodes.iter()
                    .filter(|n| node_references_asset(n, &asset_id))
                    .map(|n| n.id)
                    .collect();
                // Remove those nodes and their entire subtrees.
                for root_id in to_remove {
                    let subtree = collect_subtree(doc, root_id);
                    doc.nodes.retain(|n| !subtree.contains(&n.id));
                }
                // Remove the asset from the catalog.
                doc.assets.retain(|a| a.id != asset_id);
            }) {
                Ok(_) => {
                    info!("[io] removed asset '{}' and its scene nodes", asset_id);
                    state.pending_status = Some(format!("Removed: {asset_id}"));
                    true // needs full reimport if any nodes were removed
                }
                Err(e) => {
                    error!("[io] RemoveAsset '{}' failed: {:?}", asset_id, e);
                    false
                }
            }
        }

        EditorCommand::SaveSceneAs { path } => {
            match session.0.save_as(path) {
                Ok(_) => info!("[io] saved as: {}", path),
                Err(e) => error!("[io] save-as failed: {:?}", e),
            }
            false
        }

        EditorCommand::ImportAsset { path } => {
            let asset_id_hint = std::path::Path::new(path)
                .file_stem().and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "asset".to_string());
            let uri = path.replace('\\', "/");

            let kind = detect_asset_kind(path);
            let result = match kind {
                Some(XrdsSceneAssetKind::Gltf) =>
                    session.0.ensure_gltf_asset(Some(asset_id_hint), uri.clone())
                        .map_err(|e| format!("{e:?}")),
                Some(XrdsSceneAssetKind::Texture) =>
                    session.0.ensure_texture_asset(Some(asset_id_hint), uri.clone())
                        .map_err(|e| format!("{e:?}")),
                Some(XrdsSceneAssetKind::Audio) =>
                    session.0.ensure_audio_asset(Some(asset_id_hint), uri.clone())
                        .map_err(|e| format!("{e:?}")),
                Some(XrdsSceneAssetKind::EnvironmentMap) =>
                    session.0.register_environment_map_asset(asset_id_hint, uri.clone())
                        .map(|a| xrds_scene_graph::XrdsSceneAssetEnsureResult { asset: a, created: true })
                        .map_err(|e| format!("{e:?}")),
                None => {
                    state.pending_status = Some(format!("Unknown asset type: {path}"));
                    return false;
                }
            };

            match result {
                Ok(r) => {
                    info!("[io] imported {} asset '{}' → {}", format!("{:?}", kind.unwrap()), r.asset.id, uri);
                    state.pending_status = Some(format!("Imported: {}", r.asset.id));
                }
                Err(e) => {
                    error!("[io] import failed for '{}': {}", path, e);
                    state.pending_status = Some(format!("Import failed: {e}"));
                }
            }
            false
        }

        EditorCommand::ExportGlb { path } => {
            let doc = session.0.document().clone();
            match xrds_gltf::export_glb(&doc) {
                Ok(bytes) => {
                    let size_kb = bytes.len() / 1024;
                    match std::fs::write(path, &bytes) {
                        Ok(_) => {
                            let name = std::path::Path::new(path)
                                .file_name().and_then(|s| s.to_str()).unwrap_or(path);
                            info!("[io] exported GLB: {} ({} KB)", path, size_kb);
                            state.pending_status = Some(format!("Exported: {name} ({size_kb} KB)"));
                        }
                        Err(e) => {
                            error!("[io] GLB write failed: {}", e);
                            state.pending_status = Some(format!("Export failed: {e}"));
                        }
                    }
                }
                Err(e) => {
                    error!("[io] export_glb failed: {:?}", e);
                    state.pending_status = Some(format!("Export failed: {e:?}"));
                }
            }
            false
        }

        EditorCommand::ExportApplication { output_dir } => {
            if state.export_job.is_some() {
                state.pending_status = Some("Export already in progress…".into());
                return false;
            }

            let out = std::path::Path::new(output_dir);
            if let Err(e) = std::fs::create_dir_all(out) {
                error!("[io] cannot create output dir: {}", e);
                state.pending_status = Some(format!("Export failed: {e}"));
                return false;
            }

            // Prepare a portable copy of the document with relative asset URIs.
            let doc = session.0.document().clone();
            let (exported_doc, copy_errors) = prepare_export_document(&doc, out);

            let scene_path = out.join("scene.json");
            if let Err(e) = exported_doc.save_json(&scene_path) {
                error!("[io] save scene.json failed: {:?}", e);
                state.pending_status = Some(format!("Export failed: {e:?}"));
                return false;
            }

            // Copy workspace assets/ (fonts, models) to output_dir/assets/.
            let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..");
            copy_dir_recursive(&workspace_root.join("assets"), &out.join("assets"));

            if !copy_errors.is_empty() {
                for e in &copy_errors { error!("[io] asset copy: {}", e); }
            }

            // Spawn background cargo build.
            let out_dir_str   = output_dir.clone();
            let workspace_str = workspace_root.to_string_lossy().into_owned();
            let result = std::sync::Arc::new(std::sync::Mutex::new(
                None::<Result<String, String>>
            ));
            let result_clone = std::sync::Arc::clone(&result);

            std::thread::spawn(move || {
                let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
                let status = std::process::Command::new(&cargo)
                    .args(["build", "--release", "-p", "xrds-app"])
                    .current_dir(&workspace_str)
                    .status();

                let final_result = match status {
                    Ok(s) if s.success() => {
                        let exe_name = format!("xrds-app{}", std::env::consts::EXE_SUFFIX);
                        let src = std::path::Path::new(&workspace_str)
                            .join("target").join("release").join(&exe_name);
                        let dst = std::path::Path::new(&out_dir_str).join(&exe_name);
                        match std::fs::copy(&src, &dst) {
                            Ok(_) => Ok(format!("Exported to {out_dir_str}")),
                            Err(e) => Err(format!("Binary copy failed: {e}")),
                        }
                    }
                    Ok(s) => Err(format!("cargo build exited with code {:?}", s.code())),
                    Err(e) => Err(format!("Failed to run cargo: {e}")),
                };
                *result_clone.lock().unwrap() = Some(final_result);
            });

            state.export_job = Some(ExportJob { out_dir: output_dir.clone(), result });
            state.pending_status = Some("Building… (this may take a minute)".into());
            false
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Export helpers
// ---------------------------------------------------------------------------

/// Clone the document, copy referenced assets to `output_dir/assets/`, and
/// replace absolute URIs with relative ones so the exported app is self-contained.
fn prepare_export_document(
    doc: &xrds_scene_graph::XrdsSceneDocument,
    output_dir: &Path,
) -> (xrds_scene_graph::XrdsSceneDocument, Vec<String>) {
    let assets_dir = output_dir.join("assets");
    let _ = std::fs::create_dir_all(&assets_dir);
    let mut new_doc = doc.clone();
    let mut errors  = Vec::new();

    for asset in &mut new_doc.assets {
        let src = Path::new(&asset.uri);
        if src.is_absolute() && src.exists() {
            let filename = src.file_name().unwrap_or_default().to_string_lossy();
            let dst = assets_dir.join(filename.as_ref());
            if let Err(e) = std::fs::copy(src, &dst) {
                errors.push(format!("copy '{}': {e}", src.display()));
            } else {
                // URI = plain filename (no "assets/" prefix) because
                // asset_path in xrds-app is already set to exe_dir/assets.
                // Bevy resolves "buster_drone.glb" → exe_dir/assets/buster_drone.glb.
                asset.uri = filename.to_string();
            }
        }
    }
    (new_doc, errors)
}

/// Recursively copy `src` directory tree into `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    if !src.exists() { return; }
    let _ = std::fs::create_dir_all(dst);
    let Ok(entries) = std::fs::read_dir(src) else { return; };
    for entry in entries.flatten() {
        let dst_path = dst.join(entry.file_name());
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            copy_dir_recursive(&entry.path(), &dst_path);
        } else {
            let _ = std::fs::copy(entry.path(), dst_path);
        }
    }
}
