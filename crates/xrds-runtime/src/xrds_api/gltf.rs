use super::*;
use std::path::{Path, PathBuf};

fn resolve_gltf_document_path(path: &str) -> Option<PathBuf> {
    let document_path = path.split('#').next().unwrap_or(path);
    let document_path = Path::new(document_path);

    let candidates = if document_path.is_absolute() {
        vec![document_path.to_path_buf()]
    } else {
        vec![
            document_path.to_path_buf(),
            Path::new("assets").join(document_path),
        ]
    };

    candidates.into_iter().find(|candidate| candidate.is_file())
}

/// The asset root directory the runtime's `AssetServer` was configured with.
/// Set once by `build_bevy_app`; consulted by `relativize_asset_path`.
static CONFIGURED_ASSET_ROOT: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// Record the AssetServer root so `relativize_asset_path` can strip exactly
/// that prefix (and nothing else) from absolute asset paths.
pub(crate) fn set_configured_asset_root(root: Option<&str>) {
    let normalized = root.map(|r| r.replace('\\', "/").trim_end_matches('/').to_string());
    *CONFIGURED_ASSET_ROOT.write().unwrap() = normalized;
}

/// Strip `root` from the front of `normalized` (both forward-slash form).
/// Case-insensitive on Windows. Returns None if `normalized` is not under `root`.
fn strip_asset_root_prefix(normalized: &str, root: &str) -> Option<String> {
    let matches_prefix = if cfg!(windows) {
        normalized.len() > root.len()
            && normalized[..root.len()].eq_ignore_ascii_case(root)
    } else {
        normalized.starts_with(root)
    };
    if !matches_prefix {
        return None;
    }
    let rest = normalized[root.len()..].trim_start_matches('/');
    (!rest.is_empty()).then(|| rest.to_string())
}

/// Convert a raw catalog URI (which may be an absolute path when the scene
/// has not yet been saved) to a path that Bevy's `AssetServer` can accept.
///
/// Rules:
/// - If the path is already relative → return as-is (resolved against the
///   AssetServer root).
/// - If it is absolute and lies under the configured asset root → strip the
///   root prefix so it resolves as a normal relative asset path.
/// - Otherwise → keep the absolute path. With `UnapprovedPathMode::Allow`
///   the file reader loads absolute paths directly; anchoring on an arbitrary
///   `/assets/` segment (the old heuristic) rewrote foreign paths like
///   `C:/…/example/assets/foo.glb` into the app's own asset root, where the
///   file does not exist.
pub(super) fn relativize_asset_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if !std::path::Path::new(&normalized).is_absolute() {
        return normalized;
    }
    if let Some(root) = CONFIGURED_ASSET_ROOT.read().unwrap().as_deref() {
        if let Some(rel) = strip_asset_root_prefix(&normalized, root) {
            return rel;
        }
    }
    normalized
}


/// Bevy's GltfLoader rejects any mesh primitive that has more than this many
/// morph targets. We check this before spawning so that the error is surfaced
/// via the normal XRDS validation path instead of crashing the wgpu encoder.
const BEVY_MAX_MORPH_TARGETS: usize = 64;

pub(super) fn validate_gltf_source(path: &str, scene_index: usize) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("glTF asset path is empty".to_string());
    }

    let Some(document_path) = resolve_gltf_document_path(path) else {
        return Err(format!("glTF asset '{path}' was not found"));
    };

    let Some(extension) = document_path.extension().and_then(|ext| ext.to_str()) else {
        return Err(format!("glTF asset '{path}' has no file extension"));
    };

    if !matches!(extension.to_ascii_lowercase().as_str(), "gltf" | "glb") {
        return Err(format!("glTF asset '{path}' must end in .gltf or .glb"));
    }

    let gltf = ::gltf::Gltf::open(&document_path)
        .map_err(|error| format!("failed to parse glTF asset '{path}': {error}"))?;

    let scene_count = gltf.scenes().count();
    if scene_count == 0 {
        return Err(format!("glTF asset '{path}' contains no scenes"));
    }

    if scene_index >= scene_count {
        return Err(format!(
            "glTF asset '{path}' does not contain scene index {scene_index} (available: 0..{})",
            scene_count - 1
        ));
    }

    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let count = primitive.morph_targets().count();
            if count > BEVY_MAX_MORPH_TARGETS {
                return Err(format!(
                    "glTF asset '{path}' mesh '{}' has {count} morph targets \
                     (Bevy limit is {BEVY_MAX_MORPH_TARGETS})",
                    mesh.name().unwrap_or("<unnamed>")
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/animated")
    }

    fn sample(name: &str) -> String {
        samples_dir().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn buster_drone_validates() {
        let result = validate_gltf_source(&sample("buster_drone.glb"), 0);
        assert!(result.is_ok(), "buster_drone.glb should validate: {result:?}");
    }

    #[test]
    fn phoenix_bird_validates() {
        let result = validate_gltf_source(&sample("phoenix_bird.glb"), 0);
        assert!(result.is_ok(), "phoenix_bird.glb should validate: {result:?}");
    }

    // magic_wand_rejected_too_many_morph_targets test removed — file was deleted.

    #[test]
    fn relative_paths_pass_through_unchanged() {
        assert_eq!(relativize_asset_path("phoenix_bird.glb"), "phoenix_bird.glb");
        assert_eq!(
            relativize_asset_path("models\\animated\\phoenix_bird.glb"),
            "models/animated/phoenix_bird.glb"
        );
    }

    #[test]
    fn strip_asset_root_prefix_strips_only_under_root() {
        // Under the root → stripped.
        assert_eq!(
            strip_asset_root_prefix("F:/ws/assets/models/a.glb", "F:/ws/assets"),
            Some("models/a.glb".to_string())
        );
        // Case difference on Windows drive/dir → still stripped there.
        if cfg!(windows) {
            assert_eq!(
                strip_asset_root_prefix("f:/WS/Assets/a.glb", "F:/ws/assets"),
                Some("a.glb".to_string())
            );
        }
        // A foreign absolute path containing "/assets/" is NOT under the root
        // and must stay intact (the old heuristic broke this case).
        assert_eq!(
            strip_asset_root_prefix("C:/Users/u/example/assets/a.glb", "F:/ws/assets"),
            None
        );
        // Root itself (no trailing file) → None, not an empty string.
        assert_eq!(strip_asset_root_prefix("F:/ws/assets", "F:/ws/assets"), None);
    }
}