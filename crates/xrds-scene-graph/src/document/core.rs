use super::*;

pub const XRDS_SCENE_DOCUMENT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// HUD library types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HudTemplateId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HudItemDefId(pub u64);

/// One text item inside a `XrdsHudTemplate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsHudItemDef {
    pub id: HudItemDefId,
    /// Key used to address this item at runtime via `set_hud_item`.
    pub name: String,
    /// Canvas-local position: X right, Y up (metres).
    pub position: [f32; 2],
    pub text: String,
    pub font_size: f32,
    /// RGBA in 0-1 range.
    pub color: [f32; 4],
}

impl Default for XrdsHudItemDef {
    fn default() -> Self {
        Self {
            id: HudItemDefId(1),
            name: "item".to_string(),
            position: [0.0, 0.0],
            text: String::new(),
            font_size: 4.0,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Authored HUD layout template stored in the document's `hud_library`.
/// Linked to a `PlayerAnchor` via `XrdsScenePlayerAnchor::hud_template_id`.
/// At runtime the system instantiates one copy per active anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsHudTemplate {
    pub id: HudTemplateId,
    pub name: String,
    /// Camera-space depth in metres (positive = in front of viewer).
    pub depth: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<XrdsHudItemDef>,
}

impl Default for XrdsHudTemplate {
    fn default() -> Self {
        Self {
            id: HudTemplateId(1),
            name: "HUD".to_string(),
            depth: 0.5,
            items: Vec::new(),
        }
    }
}

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hud_library: Vec<XrdsHudTemplate>,
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
    /// places and be edited in one.
    ///
    /// Additive for now — `hud_library` above still drives the working HUD, and
    /// nothing migrates onto this until the runtime does. See
    /// `docs/xrds-widget-template-plan.md`.
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
            hud_library: Vec::new(),
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

    pub fn hud_template(&self, id: HudTemplateId) -> Option<&XrdsHudTemplate> {
        self.hud_library.iter().find(|t| t.id == id)
    }

    pub fn hud_template_mut(&mut self, id: HudTemplateId) -> Option<&mut XrdsHudTemplate> {
        self.hud_library.iter_mut().find(|t| t.id == id)
    }

    pub fn next_available_template_id(&self) -> HudTemplateId {
        HudTemplateId(
            self.hud_library.iter().map(|t| t.id.0).max().unwrap_or(0).saturating_add(1),
        )
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
            hud_library: self.hud_library.clone(),
            // Cloned through unfiltered, same as hud_library above — an
            // unreferenced registry entry in the subset is harmless, just
            // unused, same as any other unreferenced asset.
            tracks: self.tracks.clone(),
            panels: self.panels.clone(),
        })
    }
}
