use super::*;

impl XrdsSceneDocument {
    pub fn environment_map_assets(&self) -> impl Iterator<Item = &XrdsSceneAsset> {
        self.assets
            .iter()
            .filter(|asset| asset.kind == XrdsSceneAssetKind::EnvironmentMap)
    }

    pub fn environment_map_source_diagnostic(
        &self,
        asset_id: &str,
    ) -> Result<XrdsSceneAssetSourceDiagnostic, XrdsSceneAssetWorkflowError> {
        self.asset_source_diagnostic_with_kind(asset_id, XrdsSceneAssetKind::EnvironmentMap)
    }

    pub fn environment_map_source_diagnostics(&self) -> Vec<XrdsSceneAssetSourceDiagnostic> {
        self.environment_map_assets()
            .filter_map(|asset| self.environment_map_source_diagnostic(&asset.id).ok())
            .collect()
    }

    pub fn register_environment_map_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneAssetWorkflowError> {
        self.register_asset_with_kind(asset_id, uri, XrdsSceneAssetKind::EnvironmentMap)
    }

    pub fn ensure_environment_map_asset(
        &mut self,
        preferred_asset_id: Option<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneAssetWorkflowError> {
        self.ensure_asset_with_kind(preferred_asset_id, uri, XrdsSceneAssetKind::EnvironmentMap)
    }

    pub fn audio_assets(&self) -> impl Iterator<Item = &XrdsSceneAsset> {
        self.assets
            .iter()
            .filter(|asset| asset.kind == XrdsSceneAssetKind::Audio)
    }

    pub fn audio_source_diagnostic(
        &self,
        asset_id: &str,
    ) -> Result<XrdsSceneAssetSourceDiagnostic, XrdsSceneAssetWorkflowError> {
        self.asset_source_diagnostic_with_kind(asset_id, XrdsSceneAssetKind::Audio)
    }

    pub fn audio_source_diagnostics(&self) -> Vec<XrdsSceneAssetSourceDiagnostic> {
        self.audio_assets()
            .filter_map(|asset| self.audio_source_diagnostic(&asset.id).ok())
            .collect()
    }

    pub fn register_audio_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneAssetWorkflowError> {
        self.register_asset_with_kind(asset_id, uri, XrdsSceneAssetKind::Audio)
    }

    pub fn ensure_audio_asset(
        &mut self,
        preferred_asset_id: Option<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneAssetWorkflowError> {
        self.ensure_asset_with_kind(preferred_asset_id, uri, XrdsSceneAssetKind::Audio)
    }

    pub fn texture_assets(&self) -> impl Iterator<Item = &XrdsSceneAsset> {
        self.assets
            .iter()
            .filter(|asset| asset.kind == XrdsSceneAssetKind::Texture)
    }

    pub fn texture_source_diagnostic(
        &self,
        asset_id: &str,
    ) -> Result<XrdsSceneAssetSourceDiagnostic, XrdsSceneAssetWorkflowError> {
        self.asset_source_diagnostic_with_kind(asset_id, XrdsSceneAssetKind::Texture)
    }

    pub fn texture_source_diagnostics(&self) -> Vec<XrdsSceneAssetSourceDiagnostic> {
        self.texture_assets()
            .filter_map(|asset| self.texture_source_diagnostic(&asset.id).ok())
            .collect()
    }

    pub fn register_texture_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneAssetWorkflowError> {
        self.register_asset_with_kind(asset_id, uri, XrdsSceneAssetKind::Texture)
    }

    pub fn ensure_texture_asset(
        &mut self,
        preferred_asset_id: Option<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneAssetWorkflowError> {
        self.ensure_asset_with_kind(preferred_asset_id, uri, XrdsSceneAssetKind::Texture)
    }

    pub fn rebind_asset(
        &mut self,
        asset_id: &str,
        new_uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetRebindResult, XrdsSceneAssetWorkflowError> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        let new_uri = normalize_asset_uri(new_uri.into())?;

        let kind = self
            .asset(asset_id)
            .map(|asset| asset.kind)
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;

        let asset = self
            .asset_mut(asset_id)
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;
        let previous_uri = std::mem::replace(&mut asset.uri, new_uri.clone());
        let rebound_node_ids = if kind == XrdsSceneAssetKind::Gltf {
            self.rewrite_gltf_asset_fallback_uris(asset_id, &new_uri)
        } else {
            Vec::new()
        };

        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;

        Ok(XrdsSceneAssetRebindResult {
            asset_id: asset_id.to_string(),
            previous_uri,
            new_uri,
            rebound_node_ids,
        })
    }

    pub(crate) fn register_asset_with_kind(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
        kind: XrdsSceneAssetKind,
    ) -> Result<XrdsSceneAsset, XrdsSceneAssetWorkflowError> {
        let asset_id = normalize_asset_id(asset_id.into())?;
        let uri = normalize_asset_uri(uri.into())?;

        if self.asset(&asset_id).is_some() {
            return Err(XrdsSceneAssetWorkflowError::DuplicateAssetId(asset_id));
        }

        let asset = XrdsSceneAsset {
            id: asset_id,
            uri,
            kind,
        };

        self.assets.push(asset.clone());
        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;
        Ok(asset)
    }

    pub(crate) fn ensure_asset_with_kind(
        &mut self,
        preferred_asset_id: Option<String>,
        uri: impl Into<String>,
        kind: XrdsSceneAssetKind,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneAssetWorkflowError> {
        let uri = normalize_asset_uri(uri.into())?;

        if let Some(existing) = self.asset_by_uri_and_kind(&uri, kind).cloned() {
            return Ok(XrdsSceneAssetEnsureResult {
                asset: existing,
                created: false,
            });
        }

        let asset_id = match preferred_asset_id {
            Some(asset_id) => normalize_asset_id(asset_id)?,
            None => unique_generated_asset_id(self, asset_id_seed_from_uri(kind, &uri)),
        };

        let asset = self.register_asset_with_kind(asset_id, uri, kind)?;
        Ok(XrdsSceneAssetEnsureResult {
            asset,
            created: true,
        })
    }
}

pub(crate) fn normalize_asset_id(asset_id: String) -> Result<String, XrdsSceneAssetWorkflowError> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
    }

    Ok(asset_id.to_string())
}

pub(crate) fn normalize_asset_uri(uri: String) -> Result<String, XrdsSceneAssetWorkflowError> {
    let uri = uri.trim();
    if uri.is_empty() {
        return Err(XrdsSceneAssetWorkflowError::InvalidAssetUri);
    }

    Ok(uri.to_string())
}

pub(crate) fn unique_generated_asset_id(document: &XrdsSceneDocument, seed: String) -> String {
    if document.asset(&seed).is_none() {
        return seed;
    }

    let mut suffix = 2usize;
    loop {
        let candidate = format!("{seed}-{suffix}");
        if document.asset(&candidate).is_none() {
            return candidate;
        }
        suffix += 1;
    }
}

fn asset_id_seed_from_uri(kind: XrdsSceneAssetKind, uri: &str) -> String {
    let stem = std::path::Path::new(uri)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(match kind {
            XrdsSceneAssetKind::Gltf => "gltf",
            XrdsSceneAssetKind::Texture => "texture",
            XrdsSceneAssetKind::EnvironmentMap => "envmap",
            XrdsSceneAssetKind::Audio => "audio",
        });

    let mut slug = String::new();
    let mut previous_was_dash = false;
    for ch in stem.chars() {
        let normalized = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if previous_was_dash {
            None
        } else {
            Some('-')
        };

        if let Some(ch) = normalized {
            previous_was_dash = ch == '-';
            slug.push(ch);
        }
    }

    let kind_label = match kind {
        XrdsSceneAssetKind::Gltf => "gltf",
        XrdsSceneAssetKind::Texture => "texture",
        XrdsSceneAssetKind::EnvironmentMap => "envmap",
        XrdsSceneAssetKind::Audio => "audio",
    };
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("asset:{kind_label}")
    } else {
        format!("asset:{kind_label}-{slug}")
    }
}

impl XrdsSceneDocument {
    fn asset_source_diagnostic_with_kind(
        &self,
        asset_id: &str,
        expected_kind: XrdsSceneAssetKind,
    ) -> Result<XrdsSceneAssetSourceDiagnostic, XrdsSceneAssetWorkflowError> {
        let asset = self
            .asset(asset_id)
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;
        if asset.kind != expected_kind {
            return Err(XrdsSceneAssetWorkflowError::AssetKindMismatch {
                asset_id: asset.id.clone(),
                expected: expected_kind,
                found: asset.kind,
            });
        }

        let (status, resolved_path, message) =
            validate_binary_asset_source(&asset.uri, expected_kind);

        Ok(XrdsSceneAssetSourceDiagnostic {
            asset_id: asset.id.clone(),
            asset_kind: asset.kind,
            resolved_asset_uri: asset.uri.clone(),
            resolved_path,
            status,
            message,
        })
    }
}

fn resolve_binary_asset_document_path(path: &str) -> Option<std::path::PathBuf> {
    let document_path = Path::new(path);

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

fn validate_binary_asset_source(
    path: &str,
    kind: XrdsSceneAssetKind,
) -> (
    XrdsSceneAssetSourceDiagnosticStatus,
    Option<std::path::PathBuf>,
    Option<String>,
) {
    let path = path.trim();
    if path.is_empty() {
        return (
            XrdsSceneAssetSourceDiagnosticStatus::EmptyAssetUri,
            None,
            Some(format!("{} asset path is empty", asset_kind_label(kind))),
        );
    }

    let Some(document_path) = resolve_binary_asset_document_path(path) else {
        return (
            XrdsSceneAssetSourceDiagnosticStatus::MissingFile,
            None,
            Some(format!(
                "{} asset '{}' was not found",
                asset_kind_label(kind),
                path
            )),
        );
    };

    let Some(extension) = document_path.extension().and_then(|ext| ext.to_str()) else {
        return (
            XrdsSceneAssetSourceDiagnosticStatus::InvalidExtension,
            Some(document_path),
            Some(format!(
                "{} asset '{}' has no file extension",
                asset_kind_label(kind),
                path
            )),
        );
    };

    let extension = extension.to_ascii_lowercase();
    if !supported_binary_asset_extensions(kind).contains(&extension.as_str()) {
        return (
            XrdsSceneAssetSourceDiagnosticStatus::InvalidExtension,
            Some(document_path),
            Some(format!(
                "{} asset '{}' must use one of: {}",
                asset_kind_label(kind),
                path,
                supported_binary_asset_extensions(kind).join(", ")
            )),
        );
    }

    (
        XrdsSceneAssetSourceDiagnosticStatus::Valid,
        Some(document_path),
        None,
    )
}

fn asset_kind_label(kind: XrdsSceneAssetKind) -> &'static str {
    match kind {
        XrdsSceneAssetKind::Gltf => "glTF",
        XrdsSceneAssetKind::Texture => "texture",
        XrdsSceneAssetKind::EnvironmentMap => "environment map",
        XrdsSceneAssetKind::Audio => "audio",
    }
}

fn supported_binary_asset_extensions(kind: XrdsSceneAssetKind) -> &'static [&'static str] {
    match kind {
        XrdsSceneAssetKind::Texture => &[
            "png", "jpg", "jpeg", "webp", "bmp", "tga", "gif", "hdr", "exr", "ktx2", "basis", "dds",
        ],
        XrdsSceneAssetKind::EnvironmentMap => &["hdr", "exr", "ktx2", "dds"],
        XrdsSceneAssetKind::Audio => &["mp3", "ogg", "wav", "flac"],
        XrdsSceneAssetKind::Gltf => &["gltf", "glb"],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneAssetSourceDiagnostic {
    pub asset_id: String,
    pub asset_kind: XrdsSceneAssetKind,
    pub resolved_asset_uri: String,
    pub resolved_path: Option<std::path::PathBuf>,
    pub status: XrdsSceneAssetSourceDiagnosticStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneAssetSourceDiagnosticStatus {
    Valid,
    EmptyAssetUri,
    MissingFile,
    InvalidExtension,
}
