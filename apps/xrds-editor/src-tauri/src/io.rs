use bevy::log::{error, info};
use std::path::Path;
use crate::task_queue::{tag as task_tag, TaskContext, TaskLane, TaskQueue};
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
        Some("glb" | "gltf")                   => Some(XrdsSceneAssetKind::Gltf),
        Some("png" | "jpg" | "jpeg" | "webp")  => Some(XrdsSceneAssetKind::Texture),
        Some("mp3" | "wav" | "ogg" | "flac")   => Some(XrdsSceneAssetKind::Audio),
        // `.ktx2` asks the file, rather than guessing from the extension.
        //
        // The container holds either a plain 2-D texture or a cubemap, and only a
        // cubemap is usable as a skybox or IBL — Bevy's `Skybox` and
        // `EnvironmentMapLight` both require a cube texture. The KTX2 header says
        // which: `faceCount` is 6 for a cubemap and 1 otherwise. The SDK's own
        // `environment_maps/{diffuse,specular}.ktx2` report 6.
        //
        // Extension alone was wrong in both directions. As `Texture` it made skybox
        // and IBL unusable — importing the SDK's own environment map produced
        // something neither would accept, and enabling a skybox then failed document
        // validation with nothing shown ("I can't check the skybox checkbox",
        // 2026-08-19). Blanket `EnvironmentMap` would have been equally wrong for a
        // `.ktx2` compressed material texture, which is a normal thing to have.
        Some("ktx2") => Some(match ktx2_face_count(path) {
            Some(6) => XrdsSceneAssetKind::EnvironmentMap,
            // Unreadable or truncated headers fall back to Texture: a 2-D texture
            // that cannot be used as a skybox fails visibly in the picker, whereas a
            // non-cubemap offered *as* a skybox fails at render time on a headset.
            _ => XrdsSceneAssetKind::Texture,
        }),
        // Radiance `.hdr` is always a single equirectangular image — the format has
        // no cubemap concept — so it is NOT directly usable as a Bevy skybox or IBL
        // and needs converting to a cubemap KTX2 first. Kept as `EnvironmentMap`
        // because that is what it is *for*, and reclassifying it would hide a
        // downloaded panorama from the only picker an author would look in. See
        // `docs/small-phases-plan.md` — the missing conversion step is recorded
        // there rather than papered over here.
        Some("hdr")                            => Some(XrdsSceneAssetKind::EnvironmentMap),
        // OpenEXR, and the format authors actually get: ambientCG and Poly Haven
        // both ship `.exr` as the payload. Like `.hdr` it is a single
        // equirectangular image and still needs converting to a cubemap KTX2.
        Some("exr")                            => Some(XrdsSceneAssetKind::EnvironmentMap),
        _                                      => None,
    }
}

// ---------------------------------------------------------------------------
// Environment-map import contract
// ---------------------------------------------------------------------------

/// Why an image cannot serve as an environment map. Carries the explanation, not
/// just the fact.
///
/// The messages matter more than usual here. An ambientCG download contains
/// `..._HDR.exr` and `..._TONEMAPPED.jpg` side by side, and picking the wrong one
/// is the obvious mistake — so "unsupported format" would be a wasted answer. Each
/// variant says what is wrong with *this* file and what to do instead.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvironmentSourceError {
    /// An LDR format. Fatal for lighting: these cannot store values above 1.0, so
    /// the sun clamps to the same brightness as a cloud.
    NotHighDynamicRange { ext: String },
    /// Not a 2:1 latitude-longitude panorama.
    NotEquirectangular { width: u32, height: u32 },
    Undecodable { detail: String },
}

impl std::fmt::Display for EnvironmentSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Says the thing an author can act on: the file they want is usually
            // sitting in the same folder as the one they picked.
            Self::NotHighDynamicRange { ext } => write!(
                f,
                ".{ext} files hold no brightness above 1.0, so they cannot light a \
                 scene — the sun would be no brighter than a cloud. Use the .exr \
                 (or .hdr) from the same download instead."
            ),
            Self::NotEquirectangular { width, height } => write!(
                f,
                "An environment map must be an equirectangular panorama with a 2:1 \
                 width-to-height ratio. This image is {width}×{height}."
            ),
            Self::Undecodable { detail } => write!(f, "Could not read the image: {detail}"),
        }
    }
}

/// A validated equirectangular source, ready for conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentSource {
    pub width: u32,
    pub height: u32,
}

/// Decide whether a file can serve as an environment map, per the import contract
/// in `docs/editor-task-queue-and-hdr-conversion.md`.
///
/// Accepts equirectangular `.exr` and `.hdr`. `.ktx2` cubemaps are handled by
/// `detect_asset_kind` and never reach here — they are already in the target
/// format and need no conversion.
pub fn classify_environment_source(path: &str) -> Result<EnvironmentSource, EnvironmentSourceError> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Refused before decoding, and deliberately so: an LDR file is not a damaged
    // environment map, it is the wrong kind of file, and reading its pixels would
    // not change the answer.
    if !matches!(ext.as_str(), "exr" | "hdr") {
        return Err(EnvironmentSourceError::NotHighDynamicRange { ext });
    }

    // Only the dimensions are needed, so this reads the header rather than
    // decoding 4096×2048 half-floats to answer a question about aspect ratio.
    let reader = image::ImageReader::open(path)
        .map_err(|e| EnvironmentSourceError::Undecodable { detail: e.to_string() })?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| EnvironmentSourceError::Undecodable { detail: e.to_string() })?;

    // Tolerance rather than exact equality: a 2:1 panorama is what every vendor
    // ships, but rounding in an author's own export should not be a refusal.
    if height == 0 || (width as f32 / height as f32 - 2.0).abs() > 0.01 {
        return Err(EnvironmentSourceError::NotEquirectangular { width, height });
    }

    Ok(EnvironmentSource { width, height })
}

/// `faceCount` from a KTX2 header — 6 for a cubemap, 1 for a plain texture.
///
/// The header is fixed-layout and starts the file, so this reads 40 bytes rather
/// than parsing the container: a 12-byte identifier, then `vkFormat`, `typeSize`,
/// `pixelWidth`, `pixelHeight`, `pixelDepth`, `layerCount`, and `faceCount` as
/// little-endian `u32`s. Returns `None` when the file is missing, too short, or not
/// KTX2 — all of which mean "do not claim this is a cubemap".
fn ktx2_face_count(path: &str) -> Option<u32> {
    use std::io::Read;

    const KTX2_IDENTIFIER: [u8; 12] = [
        0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
    ];
    const FACE_COUNT_OFFSET: usize = 36;

    let mut header = [0u8; FACE_COUNT_OFFSET + 4];
    std::fs::File::open(path)
        .ok()?
        .read_exact(&mut header)
        .ok()?;

    if header[..12] != KTX2_IDENTIFIER {
        return None;
    }
    Some(u32::from_le_bytes(
        header[FACE_COUNT_OFFSET..FACE_COUNT_OFFSET + 4].try_into().ok()?,
    ))
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
    tasks: &mut TaskQueue,
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
            } else {
                // The frontend routes Ctrl+S to Save As when `has_save_path` is
                // false, so reaching here means the keystroke came from the Bevy
                // viewport instead (keyboard_shortcuts.rs), where no file dialog
                // can be opened. Say so rather than doing nothing: this branch
                // used to return silently, with only a comment claiming "the JS
                // layer should have shown a dialog" — which nothing did, so
                // Ctrl+S on a new scene appeared to work and saved nothing.
                let message = "Scene has never been saved — use Ctrl+Shift+S (Save As) first";
                info!("[io] {message}");
                state.pending_status = Some(message.to_string());
            }
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
                    return false;
                }
            }
            // Reimport so the runtime's asset catalog picks the new entry up.
            //
            // This returned `false`, and the consequence was invisible: an imported
            // asset landed in the *document* but never in
            // `XrdsImportedAssetCatalog`, which is what the environment resolvers
            // search. So a freshly imported environment map could not be found —
            // "even after activating skybox, I see nothing but grey background",
            // 2026-08-20 — and the same was true of every texture until some
            // unrelated structural change happened to trigger a reimport.
            //
            // A full reimport is heavier than merging the catalog alone, but
            // importing an asset is a rare, deliberate action, and the alternative
            // is a second sync path that can drift from this one.
            true
        }

        // `ExportGlb` was handled here. Scene export to glTF is retired: glTF has
        // no vocabulary for panels, triggers, Tracks, anchors or zones, so it
        // wrote a file that looked complete and was a mesh dump. The crate that
        // did this export (`xrds-gltf`) has since been deleted outright.
        //
        // Note this is unrelated to `ExportApplication`/`ExportApk` below, which
        // only *copy* existing `.glb` assets and rewrite `asset_uri`. Importing
        // and using glTF assets is unaffected.
        EditorCommand::ExportApplication { output_dir } => {
            if let Some(active) = tasks.active_in_lane(TaskLane::Build) {
                state.pending_status = Some(format!("Busy: {}", active.label));
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

            // Queue the cargo build. The build output now streams into the task
            // log rather than the editor's own stdout, so a failed export can be
            // read without a terminal.
            let out_dir_str   = output_dir.clone();
            let workspace_str = workspace_root.to_string_lossy().into_owned();

            tasks.spawn_tagged(
                format!("Export application → {output_dir}"),
                TaskLane::Build,
                Some(task_tag::EXPORT_APP),
                move |ctx| {
                    ctx.set_detail("compiling xrds-app");
                    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
                    let mut cmd = std::process::Command::new(&cargo);
                    cmd.args(["build", "--release", "-p", "xrds-app"])
                        .current_dir(&workspace_str);

                    if !ctx.run_child(cmd).map_err(|e| format!("cargo: {e}"))? {
                        return Err("cargo build failed — see the task log".into());
                    }

                    ctx.set_detail("copying binary");
                    let exe_name = format!("xrds-app{}", std::env::consts::EXE_SUFFIX);
                    let src = std::path::Path::new(&workspace_str)
                        .join("target").join("release").join(&exe_name);
                    let dst = std::path::Path::new(&out_dir_str).join(&exe_name);
                    std::fs::copy(&src, &dst).map_err(|e| format!("Binary copy failed: {e}"))?;
                    Ok(format!("Exported to {out_dir_str}"))
                },
            );
            state.pending_status = Some("Building… (this may take a minute)".into());
            false
        }

        EditorCommand::ExportApk { output_dir } => {
            // Guard: another build already running or queued. Both exports share
            // the Build lane, so this also refuses to stack an APK build behind a
            // desktop one — they contend on cargo's target-dir lock.
            if let Some(active) = tasks.active_in_lane(TaskLane::Build) {
                state.pending_status = Some(format!("Busy: {}", active.label));
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

            let out_dir_str   = output_dir.clone();
            let staging_str   = staging.to_string_lossy().into_owned();
            let workspace_str = workspace_root.to_string_lossy().into_owned();
            let staging_errors = copy_errors.clone();

            tasks.spawn_tagged(
                format!("Export APK → {output_dir}"),
                TaskLane::Build,
                Some(task_tag::EXPORT_APK),
                move |ctx| {
                    // Surface asset staging problems in the build log — a scene
                    // that references missing files otherwise produces an APK with
                    // no models and no visible explanation.
                    for e in &staging_errors {
                        ctx.log(format!("[err] asset staging: {e}"));
                    }

                    ctx.set_detail("running build script");

                    #[cfg(target_os = "windows")]
                    let cmd = {
                        let script = std::path::Path::new(&workspace_str)
                            .join("android/quest/build.ps1");
                        let mut c = std::process::Command::new("powershell.exe");
                        c.args([
                            "-ExecutionPolicy", "Bypass",
                            "-File", script.to_str().unwrap_or(""),
                            "-SceneDir", &staging_str,
                        ]);
                        c.current_dir(&workspace_str);
                        c
                    };
                    #[cfg(not(target_os = "windows"))]
                    let cmd = {
                        let script = std::path::Path::new(&workspace_str)
                            .join("android/quest/build.sh");
                        let mut c = std::process::Command::new("bash");
                        c.args([
                            script.to_str().unwrap_or(""),
                            "--scene-dir", &staging_str,
                        ]);
                        c.current_dir(&workspace_str);
                        c
                    };

                    // The log goes to disk on every path out of here, success or
                    // failure — a build log that only survives a success is
                    // useless, since a failure is the only time it is read.
                    let flush = |ctx: &TaskContext| {
                        write_build_log(&ctx.log_snapshot(), &out_dir_str);
                    };

                    let ok = match ctx.run_child(cmd) {
                        Ok(ok) => ok,
                        Err(e) => {
                            ctx.log(format!("[err] {e}"));
                            flush(&ctx);
                            return Err(format!("Failed to start build script: {e}"));
                        }
                    };
                    if !ok {
                        ctx.log("[err] build script exited non-zero");
                        flush(&ctx);
                        return Err("Build script failed — see the task log".into());
                    }

                    ctx.log("--- build script finished ---");
                    ctx.set_detail("collecting APK");

                    let apk_src = std::path::Path::new(&workspace_str)
                        .join("android/quest/build/xrds-app.apk");
                    let apk_dst = std::path::Path::new(&out_dir_str).join("xrds-app.apk");

                    if !apk_src.exists() {
                        flush(&ctx);
                        return Err("Build script succeeded but xrds-app.apk was not produced.".into());
                    }
                    if let Err(e) = std::fs::copy(&apk_src, &apk_dst) {
                        flush(&ctx);
                        return Err(format!("Copy APK failed: {e}"));
                    }
                    if let Err(e) = write_install_scripts(std::path::Path::new(&out_dir_str)) {
                        ctx.log(format!("[warn] install script write failed: {e}"));
                    }

                    ctx.log(format!("APK exported to {out_dir_str}"));
                    flush(&ctx);
                    Ok(format!("APK exported to {out_dir_str}"))
                },
            );
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

    /// `.ktx2` must import as an EnvironmentMap, not a Texture.
    ///
    /// Skybox and IBL both require an `EnvironmentMap` asset, and the SDK ships its
    /// environment maps as `.ktx2`. While this mapped to `Texture`, importing the
    /// SDK's own `environment_maps/specular.ktx2` produced an asset neither feature
    /// would accept — so enabling a skybox failed document validation with nothing
    /// shown to the author, and the checkbox simply would not tick.
    ///
    /// The container can legitimately hold a plain texture, so this is a judgement
    /// about how `.ktx2` is used here rather than a fact about the format. Pinned
    /// with a test because the alternative reading is reasonable enough that
    /// someone will otherwise "fix" it back.
    /// Asserted against the real files in `assets/environment_maps/`, not a
    /// synthetic fixture: the point is that the header parse agrees with what the
    /// SDK actually ships, and a hand-built header would only prove the parser
    /// matches itself.
    #[test]
    fn shipped_environment_maps_are_detected_as_cubemaps() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("workspace root")
            .to_path_buf();

        for name in ["diffuse.ktx2", "specular.ktx2"] {
            let path = root.join("assets/environment_maps").join(name);
            let path = path.to_string_lossy();
            assert_eq!(
                ktx2_face_count(&path),
                Some(6),
                "{name} should report 6 faces",
            );
            assert_eq!(
                detect_asset_kind(&path),
                Some(XrdsSceneAssetKind::EnvironmentMap),
                "{name} must import as an EnvironmentMap or skybox and IBL cannot use it",
            );
        }
    }

    /// A `.ktx2` that is not a cubemap — or is not readable — must not be offered
    /// as a skybox. Falling back to `Texture` fails visibly in the picker; claiming
    /// EnvironmentMap would fail at render time on a headset instead.
    #[test]
    fn a_non_cubemap_ktx2_is_a_texture() {
        let dir = std::env::temp_dir().join("xrds_ktx2_kind_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("flat.ktx2");

        // Valid identifier, faceCount = 1.
        let mut header = vec![
            0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
        ];
        header.extend(std::iter::repeat(0u8).take(24)); // through layerCount
        header.extend(1u32.to_le_bytes()); // faceCount
        std::fs::write(&path, &header).unwrap();

        let path = path.to_string_lossy().into_owned();
        assert_eq!(ktx2_face_count(&path), Some(1));
        assert_eq!(detect_asset_kind(&path), Some(XrdsSceneAssetKind::Texture));

        // A file that is not KTX2 at all must not be mistaken for one.
        let bogus = dir.join("bogus.ktx2");
        std::fs::write(&bogus, b"not a ktx2 file at all, but long enough to read").unwrap();
        assert_eq!(ktx2_face_count(&bogus.to_string_lossy()), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_extensions_keep_their_kinds() {
        assert_eq!(detect_asset_kind("sky.hdr"), Some(XrdsSceneAssetKind::EnvironmentMap));
        // Material textures are the png/jpg family and must stay Texture, or the
        // texture slots stop offering them.
        for path in ["albedo.png", "normal.jpg", "rough.jpeg", "ao.webp"] {
            assert_eq!(
                detect_asset_kind(path),
                Some(XrdsSceneAssetKind::Texture),
                "{path} should still import as a Texture",
            );
        }
    }

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

    // ── Environment-map import contract ──────────────────────────────────
    // docs/editor-task-queue-and-hdr-conversion.md. `.exr` equirectangular is the
    // contract; everything else is refused with a reason an author can act on.

    /// A 2:1 OpenEXR is what ambientCG and Poly Haven actually ship.
    ///
    /// The fixture is generated rather than committed: a real 4K panorama is 31 MB,
    /// and the property under test is the header, not the pixels.
    #[test]
    fn an_equirectangular_exr_is_accepted() {
        let dir = std::env::temp_dir().join("xrds_env_contract_exr");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("sky.exr");
        // Values above 1.0, as a real HDR panorama has — the whole point of the format.
        let buf = image::Rgb32FImage::from_fn(64, 32, |x, _| image::Rgb([x as f32, 4.0, 0.5]));
        image::DynamicImage::ImageRgb32F(buf).save(&path).expect("write exr fixture");

        let src = classify_environment_source(path.to_str().unwrap())
            .expect("a 2:1 .exr is the format the contract exists to accept");
        assert_eq!(src, EnvironmentSource { width: 64, height: 32 });

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The mistake the refusal message exists for.
    ///
    /// An ambientCG download puts `..._HDR.exr` and `..._TONEMAPPED.jpg` in one
    /// folder. Measured on a real pair, 84.4% of the environment's light lives
    /// above 1.0 and is absent from the JPEG — so this is not a lesser input, it is
    /// the wrong one, and the message must point at the file the author has.
    #[test]
    fn an_ldr_image_is_refused_with_a_reason_that_names_the_fix() {
        for ext in ["jpg", "png", "webp"] {
            let err = classify_environment_source(&format!("some/DayEnvironment_TONEMAPPED.{ext}"))
                .expect_err("LDR cannot carry the values that light a scene");
            assert_eq!(err, EnvironmentSourceError::NotHighDynamicRange { ext: ext.into() });

            let msg = err.to_string();
            assert!(msg.contains("1.0"), "must say what is missing: {msg}");
            assert!(msg.contains(".exr"), "must name the file to use instead: {msg}");
        }
    }

    /// Refused before the file is opened — an LDR path that does not exist must
    /// still produce the format complaint, not a confusing I/O error.
    #[test]
    fn the_ldr_refusal_does_not_depend_on_reading_the_file() {
        let err = classify_environment_source("nowhere/at/all/sky.png").unwrap_err();
        assert!(matches!(err, EnvironmentSourceError::NotHighDynamicRange { .. }));
    }

    /// A cubemap face, a texture atlas, or a cropped panorama. Accepting one would
    /// wrap the wrong pixels around the sky with nothing said.
    #[test]
    fn a_non_2to1_image_is_refused_and_the_message_states_its_size() {
        let dir = std::env::temp_dir().join("xrds_env_contract_square");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("square.exr");
        let buf = image::Rgb32FImage::from_fn(64, 64, |_, _| image::Rgb([2.0, 2.0, 2.0]));
        image::DynamicImage::ImageRgb32F(buf).save(&path).expect("write exr fixture");

        let err = classify_environment_source(path.to_str().unwrap()).unwrap_err();
        assert_eq!(err, EnvironmentSourceError::NotEquirectangular { width: 64, height: 64 });
        assert!(err.to_string().contains("64×64"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `.exr` must reach the environment picker at all. It was absent from
    /// `detect_asset_kind`, so importing the one file an author is most likely to
    /// download was refused outright.
    #[test]
    fn exr_and_hdr_classify_as_environment_maps() {
        for p in ["a/sky.exr", "a/sky.EXR", "a/sky.hdr"] {
            assert_eq!(detect_asset_kind(p), Some(XrdsSceneAssetKind::EnvironmentMap),
                       "{p} should reach the environment picker");
        }
    }
}
