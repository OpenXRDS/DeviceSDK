use super::*;

impl XrdsSceneDocument {
    pub fn gltf_node_authoring(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<Option<&XrdsSceneGltfNodeAuthoring>, XrdsSceneGltfWorkflowError> {
        ensure_gltf_node(self, node_id)?;
        Ok(self.gltf_node_authoring_entry(node_id))
    }

    pub fn node_gltf_default_playback(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<Option<&XrdsSceneGltfPlayback>, XrdsSceneGltfWorkflowError> {
        Ok(self
            .gltf_node_authoring(node_id)?
            .and_then(|authoring| authoring.default_playback.as_ref()))
    }

    pub fn node_gltf_morph_target_overrides(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<&[XrdsSceneGltfMorphTargetOverride], XrdsSceneGltfWorkflowError> {
        ensure_gltf_node(self, node_id)?;
        Ok(self
            .gltf_node_authoring_entry(node_id)
            .map(|authoring| authoring.morph_target_overrides.as_slice())
            .unwrap_or(&[]))
    }

    pub fn set_gltf_default_playback(
        &mut self,
        node_id: XrdsSceneNodeId,
        playback: Option<XrdsSceneGltfPlayback>,
    ) -> Result<(), XrdsSceneGltfWorkflowError> {
        ensure_gltf_node(self, node_id)?;
        let playback = playback
            .map(normalize_gltf_playback)
            .transpose()?;
        let authoring = self.gltf_node_authoring.entry(node_id.0).or_default();
        authoring.default_playback = playback;
        trim_empty_gltf_authoring(self, node_id);
        Ok(())
    }

    pub fn clear_gltf_default_playback(
        &mut self,
        node_id: XrdsSceneNodeId,
    ) -> Result<(), XrdsSceneGltfWorkflowError> {
        self.set_gltf_default_playback(node_id, None)
    }

    pub fn set_gltf_morph_target_overrides(
        &mut self,
        node_id: XrdsSceneNodeId,
        overrides: Vec<XrdsSceneGltfMorphTargetOverride>,
    ) -> Result<(), XrdsSceneGltfWorkflowError> {
        ensure_gltf_node(self, node_id)?;
        let overrides = normalize_gltf_morph_target_overrides(overrides)?;
        let authoring = self.gltf_node_authoring.entry(node_id.0).or_default();
        authoring.morph_target_overrides = overrides;
        trim_empty_gltf_authoring(self, node_id);
        Ok(())
    }

    pub fn clear_gltf_morph_target_overrides(
        &mut self,
        node_id: XrdsSceneNodeId,
    ) -> Result<(), XrdsSceneGltfWorkflowError> {
        self.set_gltf_morph_target_overrides(node_id, Vec::new())
    }

    pub fn set_gltf_morph_target_weight(
        &mut self,
        node_id: XrdsSceneNodeId,
        node: XrdsSceneGltfNodeLocator,
        mesh_name: Option<String>,
        selector: XrdsSceneGltfMorphTargetSelector,
        weight: f32,
    ) -> Result<(), XrdsSceneGltfWorkflowError> {
        ensure_gltf_node(self, node_id)?;
        let locator = normalize_gltf_node_locator(node)?;
        let mesh_name = normalize_optional_name(mesh_name, XrdsSceneGltfWorkflowError::EmptyMeshName)?;
        let selector = normalize_gltf_morph_target_selector(selector)?;
        if !weight.is_finite() {
            return Err(XrdsSceneGltfWorkflowError::InvalidMorphTargetWeight);
        }

        let authoring = self.gltf_node_authoring.entry(node_id.0).or_default();
        let entry = authoring
            .morph_target_overrides
            .iter_mut()
            .find(|candidate| candidate.node == locator && candidate.mesh_name == mesh_name);

        let override_entry = match entry {
            Some(existing) => existing,
            None => {
                authoring.morph_target_overrides.push(XrdsSceneGltfMorphTargetOverride {
                    node: locator.clone(),
                    mesh_name: mesh_name.clone(),
                    weights: Vec::new(),
                });
                authoring
                    .morph_target_overrides
                    .last_mut()
                    .expect("override was just pushed")
            }
        };

        if let Some(existing) = override_entry
            .weights
            .iter_mut()
            .find(|candidate| candidate.selector == selector)
        {
            existing.weight = weight;
        } else {
            override_entry.weights.push(XrdsSceneGltfMorphTargetWeight {
                selector,
                weight,
            });
        }

        trim_empty_gltf_authoring(self, node_id);
        Ok(())
    }
}

fn ensure_gltf_node(
    document: &XrdsSceneDocument,
    node_id: XrdsSceneNodeId,
) -> Result<(), XrdsSceneGltfWorkflowError> {
    let node = document
        .node(node_id)
        .ok_or(XrdsSceneGltfWorkflowError::NodeNotFound(node_id))?;
    match node.payload {
        XrdsSceneNodePayload::GltfAsset(_) => Ok(()),
        _ => Err(XrdsSceneGltfWorkflowError::NodeIsNotGltfAsset(node_id)),
    }
}

pub(crate) fn validate_gltf_authoring_entries(
    document: &XrdsSceneDocument,
) -> Result<(), XrdsSceneValidationError> {
    for (&node_id, authoring) in &document.gltf_node_authoring {
        let node_id = XrdsSceneNodeId(node_id);
        let Some(node) = document.node(node_id) else {
            return Err(XrdsSceneValidationError::MissingGltfAuthoringNode(node_id));
        };

        if !matches!(node.payload, XrdsSceneNodePayload::GltfAsset(_)) {
            return Err(XrdsSceneValidationError::GltfAuthoringTargetIsNotGltf(node_id));
        }

        validate_gltf_authoring(authoring)
            .map_err(|error| XrdsSceneValidationError::InvalidGltfAuthoring { node_id, error })?;
    }

    Ok(())
}

fn validate_gltf_authoring(
    authoring: &XrdsSceneGltfNodeAuthoring,
) -> Result<(), XrdsSceneGltfWorkflowError> {
    if let Some(playback) = &authoring.default_playback {
        normalize_gltf_playback(playback.clone())?;
    }

    normalize_gltf_morph_target_overrides(authoring.morph_target_overrides.clone())?;
    Ok(())
}

fn trim_empty_gltf_authoring(document: &mut XrdsSceneDocument, node_id: XrdsSceneNodeId) {
    let should_remove = document
        .gltf_node_authoring_entry(node_id)
        .is_some_and(|authoring| {
            authoring.default_playback.is_none() && authoring.morph_target_overrides.is_empty()
        });

    if should_remove {
        document.gltf_node_authoring.remove(&node_id.0);
    }
}

fn normalize_gltf_playback(
    mut playback: XrdsSceneGltfPlayback,
) -> Result<XrdsSceneGltfPlayback, XrdsSceneGltfWorkflowError> {
    playback.selector = normalize_gltf_animation_selector(playback.selector)?;
    if !playback.speed.is_finite() || playback.speed <= 0.0 {
        return Err(XrdsSceneGltfWorkflowError::InvalidPlaybackSpeed);
    }
    Ok(playback)
}

fn normalize_gltf_morph_target_overrides(
    overrides: Vec<XrdsSceneGltfMorphTargetOverride>,
) -> Result<Vec<XrdsSceneGltfMorphTargetOverride>, XrdsSceneGltfWorkflowError> {
    overrides
        .into_iter()
        .map(normalize_gltf_morph_target_override)
        .collect()
}

fn normalize_gltf_morph_target_override(
    mut override_entry: XrdsSceneGltfMorphTargetOverride,
) -> Result<XrdsSceneGltfMorphTargetOverride, XrdsSceneGltfWorkflowError> {
    override_entry.node = normalize_gltf_node_locator(override_entry.node)?;
    override_entry.mesh_name = normalize_optional_name(
        override_entry.mesh_name,
        XrdsSceneGltfWorkflowError::EmptyMeshName,
    )?;
    override_entry.weights = override_entry
        .weights
        .into_iter()
        .map(normalize_gltf_morph_target_weight)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(override_entry)
}

fn normalize_gltf_morph_target_weight(
    mut weight: XrdsSceneGltfMorphTargetWeight,
) -> Result<XrdsSceneGltfMorphTargetWeight, XrdsSceneGltfWorkflowError> {
    weight.selector = normalize_gltf_morph_target_selector(weight.selector)?;
    if !weight.weight.is_finite() {
        return Err(XrdsSceneGltfWorkflowError::InvalidMorphTargetWeight);
    }
    Ok(weight)
}

fn normalize_gltf_animation_selector(
    selector: XrdsSceneGltfAnimationSelector,
) -> Result<XrdsSceneGltfAnimationSelector, XrdsSceneGltfWorkflowError> {
    match selector {
        XrdsSceneGltfAnimationSelector::Index(index) => Ok(XrdsSceneGltfAnimationSelector::Index(index)),
        XrdsSceneGltfAnimationSelector::Name(name) => Ok(XrdsSceneGltfAnimationSelector::Name(
            normalize_required_name(name, XrdsSceneGltfWorkflowError::EmptyAnimationName)?,
        )),
    }
}

fn normalize_gltf_morph_target_selector(
    selector: XrdsSceneGltfMorphTargetSelector,
) -> Result<XrdsSceneGltfMorphTargetSelector, XrdsSceneGltfWorkflowError> {
    match selector {
        XrdsSceneGltfMorphTargetSelector::Index(index) => {
            Ok(XrdsSceneGltfMorphTargetSelector::Index(index))
        }
        XrdsSceneGltfMorphTargetSelector::Name(name) => Ok(
            XrdsSceneGltfMorphTargetSelector::Name(normalize_required_name(
                name,
                XrdsSceneGltfWorkflowError::EmptyMorphTargetName,
            )?),
        ),
    }
}

fn normalize_gltf_node_locator(
    mut locator: XrdsSceneGltfNodeLocator,
) -> Result<XrdsSceneGltfNodeLocator, XrdsSceneGltfWorkflowError> {
    locator.node_name = normalize_optional_name(
        locator.node_name,
        XrdsSceneGltfWorkflowError::EmptyNodeName,
    )?;
    Ok(locator)
}

fn normalize_required_name(
    name: String,
    error: XrdsSceneGltfWorkflowError,
) -> Result<String, XrdsSceneGltfWorkflowError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(error);
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_name(
    name: Option<String>,
    error: XrdsSceneGltfWorkflowError,
) -> Result<Option<String>, XrdsSceneGltfWorkflowError> {
    match name {
        Some(name) => Ok(Some(normalize_required_name(name, error)?)),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneGltfWorkflowError {
    NodeNotFound(XrdsSceneNodeId),
    NodeIsNotGltfAsset(XrdsSceneNodeId),
    EmptyAnimationName,
    InvalidPlaybackSpeed,
    EmptyNodeName,
    EmptyMeshName,
    EmptyMorphTargetName,
    InvalidMorphTargetWeight,
}