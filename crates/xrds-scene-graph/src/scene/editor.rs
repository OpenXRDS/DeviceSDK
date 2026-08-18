use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XrdsEditorMetadata {
    pub tags: Vec<String>,
    pub layer: Option<String>,
    pub locked: bool,
    pub hidden_in_editor: bool,
    pub user_properties: BTreeMap<String, String>,
    pub source: Option<XrdsSourceLink>,
}

impl Default for XrdsEditorMetadata {
    fn default() -> Self {
        Self {
            tags: Vec::new(),
            layer: None,
            locked: false,
            hidden_in_editor: false,
            user_properties: BTreeMap::new(),
            source: None,
        }
    }
}

impl XrdsEditorMetadata {
    pub fn set_asset_id(&mut self, asset_id: Option<String>) {
        let normalized = asset_id.and_then(|asset_id| {
            let trimmed = asset_id.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

        match normalized {
            Some(asset_id) => {
                self.source.get_or_insert_with(Default::default).asset_id = Some(asset_id);
            }
            None => {
                let Some(mut source) = self.source.take() else {
                    return;
                };

                source.asset_id = None;
                if source.source_node.is_some() || source.import_revision.is_some() {
                    self.source = Some(source);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XrdsSourceLink {
    pub asset_id: Option<String>,
    pub source_node: Option<String>,
    pub import_revision: Option<String>,
}

impl Default for XrdsSourceLink {
    fn default() -> Self {
        Self {
            asset_id: None,
            source_node: None,
            import_revision: None,
        }
    }
}