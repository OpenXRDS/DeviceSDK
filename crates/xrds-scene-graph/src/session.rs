use super::*;

#[derive(Debug, Clone)]
pub struct XrdsSceneDocumentSession {
    document: XrdsSceneDocument,
    save_path: Option<std::path::PathBuf>,
    undo_stack: Vec<XrdsSceneDocument>,
    redo_stack: Vec<XrdsSceneDocument>,
    saved_document: XrdsSceneDocument,
    history_limit: usize,
}

impl XrdsSceneDocumentSession {
    pub const DEFAULT_HISTORY_LIMIT: usize = 128;

    pub fn new(document: XrdsSceneDocument) -> Result<Self, XrdsSceneValidationError> {
        Self::with_history_limit(document, Self::DEFAULT_HISTORY_LIMIT)
    }

    pub fn with_history_limit(
        document: XrdsSceneDocument,
        history_limit: usize,
    ) -> Result<Self, XrdsSceneValidationError> {
        document.validate()?;

        Ok(Self {
            saved_document: document.clone(),
            document,
            save_path: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            history_limit,
        })
    }

    pub fn load_json(path: impl AsRef<Path>) -> Result<Self, XrdsSceneDocumentSessionError> {
        let path = path.as_ref();
        let document = XrdsSceneDocument::load_json(path)
            .map_err(XrdsSceneDocumentSessionError::Persistence)?;
        let mut session = Self::new(document).map_err(XrdsSceneDocumentSessionError::Validation)?;
        session.save_path = Some(path.to_path_buf());
        Ok(session)
    }

    pub fn document(&self) -> &XrdsSceneDocument {
        &self.document
    }

    pub fn save_path(&self) -> Option<&Path> {
        self.save_path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.document != self.saved_document
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn history_limit(&self) -> usize {
        self.history_limit
    }

    pub fn set_history_limit(&mut self, history_limit: usize) {
        self.history_limit = history_limit;
        self.trim_undo_history();
        self.trim_redo_history();
    }

    pub fn replace_document(
        &mut self,
        document: XrdsSceneDocument,
    ) -> Result<(), XrdsSceneValidationError> {
        document.validate()?;
        self.push_undo_snapshot();
        self.document = document;
        self.redo_stack.clear();
        Ok(())
    }

    pub fn edit<F>(&mut self, edit: F) -> Result<(), XrdsSceneDocumentEditError>
    where
        F: FnOnce(&mut XrdsSceneDocument),
    {
        self.apply_operation(|document| {
            edit(document);
            Ok(())
        })
    }

    pub fn place_gltf_asset(
        &mut self,
        placement: XrdsSceneGltfPlacement,
    ) -> Result<XrdsSceneNodeId, XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .place_gltf_asset(placement)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn register_gltf_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .register_gltf_asset(asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn register_image_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .register_image_asset(asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn register_texture_asset(
        &mut self,
        asset_id: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAsset, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .register_texture_asset(asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn ensure_gltf_asset(
        &mut self,
        preferred_asset_id: Option<impl Into<String>>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneDocumentEditError> {
        let preferred_asset_id = preferred_asset_id.map(Into::into);
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .ensure_gltf_asset(preferred_asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn ensure_image_asset(
        &mut self,
        preferred_asset_id: Option<impl Into<String>>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneDocumentEditError> {
        let preferred_asset_id = preferred_asset_id.map(Into::into);
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .ensure_image_asset(preferred_asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn ensure_texture_asset(
        &mut self,
        preferred_asset_id: Option<impl Into<String>>,
        uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetEnsureResult, XrdsSceneDocumentEditError> {
        let preferred_asset_id = preferred_asset_id.map(Into::into);
        let uri = uri.into();
        self.apply_operation(|document| {
            document
                .ensure_texture_asset(preferred_asset_id, uri)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn retarget_gltf_asset(
        &mut self,
        node_id: XrdsSceneNodeId,
        asset_id: impl Into<String>,
        scene_index: usize,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        self.apply_operation(|document| {
            document
                .retarget_gltf_asset(node_id, &asset_id, scene_index)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn remove_asset(
        &mut self,
        asset_id: impl Into<String>,
        policy: XrdsSceneAssetRemovalPolicy,
    ) -> Result<XrdsSceneAssetRemovalResult, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        self.apply_operation(|document| {
            document
                .remove_asset(&asset_id, policy)
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn rebind_gltf_asset(
        &mut self,
        asset_id: impl Into<String>,
        new_uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetRebindResult, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let new_uri = new_uri.into();
        self.apply_operation(|document| {
            document
                .rebind_gltf_asset(&asset_id, new_uri.clone())
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn rebind_asset(
        &mut self,
        asset_id: impl Into<String>,
        new_uri: impl Into<String>,
    ) -> Result<XrdsSceneAssetRebindResult, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let new_uri = new_uri.into();
        self.apply_operation(|document| {
            document
                .rebind_asset(&asset_id, new_uri.clone())
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn rename_asset_id(
        &mut self,
        asset_id: impl Into<String>,
        new_asset_id: impl Into<String>,
    ) -> Result<XrdsSceneAssetRenameResult, XrdsSceneDocumentEditError> {
        let asset_id = asset_id.into();
        let new_asset_id = new_asset_id.into();
        self.apply_operation(|document| {
            document
                .rename_asset_id(&asset_id, new_asset_id.clone())
                .map_err(XrdsSceneDocumentEditError::AssetWorkflow)
        })
    }

    pub fn set_node_tags(
        &mut self,
        node_id: XrdsSceneNodeId,
        tags: Vec<String>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_tags(node_id, tags)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_layer(
        &mut self,
        node_id: XrdsSceneNodeId,
        layer: Option<String>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_layer(node_id, layer)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_locked(
        &mut self,
        node_id: XrdsSceneNodeId,
        locked: bool,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_locked(node_id, locked)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_hidden_in_editor(
        &mut self,
        node_id: XrdsSceneNodeId,
        hidden_in_editor: bool,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_hidden_in_editor(node_id, hidden_in_editor)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_user_property(
        &mut self,
        node_id: XrdsSceneNodeId,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Option<String>, XrdsSceneDocumentEditError> {
        let key = key.into();
        let value = value.into();
        self.apply_operation(|document| {
            document
                .set_node_user_property(node_id, key, value)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn remove_node_user_property(
        &mut self,
        node_id: XrdsSceneNodeId,
        key: impl Into<String>,
    ) -> Result<Option<String>, XrdsSceneDocumentEditError> {
        let key = key.into();
        self.apply_operation(|document| {
            document
                .remove_node_user_property(node_id, &key)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_source_link(
        &mut self,
        node_id: XrdsSceneNodeId,
        source: Option<XrdsSourceLink>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_source_link(node_id, source)
                .map_err(XrdsSceneDocumentEditError::MetadataWorkflow)
        })
    }

    pub fn set_node_material(
        &mut self,
        node_id: XrdsSceneNodeId,
        material: XrdsSceneMaterial,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material(node_id, material)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_base_color(
        &mut self,
        node_id: XrdsSceneNodeId,
        color: XrdsColor,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_base_color(node_id, color)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_emissive(
        &mut self,
        node_id: XrdsSceneNodeId,
        emissive: XrdsLinearRgba,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_emissive(node_id, emissive)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_opacity(
        &mut self,
        node_id: XrdsSceneNodeId,
        opacity: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_opacity(node_id, opacity)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_unlit(
        &mut self,
        node_id: XrdsSceneNodeId,
        unlit: bool,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_unlit(node_id, unlit)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_pbr(
        &mut self,
        node_id: XrdsSceneNodeId,
        pbr: XrdsSceneMaterialPbrParams,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_pbr(node_id, pbr)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_textures(
        &mut self,
        node_id: XrdsSceneNodeId,
        textures: XrdsSceneMaterialTextureSlots,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_textures(node_id, textures)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_texture(
        &mut self,
        node_id: XrdsSceneNodeId,
        slot: XrdsSceneMaterialTextureSlotKind,
        texture: Option<XrdsSceneTextureRef>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_texture(node_id, slot, texture)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_metallic(
        &mut self,
        node_id: XrdsSceneNodeId,
        metallic: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_metallic(node_id, metallic)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_perceptual_roughness(
        &mut self,
        node_id: XrdsSceneNodeId,
        perceptual_roughness: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_perceptual_roughness(node_id, perceptual_roughness)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_reflectance(
        &mut self,
        node_id: XrdsSceneNodeId,
        reflectance: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_reflectance(node_id, reflectance)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_double_sided(
        &mut self,
        node_id: XrdsSceneNodeId,
        double_sided: bool,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_double_sided(node_id, double_sided)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_alpha_mode(
        &mut self,
        node_id: XrdsSceneNodeId,
        alpha_mode: XrdsSceneMaterialAlphaMode,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_alpha_mode(node_id, alpha_mode)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_node_material_alpha_cutoff(
        &mut self,
        node_id: XrdsSceneNodeId,
        alpha_cutoff: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_node_material_alpha_cutoff(node_id, alpha_cutoff)
                .map_err(XrdsSceneDocumentEditError::MaterialWorkflow)
        })
    }

    pub fn set_gltf_default_playback(
        &mut self,
        node_id: XrdsSceneNodeId,
        playback: Option<XrdsSceneGltfPlayback>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_gltf_default_playback(node_id, playback)
                .map_err(XrdsSceneDocumentEditError::GltfWorkflow)
        })
    }

    pub fn clear_gltf_default_playback(
        &mut self,
        node_id: XrdsSceneNodeId,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .clear_gltf_default_playback(node_id)
                .map_err(XrdsSceneDocumentEditError::GltfWorkflow)
        })
    }

    pub fn set_gltf_morph_target_overrides(
        &mut self,
        node_id: XrdsSceneNodeId,
        overrides: Vec<XrdsSceneGltfMorphTargetOverride>,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_gltf_morph_target_overrides(node_id, overrides)
                .map_err(XrdsSceneDocumentEditError::GltfWorkflow)
        })
    }

    pub fn clear_gltf_morph_target_overrides(
        &mut self,
        node_id: XrdsSceneNodeId,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .clear_gltf_morph_target_overrides(node_id)
                .map_err(XrdsSceneDocumentEditError::GltfWorkflow)
        })
    }

    pub fn set_gltf_morph_target_weight(
        &mut self,
        node_id: XrdsSceneNodeId,
        node: XrdsSceneGltfNodeLocator,
        mesh_name: Option<String>,
        selector: XrdsSceneGltfMorphTargetSelector,
        weight: f32,
    ) -> Result<(), XrdsSceneDocumentEditError> {
        self.apply_operation(|document| {
            document
                .set_gltf_morph_target_weight(node_id, node, mesh_name, selector, weight)
                .map_err(XrdsSceneDocumentEditError::GltfWorkflow)
        })
    }

    fn apply_operation<T, F>(&mut self, operation: F) -> Result<T, XrdsSceneDocumentEditError>
    where
        F: FnOnce(&mut XrdsSceneDocument) -> Result<T, XrdsSceneDocumentEditError>,
    {
        let before = self.document.clone();

        let value = match operation(&mut self.document) {
            Ok(value) => value,
            Err(error) => {
                self.document = before;
                return Err(error);
            }
        };

        if let Err(error) = self.document.validate() {
            self.document = before;
            return Err(XrdsSceneDocumentEditError::Validation(error));
        }

        if self.document == before {
            return Ok(value);
        }

        self.undo_stack.push(before);
        self.trim_undo_history();
        self.redo_stack.clear();
        Ok(value)
    }

    pub fn mark_saved(&mut self) {
        self.saved_document = self.document.clone();
    }

    pub fn save(&mut self) -> Result<(), XrdsSceneDocumentSessionError> {
        let Some(path) = self.save_path.clone() else {
            return Err(XrdsSceneDocumentSessionError::MissingSavePath);
        };

        self.document
            .save_json(&path)
            .map_err(XrdsSceneDocumentSessionError::Persistence)?;
        self.saved_document = self.document.clone();
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), XrdsSceneDocumentSessionError> {
        let path = path.as_ref();
        self.document
            .save_json(path)
            .map_err(XrdsSceneDocumentSessionError::Persistence)?;
        self.save_path = Some(path.to_path_buf());
        self.saved_document = self.document.clone();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo_stack.pop() else {
            return false;
        };

        self.redo_stack
            .push(std::mem::replace(&mut self.document, previous));
        self.trim_redo_history();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };

        self.undo_stack
            .push(std::mem::replace(&mut self.document, next));
        self.trim_undo_history();
        true
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.document.clone());
        self.trim_undo_history();
    }

    fn trim_undo_history(&mut self) {
        if self.history_limit == 0 {
            self.undo_stack.clear();
            return;
        }

        let overflow = self.undo_stack.len().saturating_sub(self.history_limit);
        if overflow > 0 {
            self.undo_stack.drain(0..overflow);
        }
    }

    fn trim_redo_history(&mut self) {
        if self.history_limit == 0 {
            self.redo_stack.clear();
            return;
        }

        let overflow = self.redo_stack.len().saturating_sub(self.history_limit);
        if overflow > 0 {
            self.redo_stack.drain(0..overflow);
        }
    }
}

#[derive(Debug)]
pub enum XrdsSceneDocumentSessionError {
    MissingSavePath,
    Persistence(XrdsSceneDocumentPersistenceError),
    Validation(XrdsSceneValidationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XrdsSceneDocumentEditError {
    Validation(XrdsSceneValidationError),
    AssetWorkflow(XrdsSceneAssetWorkflowError),
    GltfWorkflow(XrdsSceneGltfWorkflowError),
    MetadataWorkflow(XrdsSceneMetadataWorkflowError),
    MaterialWorkflow(XrdsSceneMaterialWorkflowError),
}
