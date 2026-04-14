use super::*;

impl XrdsSceneDocument {
    pub fn resolve_gltf_asset(&self, asset: &XrdsSceneGltfAsset) -> XrdsResolvedSceneGltfAsset {
        let asset_id = asset
            .asset_id
            .as_ref()
            .map(|asset_id| asset_id.trim())
            .filter(|asset_id| !asset_id.is_empty())
            .map(ToOwned::to_owned);

        if let Some(asset_id) = asset_id.as_deref() {
            if let Some(catalog_asset) = self.asset(asset_id) {
                if catalog_asset.kind == XrdsSceneAssetKind::Gltf {
                    return XrdsResolvedSceneGltfAsset {
                        asset_id: Some(asset_id.to_string()),
                        asset_uri: catalog_asset.uri.clone(),
                        scene_index: asset.scene_index,
                        export_policy: asset.export_policy,
                        source: XrdsSceneAssetResolutionSource::Catalog,
                    };
                }
            }
        }

        XrdsResolvedSceneGltfAsset {
            asset_id,
            asset_uri: asset.asset_uri.clone(),
            scene_index: asset.scene_index,
            export_policy: asset.export_policy,
            source: XrdsSceneAssetResolutionSource::EmbeddedFallback,
        }
    }

    pub fn gltf_node_health(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<XrdsSceneGltfNodeHealth, XrdsSceneAssetWorkflowError> {
        let node = self
            .node(node_id)
            .ok_or(XrdsSceneAssetWorkflowError::NodeNotFound(node_id))?;

        let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
            return Err(XrdsSceneAssetWorkflowError::NodeIsNotGltfAsset(node_id));
        };

        let resolved = self.resolve_gltf_asset(asset);
        let status = match (&resolved.asset_id, resolved.source) {
            (Some(_), XrdsSceneAssetResolutionSource::Catalog) => {
                XrdsSceneGltfNodeHealthStatus::CatalogResolved
            }
            (Some(_), XrdsSceneAssetResolutionSource::EmbeddedFallback) => {
                XrdsSceneGltfNodeHealthStatus::MissingCatalogAsset
            }
            (None, XrdsSceneAssetResolutionSource::EmbeddedFallback) => {
                XrdsSceneGltfNodeHealthStatus::DetachedFallback
            }
            (None, XrdsSceneAssetResolutionSource::Catalog) => {
                XrdsSceneGltfNodeHealthStatus::CatalogResolved
            }
        };

        Ok(XrdsSceneGltfNodeHealth {
            node_id,
            asset_id: asset.asset_id.clone(),
            stored_asset_uri: asset.asset_uri.clone(),
            resolved_asset_uri: resolved.asset_uri,
            scene_index: asset.scene_index,
            status,
        })
    }

    pub fn gltf_node_healths(&self) -> Vec<XrdsSceneGltfNodeHealth> {
        self.nodes
            .iter()
            .filter_map(|node| match node.payload {
                XrdsSceneNodePayload::GltfAsset(_) => self.gltf_node_health(node.id).ok(),
                _ => None,
            })
            .collect()
    }

    pub fn gltf_source_diagnostic(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<XrdsSceneGltfSourceDiagnostic, XrdsSceneAssetWorkflowError> {
        let node = self
            .node(node_id)
            .ok_or(XrdsSceneAssetWorkflowError::NodeNotFound(node_id))?;

        let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
            return Err(XrdsSceneAssetWorkflowError::NodeIsNotGltfAsset(node_id));
        };

        let resolved = self.resolve_gltf_asset(asset);
        let (status, resolved_path, message) = validate_resolved_gltf_source(&resolved);

        Ok(XrdsSceneGltfSourceDiagnostic {
            node_id,
            asset_id: resolved.asset_id,
            resolved_asset_uri: resolved.asset_uri,
            scene_index: resolved.scene_index,
            resolution_source: resolved.source,
            resolved_path,
            status,
            message,
        })
    }

    pub fn gltf_source_diagnostics(&self) -> Vec<XrdsSceneGltfSourceDiagnostic> {
        self.nodes
            .iter()
            .filter_map(|node| match node.payload {
                XrdsSceneNodePayload::GltfAsset(_) => self.gltf_source_diagnostic(node.id).ok(),
                _ => None,
            })
            .collect()
    }

    pub fn asset_usage(
        &self,
        asset_id: &str,
    ) -> Result<XrdsSceneAssetUsage, XrdsSceneAssetWorkflowError> {
        let asset = self
            .asset(asset_id)
            .cloned()
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;
        let referenced_node_ids = match asset.kind {
            XrdsSceneAssetKind::Gltf => self.gltf_asset_reference_node_ids(&asset.id),
            XrdsSceneAssetKind::Texture => self.material_texture_reference_node_ids(&asset.id),
            XrdsSceneAssetKind::EnvironmentMap => Vec::new(),
        };

        Ok(XrdsSceneAssetUsage {
            asset,
            referenced_node_ids,
        })
    }

    pub fn asset_usages(&self) -> Vec<XrdsSceneAssetUsage> {
        self.assets
            .iter()
            .cloned()
            .map(|asset| XrdsSceneAssetUsage {
                referenced_node_ids: match asset.kind {
                    XrdsSceneAssetKind::Gltf => self.gltf_asset_reference_node_ids(&asset.id),
                    XrdsSceneAssetKind::Texture => {
                        self.material_texture_reference_node_ids(&asset.id)
                    }
                    XrdsSceneAssetKind::EnvironmentMap => Vec::new(),
                },
                asset,
            })
            .collect()
    }

    pub fn asset_diagnostic_entries(&self) -> Vec<XrdsSceneAssetDiagnosticEntry> {
        self.asset_diagnostics().ui_entries()
    }

    pub fn asset_diagnostics(&self) -> XrdsSceneAssetDiagnostics {
        let node_healths = self.gltf_node_healths();
        let source_diagnostics = self.gltf_source_diagnostics();
        let texture_source_diagnostics = self.texture_source_diagnostics();
        let environment_map_source_diagnostics = self.environment_map_source_diagnostics();
        let asset_usages = self.asset_usages();

        let catalog_resolved_node_ids = node_healths
            .iter()
            .filter(|health| health.status == XrdsSceneGltfNodeHealthStatus::CatalogResolved)
            .map(|health| health.node_id)
            .collect();
        let missing_catalog_node_ids = node_healths
            .iter()
            .filter(|health| health.status == XrdsSceneGltfNodeHealthStatus::MissingCatalogAsset)
            .map(|health| health.node_id)
            .collect();
        let detached_fallback_node_ids = node_healths
            .iter()
            .filter(|health| health.status == XrdsSceneGltfNodeHealthStatus::DetachedFallback)
            .map(|health| health.node_id)
            .collect();
        let unused_asset_ids = asset_usages
            .iter()
            .filter(|usage| usage.referenced_node_ids.is_empty())
            .map(|usage| usage.asset.id.clone())
            .collect();
        let valid_source_node_ids = source_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status == XrdsSceneGltfSourceDiagnosticStatus::Valid)
            .map(|diagnostic| diagnostic.node_id)
            .collect();
        let invalid_source_node_ids = source_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status != XrdsSceneGltfSourceDiagnosticStatus::Valid)
            .map(|diagnostic| diagnostic.node_id)
            .collect();
        let valid_environment_map_asset_ids = environment_map_source_diagnostics
            .iter()
            .filter(|d| d.status == XrdsSceneAssetSourceDiagnosticStatus::Valid)
            .map(|d| d.asset_id.clone())
            .collect();
        let invalid_environment_map_asset_ids = environment_map_source_diagnostics
            .iter()
            .filter(|d| d.status != XrdsSceneAssetSourceDiagnosticStatus::Valid)
            .map(|d| d.asset_id.clone())
            .collect();
        let valid_texture_asset_ids = texture_source_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status == XrdsSceneAssetSourceDiagnosticStatus::Valid)
            .map(|diagnostic| diagnostic.asset_id.clone())
            .collect();
        let invalid_texture_asset_ids = texture_source_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.status != XrdsSceneAssetSourceDiagnosticStatus::Valid)
            .map(|diagnostic| diagnostic.asset_id.clone())
            .collect();

        XrdsSceneAssetDiagnostics {
            node_healths,
            source_diagnostics,
            texture_source_diagnostics,
            environment_map_source_diagnostics,
            asset_usages,
            catalog_resolved_node_ids,
            missing_catalog_node_ids,
            detached_fallback_node_ids,
            valid_source_node_ids,
            invalid_source_node_ids,
            valid_texture_asset_ids,
            invalid_texture_asset_ids,
            valid_environment_map_asset_ids,
            invalid_environment_map_asset_ids,
            unused_asset_ids,
        }
    }

    pub fn register_gltf_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneAssetWorkflowError> {
        self.register_asset_with_kind(asset_id, uri, XrdsSceneAssetKind::Gltf)
    }

    pub fn ensure_gltf_asset(
        &mut self,
        preferred_asset_id: Option<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneAssetWorkflowError> {
        self.ensure_asset_with_kind(preferred_asset_id, uri, XrdsSceneAssetKind::Gltf)
    }

    pub fn place_gltf_asset(
        &mut self,
        placement: XrdsSceneGltfPlacement,
    ) -> Result<XrdsSceneNodeId, XrdsSceneAssetWorkflowError> {
        let asset_id = placement.asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        let Some(asset) = self.asset(asset_id).cloned() else {
            return Err(XrdsSceneAssetWorkflowError::AssetNotFound(
                asset_id.to_string(),
            ));
        };

        if asset.kind != XrdsSceneAssetKind::Gltf {
            return Err(XrdsSceneAssetWorkflowError::AssetKindMismatch {
                asset_id: asset.id,
                expected: XrdsSceneAssetKind::Gltf,
                found: asset.kind,
            });
        }

        if let Some(parent_id) = placement.parent_id {
            if self.node(parent_id).is_none() {
                return Err(XrdsSceneAssetWorkflowError::ParentNotFound(parent_id));
            }
        }

        let node_id = placement
            .node_id
            .unwrap_or_else(|| self.next_available_node_id());
        if self.node(node_id).is_some() {
            return Err(XrdsSceneAssetWorkflowError::NodeIdInUse(node_id));
        }

        let mut editor = placement.editor;
        editor.set_asset_id(Some(asset.id.clone()));

        self.nodes.push(XrdsSceneNode {
            id: node_id,
            parent_id: placement.parent_id,
            name: placement.name,
            enabled: placement.enabled,
            visible: placement.visible,
            transform: placement.transform,
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some(asset.id),
                asset_uri: asset.uri,
                scene_index: placement.scene_index,
                export_policy: placement.export_policy,
            }),
            editor,
        });

        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;
        Ok(node_id)
    }

    pub fn retarget_gltf_asset(
        &mut self,
        node_id: XrdsSceneNodeId,
        asset_id: &str,
        scene_index: usize,
    ) -> Result<(), XrdsSceneAssetWorkflowError> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        let Some(asset) = self.asset(asset_id).cloned() else {
            return Err(XrdsSceneAssetWorkflowError::AssetNotFound(
                asset_id.to_string(),
            ));
        };

        if asset.kind != XrdsSceneAssetKind::Gltf {
            return Err(XrdsSceneAssetWorkflowError::AssetKindMismatch {
                asset_id: asset.id,
                expected: XrdsSceneAssetKind::Gltf,
                found: asset.kind,
            });
        }

        let Some(node) = self.node_mut(node_id) else {
            return Err(XrdsSceneAssetWorkflowError::NodeNotFound(node_id));
        };

        let XrdsSceneNodePayload::GltfAsset(gltf_asset) = &mut node.payload else {
            return Err(XrdsSceneAssetWorkflowError::NodeIsNotGltfAsset(node_id));
        };

        gltf_asset.asset_id = Some(asset.id.clone());
        gltf_asset.asset_uri = asset.uri;
        gltf_asset.scene_index = scene_index;
        node.editor.set_asset_id(Some(asset.id));

        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;
        Ok(())
    }

    pub fn remove_asset(
        &mut self,
        asset_id: &str,
        policy: XrdsSceneAssetRemovalPolicy,
    ) -> Result<XrdsSceneAssetRemovalResult, XrdsSceneAssetWorkflowError> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        let referenced_node_ids = self.gltf_asset_reference_node_ids(asset_id);
        if policy == XrdsSceneAssetRemovalPolicy::RejectIfReferenced
            && !referenced_node_ids.is_empty()
        {
            return Err(XrdsSceneAssetWorkflowError::AssetInUse {
                asset_id: asset_id.to_string(),
                node_ids: referenced_node_ids,
            });
        }

        let asset_index = self
            .assets
            .iter()
            .position(|asset| asset.id == asset_id)
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;
        let removed_asset = self.assets.remove(asset_index);

        let detached_node_ids = if policy == XrdsSceneAssetRemovalPolicy::DetachReferencingNodes {
            self.detach_gltf_asset_references(asset_id, &removed_asset.uri)
        } else {
            Vec::new()
        };

        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;

        Ok(XrdsSceneAssetRemovalResult {
            removed_asset,
            detached_node_ids,
        })
    }

    pub fn rebind_gltf_asset(
        &mut self,
        asset_id: &str,
        new_uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetRebindResult, XrdsSceneAssetWorkflowError> {
        let asset_id = asset_id.trim();
        let Some(asset) = self.asset(asset_id) else {
            return Err(XrdsSceneAssetWorkflowError::AssetNotFound(
                asset_id.to_string(),
            ));
        };
        if asset.kind != XrdsSceneAssetKind::Gltf {
            return Err(XrdsSceneAssetWorkflowError::AssetKindMismatch {
                asset_id: asset.id.clone(),
                expected: XrdsSceneAssetKind::Gltf,
                found: asset.kind,
            });
        }

        self.rebind_asset(asset_id, new_uri)
    }

    pub fn rename_asset_id(
        &mut self,
        asset_id: &str,
        new_asset_id: impl Into<String>,
    ) -> Result<XrdsSceneAssetRenameResult, XrdsSceneAssetWorkflowError> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        let new_asset_id = new_asset_id.into();
        let new_asset_id = new_asset_id.trim();
        if new_asset_id.is_empty() {
            return Err(XrdsSceneAssetWorkflowError::InvalidAssetId);
        }

        if asset_id == new_asset_id {
            return Ok(XrdsSceneAssetRenameResult {
                previous_asset_id: asset_id.to_string(),
                new_asset_id: new_asset_id.to_string(),
                rewritten_node_ids: Vec::new(),
            });
        }

        if self.asset(new_asset_id).is_some() {
            return Err(XrdsSceneAssetWorkflowError::DuplicateAssetId(
                new_asset_id.to_string(),
            ));
        }

        let asset = self
            .asset_mut(asset_id)
            .ok_or_else(|| XrdsSceneAssetWorkflowError::AssetNotFound(asset_id.to_string()))?;

        asset.id = new_asset_id.to_string();
        let rewritten_node_ids = self.rewrite_gltf_asset_ids(asset_id, new_asset_id);

        self.validate()
            .map_err(XrdsSceneAssetWorkflowError::Validation)?;

        Ok(XrdsSceneAssetRenameResult {
            previous_asset_id: asset_id.to_string(),
            new_asset_id: new_asset_id.to_string(),
            rewritten_node_ids,
        })
    }

    fn detach_gltf_asset_references(
        &mut self,
        asset_id: &str,
        fallback_uri: &str,
    ) -> Vec<XrdsSceneNodeId> {
        let mut detached_node_ids = Vec::new();

        for node in &mut self.nodes {
            let XrdsSceneNodePayload::GltfAsset(asset) = &mut node.payload else {
                continue;
            };

            if asset.asset_id.as_deref() != Some(asset_id) {
                continue;
            }

            asset.asset_id = None;
            asset.asset_uri = fallback_uri.to_string();
            node.editor.set_asset_id(None);
            detached_node_ids.push(node.id);
        }

        detached_node_ids
    }

    pub(crate) fn rewrite_gltf_asset_fallback_uris(
        &mut self,
        asset_id: &str,
        new_uri: &str,
    ) -> Vec<XrdsSceneNodeId> {
        let mut rebound_node_ids = Vec::new();

        for node in &mut self.nodes {
            let XrdsSceneNodePayload::GltfAsset(asset) = &mut node.payload else {
                continue;
            };

            if asset.asset_id.as_deref() != Some(asset_id) {
                continue;
            }

            asset.asset_uri = new_uri.to_string();
            rebound_node_ids.push(node.id);
        }

        rebound_node_ids
    }

    fn rewrite_gltf_asset_ids(
        &mut self,
        previous_asset_id: &str,
        new_asset_id: &str,
    ) -> Vec<XrdsSceneNodeId> {
        let mut rewritten_node_ids = Vec::new();

        for node in &mut self.nodes {
            let XrdsSceneNodePayload::GltfAsset(asset) = &mut node.payload else {
                continue;
            };

            if asset.asset_id.as_deref() != Some(previous_asset_id) {
                continue;
            }

            asset.asset_id = Some(new_asset_id.to_string());
            node.editor.set_asset_id(Some(new_asset_id.to_string()));
            rewritten_node_ids.push(node.id);
        }

        rewritten_node_ids
    }
}

fn resolve_gltf_document_path(path: &str) -> Option<std::path::PathBuf> {
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

fn validate_resolved_gltf_source(
    resolved: &XrdsResolvedSceneGltfAsset,
) -> (
    XrdsSceneGltfSourceDiagnosticStatus,
    Option<std::path::PathBuf>,
    Option<String>,
) {
    let path = resolved.asset_uri.trim();
    if path.is_empty() {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::EmptyAssetUri,
            None,
            Some("glTF asset path is empty".to_string()),
        );
    }

    let Some(document_path) = resolve_gltf_document_path(path) else {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::MissingFile,
            None,
            Some(format!("glTF asset '{path}' was not found")),
        );
    };

    let Some(extension) = document_path.extension().and_then(|ext| ext.to_str()) else {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::InvalidExtension,
            Some(document_path),
            Some(format!("glTF asset '{path}' has no file extension")),
        );
    };

    if !matches!(extension.to_ascii_lowercase().as_str(), "gltf" | "glb") {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::InvalidExtension,
            Some(document_path),
            Some(format!("glTF asset '{path}' must end in .gltf or .glb")),
        );
    }

    let gltf = match ::gltf::Gltf::open(&document_path) {
        Ok(gltf) => gltf,
        Err(error) => {
            return (
                XrdsSceneGltfSourceDiagnosticStatus::ParseError,
                Some(document_path),
                Some(format!("failed to parse glTF asset '{path}': {error}")),
            )
        }
    };

    let scene_count = gltf.scenes().count();
    if scene_count == 0 {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::NoScenes,
            Some(document_path),
            Some(format!("glTF asset '{path}' contains no scenes")),
        );
    }

    if resolved.scene_index >= scene_count {
        return (
            XrdsSceneGltfSourceDiagnosticStatus::MissingSceneIndex,
            Some(document_path),
            Some(format!(
                "glTF asset '{path}' does not contain scene index {} (available: 0..{})",
                resolved.scene_index,
                scene_count - 1
            )),
        );
    }

    (
        XrdsSceneGltfSourceDiagnosticStatus::Valid,
        Some(document_path),
        None,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsResolvedSceneGltfAsset {
    pub asset_id: Option<String>,
    pub asset_uri: String,
    pub scene_index: usize,
    pub export_policy: XrdsGltfAssetExportPolicy,
    pub source: XrdsSceneAssetResolutionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneAssetResolutionSource {
    Catalog,
    EmbeddedFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneGltfNodeHealth {
    pub node_id: XrdsSceneNodeId,
    pub asset_id: Option<String>,
    pub stored_asset_uri: String,
    pub resolved_asset_uri: String,
    pub scene_index: usize,
    pub status: XrdsSceneGltfNodeHealthStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneGltfNodeHealthStatus {
    CatalogResolved,
    MissingCatalogAsset,
    DetachedFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneGltfSourceDiagnostic {
    pub node_id: XrdsSceneNodeId,
    pub asset_id: Option<String>,
    pub resolved_asset_uri: String,
    pub scene_index: usize,
    pub resolution_source: XrdsSceneAssetResolutionSource,
    pub resolved_path: Option<std::path::PathBuf>,
    pub status: XrdsSceneGltfSourceDiagnosticStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneGltfSourceDiagnosticStatus {
    Valid,
    EmptyAssetUri,
    MissingFile,
    InvalidExtension,
    ParseError,
    NoScenes,
    MissingSceneIndex,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsSceneAssetUsage {
    pub asset: XrdsSceneAsset,
    pub referenced_node_ids: Vec<XrdsSceneNodeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsSceneAssetDiagnostics {
    pub node_healths: Vec<XrdsSceneGltfNodeHealth>,
    pub source_diagnostics: Vec<XrdsSceneGltfSourceDiagnostic>,
    pub texture_source_diagnostics: Vec<XrdsSceneAssetSourceDiagnostic>,
    pub environment_map_source_diagnostics: Vec<XrdsSceneAssetSourceDiagnostic>,
    pub asset_usages: Vec<XrdsSceneAssetUsage>,
    pub catalog_resolved_node_ids: Vec<XrdsSceneNodeId>,
    pub missing_catalog_node_ids: Vec<XrdsSceneNodeId>,
    pub detached_fallback_node_ids: Vec<XrdsSceneNodeId>,
    pub valid_source_node_ids: Vec<XrdsSceneNodeId>,
    pub invalid_source_node_ids: Vec<XrdsSceneNodeId>,
    pub valid_texture_asset_ids: Vec<String>,
    pub invalid_texture_asset_ids: Vec<String>,
    pub valid_environment_map_asset_ids: Vec<String>,
    pub invalid_environment_map_asset_ids: Vec<String>,
    pub unused_asset_ids: Vec<String>,
}

impl XrdsSceneAssetDiagnostics {
    pub fn ui_entries(&self) -> Vec<XrdsSceneAssetDiagnosticEntry> {
        let mut entries = Vec::new();

        for diagnostic in &self.source_diagnostics {
            if diagnostic.status == XrdsSceneGltfSourceDiagnosticStatus::Valid {
                continue;
            }

            entries.push(XrdsSceneAssetDiagnosticEntry {
                subject: XrdsSceneAssetDiagnosticSubject::Node(diagnostic.node_id),
                severity: XrdsSceneAssetDiagnosticSeverity::Error,
                title: "glTF source issue".to_string(),
                detail: diagnostic
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("glTF node {:?} has an invalid source", diagnostic.node_id)),
            });
        }

        for diagnostic in &self.environment_map_source_diagnostics {
            if diagnostic.status == XrdsSceneAssetSourceDiagnosticStatus::Valid {
                continue;
            }

            entries.push(XrdsSceneAssetDiagnosticEntry {
                subject: XrdsSceneAssetDiagnosticSubject::Asset {
                    asset_id: diagnostic.asset_id.clone(),
                    kind: diagnostic.asset_kind,
                },
                severity: XrdsSceneAssetDiagnosticSeverity::Error,
                title: "Environment map source issue".to_string(),
                detail: diagnostic.message.clone().unwrap_or_else(|| {
                    format!(
                        "Environment map asset '{}' has an invalid source",
                        diagnostic.asset_id
                    )
                }),
            });
        }

        for diagnostic in &self.texture_source_diagnostics {
            if diagnostic.status == XrdsSceneAssetSourceDiagnosticStatus::Valid {
                continue;
            }

            entries.push(XrdsSceneAssetDiagnosticEntry {
                subject: XrdsSceneAssetDiagnosticSubject::Asset {
                    asset_id: diagnostic.asset_id.clone(),
                    kind: diagnostic.asset_kind,
                },
                severity: XrdsSceneAssetDiagnosticSeverity::Error,
                title: "Texture source issue".to_string(),
                detail: diagnostic.message.clone().unwrap_or_else(|| {
                    format!("Texture asset '{}' has an invalid source", diagnostic.asset_id)
                }),
            });
        }

        for asset_id in &self.unused_asset_ids {
            entries.push(XrdsSceneAssetDiagnosticEntry {
                subject: XrdsSceneAssetDiagnosticSubject::Asset {
                    asset_id: asset_id.clone(),
                    kind: self
                        .asset_usages
                        .iter()
                        .find(|usage| usage.asset.id == *asset_id)
                        .map(|usage| usage.asset.kind)
                        .unwrap_or(XrdsSceneAssetKind::Gltf),
                },
                severity: XrdsSceneAssetDiagnosticSeverity::Info,
                title: "Unused asset".to_string(),
                detail: format!("Asset '{}' is not referenced by authored scene content", asset_id),
            });
        }

        entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneAssetDiagnosticEntry {
    pub subject: XrdsSceneAssetDiagnosticSubject,
    pub severity: XrdsSceneAssetDiagnosticSeverity,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneAssetDiagnosticSubject {
    Node(XrdsSceneNodeId),
    Asset { asset_id: String, kind: XrdsSceneAssetKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneAssetDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsSceneAssetEnsureResult {
    pub asset: XrdsSceneAsset,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsSceneGltfPlacement {
    pub asset_id: String,
    pub node_id: Option<XrdsSceneNodeId>,
    pub parent_id: Option<XrdsSceneNodeId>,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: XrdsSceneTransform,
    pub scene_index: usize,
    pub export_policy: XrdsGltfAssetExportPolicy,
    pub editor: XrdsEditorMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneAssetWorkflowError {
    InvalidAssetId,
    InvalidAssetUri,
    AssetNotFound(String),
    DuplicateAssetId(String),
    DuplicateAssetUri {
        uri: String,
        asset_id: String,
    },
    AssetInUse {
        asset_id: String,
        node_ids: Vec<XrdsSceneNodeId>,
    },
    AssetKindMismatch {
        asset_id: String,
        expected: XrdsSceneAssetKind,
        found: XrdsSceneAssetKind,
    },
    ParentNotFound(XrdsSceneNodeId),
    NodeIdInUse(XrdsSceneNodeId),
    NodeNotFound(XrdsSceneNodeId),
    NodeIsNotGltfAsset(XrdsSceneNodeId),
    Validation(XrdsSceneValidationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneAssetRemovalPolicy {
    RejectIfReferenced,
    DetachReferencingNodes,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XrdsSceneAssetRemovalResult {
    pub removed_asset: XrdsSceneAsset,
    pub detached_node_ids: Vec<XrdsSceneNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneAssetRebindResult {
    pub asset_id: String,
    pub previous_uri: String,
    pub new_uri: String,
    pub rebound_node_ids: Vec<XrdsSceneNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneAssetRenameResult {
    pub previous_asset_id: String,
    pub new_asset_id: String,
    pub rewritten_node_ids: Vec<XrdsSceneNodeId>,
}
