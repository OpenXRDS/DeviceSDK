use super::*;

pub const XRDS_SCENE_DOCUMENT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneDocument {
    pub version: u32,
    pub metadata: XrdsSceneMetadata,
    pub assets: Vec<XrdsSceneAsset>,
    pub nodes: Vec<XrdsSceneNode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gltf_node_authoring: BTreeMap<u64, XrdsSceneGltfNodeAuthoring>,
}

impl Default for XrdsSceneDocument {
    fn default() -> Self {
        Self {
            version: XRDS_SCENE_DOCUMENT_VERSION,
            metadata: XrdsSceneMetadata::default(),
            assets: Vec::new(),
            nodes: Vec::new(),
            gltf_node_authoring: BTreeMap::new(),
        }
    }
}

impl XrdsSceneDocument {
    pub fn gltf_asset_reference_node_ids(&self, asset_id: &str) -> Vec<XrdsSceneNodeId> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Vec::new();
        }

        self.nodes
            .iter()
            .filter_map(|node| match &node.payload {
                XrdsSceneNodePayload::GltfAsset(asset)
                    if asset.asset_id.as_deref() == Some(asset_id) =>
                {
                    Some(node.id)
                }
                _ => None,
            })
            .collect()
    }

    pub fn asset(&self, id: &str) -> Option<&XrdsSceneAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn asset_mut(&mut self, id: &str) -> Option<&mut XrdsSceneAsset> {
        self.assets.iter_mut().find(|asset| asset.id == id)
    }

    pub fn asset_by_uri(&self, uri: &str) -> Option<&XrdsSceneAsset> {
        self.assets.iter().find(|asset| asset.uri == uri)
    }

    pub fn asset_by_uri_and_kind(
        &self,
        uri: &str,
        kind: XrdsSceneAssetKind,
    ) -> Option<&XrdsSceneAsset> {
        self.assets
            .iter()
            .find(|asset| asset.kind == kind && asset.uri == uri)
    }

    pub fn material_texture_reference_node_ids(&self, asset_id: &str) -> Vec<XrdsSceneNodeId> {
        let asset_id = asset_id.trim();
        if asset_id.is_empty() {
            return Vec::new();
        }

        self.nodes
            .iter()
            .filter_map(|node| {
                let material = crate::document::material::node_material_ref(node)?;
                let is_referenced = [
                    XrdsSceneMaterialTextureSlotKind::BaseColor,
                    XrdsSceneMaterialTextureSlotKind::MetallicRoughness,
                    XrdsSceneMaterialTextureSlotKind::Normal,
                    XrdsSceneMaterialTextureSlotKind::Occlusion,
                    XrdsSceneMaterialTextureSlotKind::Emissive,
                ]
                .into_iter()
                .any(|slot| {
                    material
                        .textures
                        .get(slot)
                        .is_some_and(|texture| texture.texture_asset_id == asset_id)
                });

                is_referenced.then_some(node.id)
            })
            .collect()
    }

    pub fn node(&self, id: XrdsSceneNodeId) -> Option<&XrdsSceneNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn node_mut(&mut self, id: XrdsSceneNodeId) -> Option<&mut XrdsSceneNode> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }

    pub fn root_nodes(&self) -> impl Iterator<Item = &XrdsSceneNode> {
        self.nodes.iter().filter(|node| node.parent_id.is_none())
    }

    pub fn children_of(&self, parent_id: XrdsSceneNodeId) -> impl Iterator<Item = &XrdsSceneNode> {
        self.nodes
            .iter()
            .filter(move |node| node.parent_id == Some(parent_id))
    }

    pub fn next_available_node_id(&self) -> XrdsSceneNodeId {
        XrdsSceneNodeId(
            self.nodes
                .iter()
                .map(|node| node.id.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        )
    }

    pub(crate) fn gltf_node_authoring_entry(
        &self,
        node_id: XrdsSceneNodeId,
    ) -> Option<&XrdsSceneGltfNodeAuthoring> {
        self.gltf_node_authoring.get(&node_id.0)
    }

    /// Returns a new document containing only `root_id` and all its descendants.
    ///
    /// The root node's `parent_id` is cleared to `None` so it becomes a top-level
    /// export root. Returns `None` if `root_id` does not exist in this document.
    pub fn subtree_document(&self, root_id: XrdsSceneNodeId) -> Option<XrdsSceneDocument> {
        use std::collections::{HashSet, VecDeque};

        let mut ids: Vec<XrdsSceneNodeId> = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(root_id);
        while let Some(id) = queue.pop_front() {
            if self.node(id).is_none() {
                continue;
            }
            ids.push(id);
            for child in self.children_of(id) {
                queue.push_back(child.id);
            }
        }
        if ids.is_empty() {
            return None;
        }

        let id_set: HashSet<XrdsSceneNodeId> = ids.iter().copied().collect();

        let mut nodes: Vec<XrdsSceneNode> = self.nodes.iter()
            .filter(|n| id_set.contains(&n.id))
            .cloned()
            .collect();

        if let Some(root_node) = nodes.iter_mut().find(|n| n.id == root_id) {
            root_node.parent_id = None;
            root_node.transform = XrdsSceneTransform::default(); // export at origin, not world position
        }

        let gltf_node_authoring = self.gltf_node_authoring.iter()
            .filter(|(k, _)| id_set.contains(&XrdsSceneNodeId(**k)))
            .map(|(k, v)| (*k, v.clone()))
            .collect();

        Some(XrdsSceneDocument {
            version: self.version,
            metadata: self.metadata.clone(),
            assets: self.assets.clone(),
            nodes,
            gltf_node_authoring,
        })
    }
}
