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

    Ok(())
}