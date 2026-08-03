use bevy::log::{error, info};
use std::io::{BufRead, BufReader};
use std::path::Path;
use xrds_gltf;
use crate::editor_state::{ApkExportJob, ExportJob};
use xrds_scene_graph::{
    XrdsSceneDocument, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneDocumentSession,
    XrdsSceneNodePayload, XrdsSceneAssetKind,
};
use crate::bevy_scene::build_default_document;
use crate::bridge::{ApkPrerequisite, EditorCommand};
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
            let scene_dir = session.0.save_path().and_then(|p| p.parent()).map(|p| p.to_path_buf());
            let (exported_doc, copy_errors) = prepare_export_document(&doc, out, scene_dir.as_deref());

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

        EditorCommand::ExportApk { output_dir } => {
            // Guard: another APK export already running.
            if state.apk_export_job.is_some() {
                state.pending_status = Some("APK export already in progress…".into());
                return false;
            }

            // Guard: scene must be saved before export.
            if session.0.is_dirty() || session.0.save_path().is_none() {
                state.pending_status = Some("Save the scene before exporting.".into());
                return false;
            }

            // Create output directory.
            let out = std::path::PathBuf::from(output_dir);
            if let Err(e) = std::fs::create_dir_all(&out) {
                state.pending_status = Some(format!("APK export failed: {e}"));
                return false;
            }

            // Stage scene + user assets into a temp directory.
            let staging = std::env::temp_dir().join("xrds-apk-stage");
            let _ = std::fs::remove_dir_all(&staging);
            if let Err(e) = std::fs::create_dir_all(&staging) {
                state.pending_status = Some(format!("APK export: cannot create staging dir: {e}"));
                return false;
            }

            let doc = session.0.document().clone();
            let scene_dir = session.0.save_path().and_then(|p| p.parent()).map(|p| p.to_path_buf());
            let (exported_doc, copy_errors) = prepare_export_document(&doc, &staging, scene_dir.as_deref());
            for e in &copy_errors { error!("[io/apk] asset copy: {}", e); }

            let scene_path = staging.join("scene.json");
            if let Err(e) = exported_doc.save_json(&scene_path) {
                state.pending_status = Some(format!("APK export: save scene.json failed: {e:?}"));
                return false;
            }

            let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..");

            // Shared log + result for the background thread.
            let log    = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
            // Surface asset staging problems in the build log — a scene that
            // references missing files otherwise produces an APK with no models
            // and no visible explanation.
            {
                let mut log_lines = log.lock().unwrap();
                for e in &copy_errors {
                    log_lines.push(format!("[err] asset staging: {e}"));
                }
            }
            let result = std::sync::Arc::new(std::sync::Mutex::new(
                None::<Result<String, String>>
            ));
            let log_clone    = std::sync::Arc::clone(&log);
            let result_clone = std::sync::Arc::clone(&result);
            let out_dir_str  = output_dir.clone();
            let staging_str  = staging.to_string_lossy().into_owned();
            let workspace_str = workspace_root.to_string_lossy().into_owned();

            std::thread::spawn(move || {
                let push = |line: String| {
                    log_clone.lock().unwrap().push(line);
                };

                // Run platform build script.
                #[cfg(target_os = "windows")]
                let mut cmd = {
                    let script = std::path::Path::new(&workspace_str)
                        .join("android/quest/build.ps1");
                    let mut c = std::process::Command::new("powershell.exe");
                    c.args([
                        "-ExecutionPolicy", "Bypass",
                        "-File", script.to_str().unwrap_or(""),
                        "-SceneDir", &staging_str,
                    ]);
                    c
                };
                #[cfg(not(target_os = "windows"))]
                let mut cmd = {
                    let script = std::path::Path::new(&workspace_str)
                        .join("android/quest/build.sh");
                    let mut c = std::process::Command::new("bash");
                    c.args([
                        script.to_str().unwrap_or(""),
                        "--scene-dir", &staging_str,
                    ]);
                    c
                };

                cmd.current_dir(&workspace_str)
                   .stdout(std::process::Stdio::piped())
                   .stderr(std::process::Stdio::piped());

                let mut child = match cmd.spawn() {
                    Ok(c) => c,
                    Err(e) => {
                        *result_clone.lock().unwrap() = Some(Err(format!("Failed to start build script: {e}")));
                        return;
                    }
                };

                // Stream stdout and stderr line-by-line from separate threads.
                // Use take() so child remains valid for wait() below.
                let log_out = std::sync::Arc::clone(&log_clone);
                let log_err = std::sync::Arc::clone(&log_clone);

                let stdout = child.stdout.take().unwrap();
                let stderr = child.stderr.take().unwrap();
                let (tx, rx) = std::sync::mpsc::channel::<()>();
                let tx2 = tx.clone();

                std::thread::spawn(move || {
                    for line in BufReader::new(stdout).lines().flatten() {
                        log_out.lock().unwrap().push(line);
                    }
                    let _ = tx.send(());
                });
                std::thread::spawn(move || {
                    for line in BufReader::new(stderr).lines().flatten() {
                        // cargo/gradle write ALL diagnostics to stderr — progress,
                        // warnings, and errors alike. Tag only real errors so the
                        // build log doesn't present routine output as failures.
                        let trimmed = line.trim_start();
                        let tagged = if trimmed.starts_with("error")
                            || trimmed.contains("error:")
                            || trimmed.contains("error[")
                        {
                            format!("[err] {line}")
                        } else {
                            line
                        };
                        log_err.lock().unwrap().push(tagged);
                    }
                    let _ = tx2.send(());
                });

                // Wait for both reader threads to drain, then collect exit status.
                rx.recv().ok();
                rx.recv().ok();

                let exit_status = child.wait();
                let success = exit_status.as_ref().map(|s| s.success()).unwrap_or(false);

                if !success {
                    let code = exit_status.ok()
                        .and_then(|s| s.code())
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    push(format!("[err] Build script exited with code {code}"));
                    // Flush the log to disk before returning.
                    write_build_log(&log_clone.lock().unwrap(), &out_dir_str);
                    *result_clone.lock().unwrap() = Some(Err(
                        format!("Build script failed (exit code {code}) — see log for details")
                    ));
                    return;
                }

                push(String::from("--- build script finished ---"));

                // Verify APK was produced.
                let apk_src = std::path::Path::new(&workspace_str)
                    .join("android/quest/build/xrds-app.apk");
                let apk_dst = std::path::Path::new(&out_dir_str).join("xrds-app.apk");

                if !apk_src.exists() {
                    *result_clone.lock().unwrap() = Some(Err(
                        "Build script succeeded but xrds-app.apk was not produced.".into()
                    ));
                    return;
                }

                if let Err(e) = std::fs::copy(&apk_src, &apk_dst) {
                    *result_clone.lock().unwrap() = Some(Err(format!("Copy APK failed: {e}")));
                    return;
                }

                // Write install scripts alongside the APK.
                if let Err(e) = write_install_scripts(std::path::Path::new(&out_dir_str)) {
                    push(format!("[warn] install script write failed: {e}"));
                }

                push(format!("APK exported to {out_dir_str}"));
                write_build_log(&log_clone.lock().unwrap(), &out_dir_str);
                *result_clone.lock().unwrap() = Some(Ok(
                    format!("APK exported to {out_dir_str}")
                ));
            });

            state.apk_export_job = Some(ApkExportJob { out_dir: output_dir.clone(), log, result });
            state.pending_status = Some("Building APK… (this may take several minutes)".into());
            info!("[io] APK export started → {}", output_dir);
            false
        }

        EditorCommand::CheckApkPrerequisites => {
            let prereqs = check_apk_prerequisites();
            let failed: Vec<&str> = prereqs.iter().filter(|p| !p.ok).map(|p| p.name.as_str()).collect();
            if failed.is_empty() {
                info!("[io] APK prerequisites: all OK");
            } else {
                info!("[io] APK prerequisites: missing {:?}", failed);
            }
            state.apk_prerequisites = Some(prereqs);
            false
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// APK prerequisite check
// ---------------------------------------------------------------------------

fn check_apk_prerequisites() -> Vec<ApkPrerequisite> {
    let mut items = Vec::new();

    // 1. ANDROID_HOME + build-tools/
    let android_home = std::env::var("ANDROID_HOME").unwrap_or_default();
    let sdk_ok = !android_home.is_empty()
        && Path::new(&android_home).join("build-tools").exists();
    items.push(ApkPrerequisite {
        name: "Android SDK (ANDROID_HOME)".into(),
        ok: sdk_ok,
        hint: if sdk_ok { String::new() } else {
            "Set ANDROID_HOME to your Android SDK root \
             (e.g. %LOCALAPPDATA%\\Android\\Sdk on Windows)".into()
        },
    });

    // 2. Android NDK — try env vars, then $ANDROID_HOME/ndk/
    let ndk_ok = std::env::var("ANDROID_NDK_HOME")
        .or_else(|_| std::env::var("NDK_HOME"))
        .or_else(|_| std::env::var("ANDROID_NDK_ROOT"))
        .map(|p| Path::new(&p).exists())
        .unwrap_or_else(|_| {
            !android_home.is_empty() && Path::new(&android_home).join("ndk").exists()
        });
    items.push(ApkPrerequisite {
        name: "Android NDK".into(),
        ok: ndk_ok,
        hint: if ndk_ok { String::new() } else {
            "Install NDK via Android Studio SDK Manager, \
             or set ANDROID_NDK_HOME to the NDK directory".into()
        },
    });

    // 3. cargo-ndk
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let cargo_ndk_ok = std::process::Command::new(&cargo)
        .args(["ndk", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    items.push(ApkPrerequisite {
        name: "cargo-ndk".into(),
        ok: cargo_ndk_ok,
        hint: if cargo_ndk_ok { String::new() } else {
            "Run: cargo install cargo-ndk".into()
        },
    });

    // 4. OpenXR loader (fetched by fetch_loader.ps1 / fetch_loader.sh)
    let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..");
    let loader = workspace_root
        .join("android/quest/libs/arm64-v8a/libopenxr_loader.so");
    let loader_ok = loader.exists();
    items.push(ApkPrerequisite {
        name: "OpenXR loader (libopenxr_loader.so)".into(),
        ok: loader_ok,
        hint: if loader_ok { String::new() } else {
            "Run android/quest/fetch_loader.ps1 (Windows) or \
             android/quest/fetch_loader.sh (Linux/macOS) to download the loader".into()
        },
    });

    items
}

// ---------------------------------------------------------------------------
// Export helpers
// ---------------------------------------------------------------------------

/// Clone the document, copy referenced assets to `output_dir/assets/`, and
/// replace absolute URIs with relative ones so the exported app is self-contained.
///
/// `scene_dir` is the directory of the opened scene file (save path parent);
/// relative catalog URIs are resolved against it (`<scene_dir>/<uri>`, then
/// `<scene_dir>/assets/<uri>`) so portable scenes stage their files too — a
/// relative URI whose file cannot be found is reported in the error list.
fn prepare_export_document(
    doc: &xrds_scene_graph::XrdsSceneDocument,
    output_dir: &Path,
    scene_dir: Option<&Path>,
) -> (xrds_scene_graph::XrdsSceneDocument, Vec<String>) {
    let assets_dir = output_dir.join("assets");
    let _ = std::fs::create_dir_all(&assets_dir);
    let mut new_doc = doc.clone();
    let mut errors  = Vec::new();

    // Pass 1: copy every catalog file into assets/ and make its URI relative.
    // Absolute URIs are flattened to a bare filename; relative URIs keep their
    // subdirectory structure (e.g. "environment_maps/diffuse.ktx2"). URIs carry
    // no "assets/" prefix; the runtime's asset server root is exe_dir/assets/
    // (desktop) or the extracted APK assets root (Android), so these relative
    // URIs resolve correctly in both contexts.
    for asset in &mut new_doc.assets {
        let src = Path::new(&asset.uri);
        if src.is_absolute() {
            if src.exists() {
                let filename = src.file_name().unwrap_or_default().to_string_lossy();
                let dst = assets_dir.join(filename.as_ref());
                if let Err(e) = std::fs::copy(src, &dst) {
                    errors.push(format!("copy '{}': {e}", src.display()));
                } else {
                    asset.uri = filename.to_string();
                }
            } else {
                errors.push(format!("asset '{}' not found: {}", asset.id, asset.uri));
            }
        } else {
            let resolved = scene_dir.and_then(|d| {
                [d.join(&asset.uri), d.join("assets").join(&asset.uri)]
                    .into_iter()
                    .find(|c| c.is_file())
            });
            match resolved {
                Some(found) => {
                    let rel = asset.uri.replace('\\', "/");
                    let dst = assets_dir.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
                    if let Some(parent) = dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(e) = std::fs::copy(&found, &dst) {
                        errors.push(format!("copy '{}': {e}", found.display()));
                    } else {
                        asset.uri = rel;
                    }
                }
                None => errors.push(format!(
                    "asset '{}' not found near the scene file: {}",
                    asset.id, asset.uri
                )),
            }
        }
    }

    // Pass 2: build a catalog id→uri snapshot (now with rewritten URIs).
    let catalog_uri: std::collections::HashMap<String, String> = new_doc.assets.iter()
        .map(|a| (a.id.clone(), a.uri.clone()))
        .collect();

    // Pass 3: rewrite asset_uri in GltfAsset node payloads so the on-disk
    // scene.json is self-contained regardless of which resolution path the
    // runtime chooses (catalog vs. node-level fallback).
    for node in &mut new_doc.nodes {
        if let XrdsSceneNodePayload::GltfAsset(ref mut gltf) = node.payload {
            let portable = gltf.asset_id.as_deref()
                .and_then(|id| catalog_uri.get(id))
                .cloned()
                .or_else(|| {
                    // No catalog entry: try to copy the file and return the bare name.
                    let src = Path::new(&gltf.asset_uri);
                    if src.is_absolute() && src.exists() {
                        let filename = src.file_name()?.to_string_lossy().into_owned();
                        let dst = assets_dir.join(&filename);
                        std::fs::copy(src, &dst).ok()?;
                        Some(filename)
                    } else {
                        None
                    }
                });
            if let Some(uri) = portable {
                gltf.asset_uri = uri;
            }
        }
    }

    (new_doc, errors)
}

/// Write all log lines to `output_dir/build_log.txt`.
fn write_build_log(lines: &[String], output_dir: &str) {
    let path = std::path::Path::new(output_dir).join("build_log.txt");
    let content = lines.join("\n");
    if let Err(e) = std::fs::write(&path, content) {
        error!("[io/apk] could not write build_log.txt: {e}");
    }
}

/// Write install.ps1, install.sh and README.txt into `output_dir`.
fn write_install_scripts(output_dir: &Path) -> Result<(), String> {
    let pkg = "org.openxrds.devicesdk";
    let activity = "android.app.NativeActivity";

    std::fs::write(
        output_dir.join("install.ps1"),
        format!(
            "# Install xrds-app.apk on a connected Meta Quest and launch it.\r\n\
             # Requirements: adb in PATH, Quest in Developer Mode, connected via USB.\r\n\
             $Pkg = \"{pkg}\"\r\n\
             $ExtScene = \"/sdcard/Android/data/$Pkg/files/scene.json\"\r\n\
             # Remove any dev-mode scene pushed to external storage so the APK-bundled\r\n\
             # scene takes effect. Errors are silenced — the file may not exist.\r\n\
             Write-Host \"Clearing external scene override (if any)...\"\r\n\
             adb shell rm -f \"$ExtScene\" 2>$null\r\n\
             Write-Host \"Installing APK...\"\r\n\
             adb install -r \"$PSScriptRoot\\xrds-app.apk\"\r\n\
             Write-Host \"Launching app...\"\r\n\
             adb shell am start -n \"$Pkg/{activity}\"\r\n\
             Write-Host \"\"\r\n\
             Write-Host \"Done. The app launched on your Quest.\"\r\n\
             Write-Host \"To find it later: App Library -> All -> Unknown Sources -> XRDS App\"\r\n"
        ),
    ).map_err(|e| format!("write install.ps1: {e}"))?;

    let sh_path = output_dir.join("install.sh");
    std::fs::write(
        &sh_path,
        format!(
            "#!/usr/bin/env bash\n\
             # Install xrds-app.apk on a connected Meta Quest and launch it.\n\
             # Requirements: adb in PATH, Quest in Developer Mode, connected via USB.\n\
             set -e\n\
             PKG=\"{pkg}\"\n\
             DIR=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
             EXT_SCENE=\"/sdcard/Android/data/$PKG/files/scene.json\"\n\
             # Remove any dev-mode scene pushed to external storage so the APK-bundled\n\
             # scene takes effect. Errors are silenced — the file may not exist.\n\
             echo \"Clearing external scene override (if any)...\"\n\
             adb shell rm -f \"$EXT_SCENE\" 2>/dev/null || true\n\
             echo \"Installing APK...\"\n\
             adb install -r \"$DIR/xrds-app.apk\"\n\
             echo \"Launching app...\"\n\
             adb shell am start -n \"$PKG/{activity}\"\n\
             echo \"\"\n\
             echo \"Done. The app launched on your Quest.\"\n\
             echo \"To find it later: App Library -> All -> Unknown Sources -> XRDS App\"\n"
        ),
    ).map_err(|e| format!("write install.sh: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&sh_path)
            .map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&sh_path, perms).map_err(|e| e.to_string())?;
    }

    std::fs::write(
        output_dir.join("README.txt"),
        "XRDS App — Meta Quest APK Bundle\r\n\
         =================================\r\n\
         \r\n\
         Requirements\r\n\
         ------------\r\n\
         - Meta Quest 3 or Quest Pro (Quest 2 not supported)\r\n\
         - Developer Mode enabled on the headset\r\n\
         - USB cable connected between PC and Quest\r\n\
         - Android Platform Tools (adb) in your PATH\r\n\
         \r\n\
         Install and launch\r\n\
         ------------------\r\n\
         Windows:  .\\install.ps1\r\n\
         Linux/Mac: bash install.sh\r\n\
         \r\n\
         Manual ADB commands:\r\n\
           adb install -r xrds-app.apk\r\n\
           adb shell am start -n org.openxrds.devicesdk/android.app.NativeActivity\r\n\
         \r\n\
         Finding the app after installation\r\n\
         -----------------------------------\r\n\
         The app does not appear in the main Quest app library — Meta only shows\r\n\
         store-purchased apps there. To find it:\r\n\
         \r\n\
           Quest universal menu -> App Library -> All\r\n\
           -> scroll to the \"Unknown Sources\" section\r\n\
         \r\n\
         The app is listed there as \"XRDS App\".\r\n\
         You can also launch it at any time from your PC:\r\n\
           adb shell am start -n org.openxrds.devicesdk/android.app.NativeActivity\r\n\
         \r\n\
         Scene not loading / seeing the default scene\r\n\
         --------------------------------------------\r\n\
         If you previously pushed a scene.json to the device for dev-mode testing,\r\n\
         that file overrides the APK-bundled scene. The install scripts clear it\r\n\
         automatically, but to remove it manually:\r\n\
           adb shell rm /sdcard/Android/data/org.openxrds.devicesdk/files/scene.json\r\n\
         Then relaunch the app.\r\n\
         \r\n\
         Troubleshooting\r\n\
         ---------------\r\n\
         - INSTALL_FAILED_UPDATE_INCOMPATIBLE (signing mismatch):\r\n\
             adb uninstall org.openxrds.devicesdk\r\n\
           then re-run install.\r\n\
         - App crashes on launch, check logcat:\r\n\
             adb logcat -s xrds\r\n\
         - Developer Mode: Settings -> Developer -> USB Debugging must be ON.\r\n\
           Wear the headset and accept the \"Allow USB Debugging\" prompt.\r\n"
    ).map_err(|e| format!("write README.txt: {e}"))?;

    Ok(())
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use xrds_scene_graph::{XrdsSceneAsset, XrdsSceneGltfAsset, XrdsSceneNode, XrdsSceneNodeId};

    /// Regression: a scene with RELATIVE catalog URIs (the portable layout
    /// produced by export / dev-mode pushes) must still stage its asset files
    /// when re-exported. Before the fix only absolute URIs were copied, so the
    /// APK shipped with scene.json but no models.
    #[test]
    fn prepare_export_stages_relative_uri_assets() {
        let base = std::env::temp_dir()
            .join(format!("xrds-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let scene_dir = base.join("scene");
        let out_dir = base.join("out");
        std::fs::create_dir_all(scene_dir.join("assets/environment_maps")).unwrap();
        std::fs::write(scene_dir.join("assets/drone.glb"), b"stub").unwrap();
        std::fs::write(scene_dir.join("assets/environment_maps/diffuse.ktx2"), b"stub").unwrap();

        let doc = XrdsSceneDocument {
            assets: vec![
                XrdsSceneAsset {
                    id: "drone".into(),
                    uri: "drone.glb".into(),
                    kind: XrdsSceneAssetKind::Gltf,
                },
                XrdsSceneAsset {
                    id: "env_diffuse".into(),
                    uri: "environment_maps/diffuse.ktx2".into(),
                    kind: XrdsSceneAssetKind::EnvironmentMap,
                },
            ],
            nodes: vec![XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "drone".into(),
                enabled: true,
                visible: true,
                grabbable: false,
                transform: Default::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("drone".into()),
                    asset_uri: "C:/somewhere/else/drone.glb".into(),
                    scene_index: 0,
                    export_policy: xrds_scene_graph::XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: Default::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            }],
            ..Default::default()
        };

        let (exported, errors) = prepare_export_document(&doc, &out_dir, Some(&scene_dir));

        assert!(errors.is_empty(), "unexpected staging errors: {errors:?}");
        assert!(out_dir.join("assets/drone.glb").is_file());
        assert!(out_dir.join("assets/environment_maps/diffuse.ktx2").is_file());
        assert_eq!(exported.assets[0].uri, "drone.glb");
        assert_eq!(exported.assets[1].uri, "environment_maps/diffuse.ktx2");
        // Payload fallback URI must be rewritten to the catalog's portable URI.
        let XrdsSceneNodePayload::GltfAsset(gltf) = &exported.nodes[0].payload else {
            panic!("expected gltf payload");
        };
        assert_eq!(gltf.asset_uri, "drone.glb");

        let _ = std::fs::remove_dir_all(&base);
    }

    /// A relative URI whose file cannot be found near the scene must produce a
    /// staging error (surfaced in the build log) instead of silently exporting
    /// an APK without the asset.
    #[test]
    fn prepare_export_reports_missing_relative_assets() {
        let base = std::env::temp_dir()
            .join(format!("xrds-export-missing-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let scene_dir = base.join("scene");
        let out_dir = base.join("out");
        std::fs::create_dir_all(&scene_dir).unwrap();

        let doc = XrdsSceneDocument {
            assets: vec![XrdsSceneAsset {
                id: "ghost".into(),
                uri: "ghost.glb".into(),
                kind: XrdsSceneAssetKind::Gltf,
            }],
            ..Default::default()
        };

        let (_, errors) = prepare_export_document(&doc, &out_dir, Some(&scene_dir));
        assert_eq!(errors.len(), 1, "expected one staging error: {errors:?}");
        assert!(errors[0].contains("ghost.glb"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
