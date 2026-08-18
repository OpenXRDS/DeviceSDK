use super::*;

impl XrdsSceneDocument {
    pub fn editor_metadata(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Result<&XrdsEditorMetadata, XrdsSceneMetadataWorkflowError> {
        self.node(node_id)
            .map(|node| &node.editor)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))
    }

    pub fn set_node_tags(
        &mut self,
        node_id: XrdsSceneNodeId,
        tags: Vec<String>,
    ) -> Result<(), XrdsSceneMetadataWorkflowError> {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        node.editor.tags = normalize_metadata_tags(tags);
        Ok(())
    }

    pub fn set_node_layer(
        &mut self,
        node_id: XrdsSceneNodeId,
        layer: Option<String>,
    ) -> Result<(), XrdsSceneMetadataWorkflowError> {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        node.editor.layer = normalize_optional_metadata_text(layer);
        Ok(())
    }

    pub fn set_node_locked(
        &mut self,
        node_id: XrdsSceneNodeId,
        locked: bool,
    ) -> Result<(), XrdsSceneMetadataWorkflowError> {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        node.editor.locked = locked;
        Ok(())
    }

    pub fn set_node_hidden_in_editor(
        &mut self,
        node_id: XrdsSceneNodeId,
        hidden_in_editor: bool,
    ) -> Result<(), XrdsSceneMetadataWorkflowError> {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        node.editor.hidden_in_editor = hidden_in_editor;
        Ok(())
    }

    pub fn set_node_user_property(
        &mut self,
        node_id: XrdsSceneNodeId,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Option<String>, XrdsSceneMetadataWorkflowError> {
        let key = normalize_metadata_key(key.into())?;
        let value = value.into();
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        Ok(node.editor.user_properties.insert(key, value))
    }

    pub fn remove_node_user_property(
        &mut self,
        node_id: XrdsSceneNodeId,
        key: &str,
    ) -> Result<Option<String>, XrdsSceneMetadataWorkflowError> {
        let key = normalize_metadata_key(key.to_string())?;
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        Ok(node.editor.user_properties.remove(&key))
    }

    pub fn set_node_source_link(
        &mut self,
        node_id: XrdsSceneNodeId,
        source: Option<XrdsSourceLink>,
    ) -> Result<(), XrdsSceneMetadataWorkflowError> {
        let node = self
            .node_mut(node_id)
            .ok_or(XrdsSceneMetadataWorkflowError::NodeNotFound(node_id))?;
        node.editor.source = normalize_source_link(source);
        Ok(())
    }
}

fn normalize_metadata_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }

        if seen.insert(tag.to_string()) {
            normalized.push(tag.to_string());
        }
    }

    normalized
}

fn normalize_optional_metadata_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn normalize_metadata_key(key: String) -> Result<String, XrdsSceneMetadataWorkflowError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(XrdsSceneMetadataWorkflowError::EmptyPropertyKey);
    }

    Ok(key.to_string())
}

fn normalize_source_link(source: Option<XrdsSourceLink>) -> Option<XrdsSourceLink> {
    let mut source = source?;
    source.asset_id = normalize_optional_metadata_text(source.asset_id);
    source.source_node = normalize_optional_metadata_text(source.source_node);
    source.import_revision = normalize_optional_metadata_text(source.import_revision);

    if source.asset_id.is_none() && source.source_node.is_none() && source.import_revision.is_none()
    {
        None
    } else {
        Some(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XrdsSceneMetadataWorkflowError {
    NodeNotFound(XrdsSceneNodeId),
    EmptyPropertyKey,
}