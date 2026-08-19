use super::*;

pub const XRDS_SCENE_DOCUMENT_VERSION: u32 = 1;

// `HudTemplateId`, `HudItemDefId`, `XrdsHudItemDef` and `XrdsHudTemplate` lived
// here until the panel-template unification retired them. A HUD is no longer a
// separate kind of thing: it is an `XrdsPanelTemplate` whose attachment happens
// to be a `PlayerAnchor`, and a HUD text item is a `Label` element. See
// `docs/done/xrds-widget-template-plan.md` §A4b-1.

// ---------------------------------------------------------------------------
// Scene document
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneDocument {
    pub version: u32,
    pub metadata: XrdsSceneMetadata,
    pub assets: Vec<XrdsSceneAsset>,
    pub nodes: Vec<XrdsSceneNode>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gltf_node_authoring: BTreeMap<u64, XrdsSceneGltfNodeAuthoring>,
    /// Named [`XrdsTrack`] templates, referenced by
    /// `XrdsTriggerBinding::track` by name — the *template* half of the
    /// template/instance split: one piece of choreography, fired from many
    /// bindings, edited in one place.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<XrdsNamedTrack>,
    /// Reusable [`XrdsPanelTemplate`]s — the unified model behind HUD panels and
    /// world-space panels, where the only difference is attachment.
    ///
    /// Mirrors `tracks`: a registry of named, reusable definitions referenced by
    /// whatever instances them, so one panel authored once can appear in several
    /// places and be edited in one. Because a `Panel` node stores nothing but a
    /// `template_id`, this registry *is* the content — it has to survive export,
    /// or the panels in a reloaded document are empty shells.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub panels: Vec<XrdsPanelTemplate>,
}

impl Default for XrdsSceneDocument {
    fn default() -> Self {
        Self {
            version: XRDS_SCENE_DOCUMENT_VERSION,
            metadata: XrdsSceneMetadata::default(),
            assets: Vec::new(),
            nodes: Vec::new(),
            gltf_node_authoring: BTreeMap::new(),
            tracks: Vec::new(),
            panels: Vec::new(),
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

    /// Whether `node_id`'s ancestor chain reaches a `PlayerAnchor` — i.e. whether
    /// the node is head-locked rather than placed in the world.
    ///
    /// Head-locking is expressed by *parenting*, not by a flag (the older HUD
    /// vocabulary was retired in favour of it), so this walk is the only way to
    /// answer the question.
    ///
    /// **Ancestors, not the immediate parent.** A Panel nested under an ordinary
    /// grouping node that is itself under a `PlayerAnchor` is still head-locked,
    /// and that is the arrangement an author is most likely to build. A check
    /// that only looked one level up would pass exactly the scenes that need
    /// catching.
    ///
    /// A malformed document can contain a parent cycle, so the walk tracks what it
    /// has seen and stops rather than hanging. Diagnostics run on documents that
    /// have just been loaded from disk and are not yet trusted.
    pub fn is_head_locked(&self, node_id: XrdsSceneNodeId) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut current = self.node(node_id).and_then(|n| n.parent_id);

        while let Some(id) = current {
            if !seen.insert(id) {
                return false; // cycle
            }
            let Some(node) = self.node(id) else { return false };
            if matches!(node.payload, XrdsSceneNodePayload::PlayerAnchor(_)) {
                return true;
            }
            current = node.parent_id;
        }
        false
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

    pub fn panel_template(&self, id: XrdsPanelTemplateId) -> Option<&XrdsPanelTemplate> {
        self.panels.iter().find(|t| t.id == id)
    }

    pub fn panel_template_mut(
        &mut self,
        id: XrdsPanelTemplateId,
    ) -> Option<&mut XrdsPanelTemplate> {
        self.panels.iter_mut().find(|t| t.id == id)
    }

    /// Look a template up by its authored name.
    ///
    /// Both lookups exist because the two halves address differently: an
    /// *instance* stores an id (stable across renames), while an *author* picks
    /// by name.
    pub fn panel_template_by_name(&self, name: &str) -> Option<&XrdsPanelTemplate> {
        self.panels.iter().find(|t| t.name == name)
    }

    /// Lowest id not already used by a panel template.
    pub fn next_available_panel_template_id(&self) -> XrdsPanelTemplateId {
        XrdsPanelTemplateId(self.panels.iter().map(|t| t.id.0).max().unwrap_or(0) + 1)
    }

    /// Looks up a Track by name — what `XrdsTriggerBinding::track`
    /// resolves against.
    pub fn track(&self, name: &str) -> Option<&XrdsNamedTrack> {
        self.tracks.iter().find(|t| t.name == name)
    }

    pub fn track_mut(&mut self, name: &str) -> Option<&mut XrdsNamedTrack> {
        self.tracks.iter_mut().find(|t| t.name == name)
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
            // Both registries clone through unfiltered — an unreferenced entry
            // in the subset is harmless, just unused, same as any other
            // unreferenced asset.
            tracks: self.tracks.clone(),
            panels: self.panels.clone(),
        })
    }
}
