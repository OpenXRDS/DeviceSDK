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

/// Convert a raw catalog URI (which may be an absolute path when the scene
/// has not yet been saved) to a path relative to the `assets/` root that
/// Bevy's `AssetServer` can accept.
///
/// Rules:
/// - If the path is already relative → return as-is.
/// - If it is absolute and contains `/assets/` → strip everything up to and
///   including that segment.
/// - Otherwise (absolute but no `/assets/` anchor) → return as-is and let
///   the caller handle the error.
pub(super) fn relativize_asset_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if !std::path::Path::new(&normalized).is_absolute() {
        return normalized;
    }
    if let Some(idx) = normalized.find("/assets/") {
        normalized[idx + "/assets/".len()..].to_string()
    } else {
        normalized
    }
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
}