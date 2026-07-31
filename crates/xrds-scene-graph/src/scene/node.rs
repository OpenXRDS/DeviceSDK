use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct XrdsSceneNodeId(pub u64);

impl From<XrdsId> for XrdsSceneNodeId {
    fn from(value: XrdsId) -> Self {
        Self(value.0)
    }
}

impl From<XrdsSceneNodeId> for XrdsId {
    fn from(value: XrdsSceneNodeId) -> Self {
        XrdsId(value.0)
    }
}

/// XR compositor blend mode for the scene (VR = opaque, AR = passthrough).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsXrBlendMode {
    /// Fully opaque VR rendering. Default.
    #[default]
    Opaque,
    /// Alpha-blend compositor passthrough (AR).
    AlphaBlend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneMetadata {
    pub name: String,
    pub authored_by: Option<String>,
    pub default_scene_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<XrdsSceneEnvironment>,
    #[serde(default, skip_serializing_if = "XrdsXrBlendMode::is_default")]
    pub xr_blend_mode: XrdsXrBlendMode,
    pub extras: BTreeMap<String, String>,
}

impl XrdsXrBlendMode {
    fn is_default(&self) -> bool {
        *self == XrdsXrBlendMode::Opaque
    }
}

impl Default for XrdsSceneMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled Scene".to_string(),
            authored_by: None,
            default_scene_label: None,
            environment: None,
            xr_blend_mode: XrdsXrBlendMode::Opaque,
            extras: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneNode {
    pub id: XrdsSceneNodeId,
    pub parent_id: Option<XrdsSceneNodeId>,
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    /// When true the XR grab system allows the player to pick up this entity
    /// with the controller trigger.  Saved to the scene document so the
    /// attribute survives export and reload.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub grabbable: bool,
    pub transform: XrdsSceneTransform,
    pub payload: XrdsSceneNodePayload,
    pub editor: XrdsEditorMetadata,
    /// Trigger-action bindings for this node — "when trigger kind K
    /// fires, run sequence S." Applies regardless of payload kind (any
    /// node can carry these, not just `InteractionZone` — e.g. a plain
    /// physics-body player node can bind a collision-sourced trigger).
    /// See `docs/xrds-scenegraph-trigger-action-sequencing.md`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<XrdsTriggerBinding>,
}

#[derive(Debug, Clone)]
pub struct XrdsSceneRuntimeNode {
    pub id: XrdsId,
    pub parent_id: Option<XrdsId>,
    pub component: XrdsSceneRuntimeComponent,
    pub material: Option<XrdsMaterialParams>,
    pub editor: XrdsEditorMetadata,
    pub gltf_node_authoring: Option<XrdsSceneGltfNodeAuthoring>,
}

/// Data for a HUD text runtime node.  Does not have a world-space transform.
#[derive(Debug, Clone)]
pub struct XrdsHudTextData {
    pub id: XrdsId,
    pub text: String,
    pub font_size: f32,
    pub color: [f32; 4],
    pub anchor: XrdsHudAnchor,
    pub offset: [f32; 2],
}

#[derive(Debug, Clone)]
pub enum XrdsSceneRuntimeComponent {
    Node(XrdsNode),
    Camera(XrdsCamera),
    GltfAsset(XrdsGltfAsset),
    Cube(XrdsCube),
    Cylinder(XrdsCylinder),
    Sphere(XrdsSphere),
    Plane3D(XrdsPlane3D),
    Tetrahedron(XrdsTetrahedron),
    AmbientLight(XrdsAmbientLight),
    DirectionalLight(XrdsDirectionalLight),
    PointLight(XrdsPointLight),
    SpotLight(XrdsSpotLight),
    AudioClip(XrdsAudioClip),
    HudText(XrdsHudTextData),
    Text(XrdsText),
    ExtrudedText(XrdsExtrudedText),
    /// Carries the base node (name/transform/visible) plus zone-specific data.
    InteractionZone(XrdsNode, xrds_components::XrdsInteractionZone),
    /// World-space UI panel with ordered child widgets and optional layout policy.
    WorldPanel(xrds_components::XrdsWorldPanel, Vec<XrdsSceneWorldWidget>, XrdsSceneWorldLayout),
}

impl XrdsSceneNode {
    pub fn gltf_export_class(&self) -> XrdsGltfExportClass {
        self.payload.gltf_export_class()
    }

    pub fn to_runtime_node(&self) -> XrdsSceneRuntimeNode {
        self.to_runtime_node_with_gltf_asset_uri(None)
    }

    pub(crate) fn to_runtime_node_with_gltf_asset_uri(
        &self,
        gltf_asset_uri: Option<&str>,
    ) -> XrdsSceneRuntimeNode {
        let transform: TransformParams = self.transform.into();
        let mut editor = self.editor.clone();

        if let XrdsSceneNodePayload::GltfAsset(asset) = &self.payload {
            editor.set_asset_id(asset.asset_id.clone());
        }

        match &self.payload {
            XrdsSceneNodePayload::Empty => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Node(XrdsNode {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Camera(camera) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Camera(XrdsCamera {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    projection: camera.projection.into(),
                    look_at: camera.look_at,
                    clear_color: Default::default(),
                    tonemapping: Default::default(),
                    bloom: Default::default(),
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::GltfAsset(asset) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::GltfAsset(XrdsGltfAsset {
                    name: self.name.clone(),
                    transform,
                    visible: self.visible,
                    gltf_asset_path: gltf_asset_uri.unwrap_or(&asset.asset_uri).to_string(),
                    scene_index: asset.scene_index,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Cube(cube) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Cube(XrdsCube {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    size: cube.size,
                    physics_body: cube.physics_body,
                    gravity_scale: cube.gravity_scale,
                    mass: cube.mass,
                }),
                material: Some(cube.material.clone().into()),
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Cylinder(cylinder) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Cylinder(XrdsCylinder {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    radius: cylinder.radius,
                    height: cylinder.height,
                    physics_body: cylinder.physics_body,
                    gravity_scale: cylinder.gravity_scale,
                    mass: cylinder.mass,
                }),
                material: Some(cylinder.material.clone().into()),
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Sphere(sphere) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Sphere(XrdsSphere {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    radius: sphere.radius,
                    physics_body: sphere.physics_body,
                    gravity_scale: sphere.gravity_scale,
                    mass: sphere.mass,
                }),
                material: Some(sphere.material.clone().into()),
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Plane3D(plane) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Plane3D(XrdsPlane3D {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    size: plane.size,
                    physics_body: plane.physics_body,
                    gravity_scale: plane.gravity_scale,
                    mass: plane.mass,
                }),
                material: Some(plane.material.clone().into()),
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::Tetrahedron(tetrahedron) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Tetrahedron(XrdsTetrahedron {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    vertices: tetrahedron.vertices.map(Into::into),
                }),
                material: Some(tetrahedron.material.clone().into()),
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::AmbientLight(light) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::AmbientLight(XrdsAmbientLight {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    color: XrdsColor { rgba: light.color },
                    brightness: light.brightness,
                    affects_baked_lighting: light.affects_baked_lighting,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::DirectionalLight(light) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::DirectionalLight(XrdsDirectionalLight {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    color: XrdsColor { rgba: light.color },
                    illuminance: light.illuminance,
                    shadows: light.shadows,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::PointLight(light) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::PointLight(XrdsPointLight {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    color: XrdsColor { rgba: light.color },
                    intensity: light.intensity,
                    range: light.range,
                    radius: light.radius,
                    shadows: light.shadows,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::SpotLight(light) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::SpotLight(XrdsSpotLight {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    color: XrdsColor { rgba: light.color },
                    intensity: light.intensity,
                    range: light.range,
                    inner_angle: light.inner_angle,
                    outer_angle: light.outer_angle,
                    shadows: light.shadows,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::AudioClip(clip) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::AudioClip(XrdsAudioClip {
                    name: self.name.clone(),
                    transform,
                    visible: self.visible,
                    audio_asset_id: clip.asset_id.clone(),
                    volume: clip.volume,
                    looped: clip.looped,
                    spatial: clip.spatial,
                    autoplay: clip.autoplay,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            XrdsSceneNodePayload::InteractionZone(z) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::InteractionZone(
                    XrdsNode {
                        name:    self.name.clone(),
                        enabled: self.enabled,
                        visible: self.visible,
                        transform,
                    },
                    xrds_components::XrdsInteractionZone {
                        shape:     z.shape,
                        grab_type: z.grab_type,
                        hoverable: z.hoverable,
                    },
                ),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // Player spawn points are document-only markers; at runtime the pawn is spawned
            // separately. Represent as an empty node so the transform is preserved.
            XrdsSceneNodePayload::PlayerSpawn(_) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Node(XrdsNode {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // Player is the world-space pawn entity; spawn as empty node.
            // XrdsPlayerRoot is inserted by the runtime importer after spawn.
            XrdsSceneNodePayload::Player(_) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Node(XrdsNode {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // PlayerAnchor is a reference frame for HUD children; spawn as empty node.
            // XrdsPlayerAnchorRoot is inserted by the runtime importer after spawn.
            XrdsSceneNodePayload::PlayerAnchor(_) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Node(XrdsNode {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // HUD text nodes are screen-space UI; carry their display data into runtime.
            XrdsSceneNodePayload::HudText(hud) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::HudText(XrdsHudTextData {
                    id: self.id.into(),
                    text: hud.text.clone(),
                    font_size: hud.font_size,
                    color: hud.color,
                    anchor: hud.anchor,
                    offset: hud.offset,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // World-space text node.
            XrdsSceneNodePayload::Text(t) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Text(XrdsText {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    text: t.text.clone(),
                    font_size: t.font_size,
                    color: t.color,
                    alignment: match t.alignment {
                        XrdsSceneTextAlignment::Left => XrdsTextAlignment::Left,
                        XrdsSceneTextAlignment::Center => XrdsTextAlignment::Center,
                        XrdsSceneTextAlignment::Right => XrdsTextAlignment::Right,
                    },
                    anchor: match t.anchor {
                        XrdsSceneTextAnchor::World                            => XrdsTextAnchor::World,
                        XrdsSceneTextAnchor::Billboard                        => XrdsTextAnchor::Billboard,
                        XrdsSceneTextAnchor::HeadLocked                       => XrdsTextAnchor::HeadLocked,
                        XrdsSceneTextAnchor::BodyLocked                       => XrdsTextAnchor::BodyLocked,
                        XrdsSceneTextAnchor::ComfortPinned { depth_m }        => XrdsTextAnchor::ComfortPinned { depth_m },
                        XrdsSceneTextAnchor::Cylindrical  { radius_m }        => XrdsTextAnchor::Cylindrical  { radius_m },
                    },
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // Extruded 3D text node.
            XrdsSceneNodePayload::ExtrudedText(t) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::ExtrudedText(XrdsExtrudedText {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                    text: t.text.clone(),
                    font_size: t.font_size,
                    color: t.color,
                    depth: t.depth,
                    alignment: match t.alignment {
                        XrdsSceneTextAlignment::Left => XrdsExtrudedTextAlignment::Left,
                        XrdsSceneTextAlignment::Center => XrdsExtrudedTextAlignment::Center,
                        XrdsSceneTextAlignment::Right => XrdsExtrudedTextAlignment::Right,
                    },
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // World-space UI panel: carries the full panel descriptor and its child widgets.
            XrdsSceneNodePayload::WorldPanel(panel) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::WorldPanel(
                    xrds_components::XrdsWorldPanel {
                        name: self.name.clone(),
                        enabled: self.enabled,
                        visible: self.visible,
                        transform,
                        size: panel.size,
                        color: panel.color,
                        corner_radius: panel.corner_radius,
                        opacity: panel.opacity,
                    },
                    panel.widgets.clone(),
                    panel.layout.clone(),
                ),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
            // Spawn zones are document-only volumes; represented as an empty node at runtime.
            // The XrdsPlayerSpawnZone component is inserted by tag_spawn_zone_entities().
            XrdsSceneNodePayload::PlayerSpawnZone(_) => XrdsSceneRuntimeNode {
                id: self.id.into(),
                parent_id: self.parent_id.map(Into::into),
                component: XrdsSceneRuntimeComponent::Node(XrdsNode {
                    name: self.name.clone(),
                    enabled: self.enabled,
                    visible: self.visible,
                    transform,
                }),
                material: None,
                editor,
                gltf_node_authoring: None,
            },
        }
    }

    fn from_parts(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        name: &str,
        enabled: bool,
        visible: bool,
        transform: TransformParams,
        payload: XrdsSceneNodePayload,
    ) -> Self {
        Self {
            id,
            parent_id,
            name: name.to_string(),
            enabled,
            visible,
            grabbable: false,
            transform: transform.into(),
            payload,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }
    }

    pub fn from_xrds_node(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        node: &XrdsNode,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &node.name,
            node.enabled,
            node.visible,
            node.transform,
            XrdsSceneNodePayload::Empty,
        )
    }

    pub fn from_xrds_camera(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        camera: &XrdsCamera,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &camera.name,
            camera.enabled,
            camera.visible,
            camera.transform,
            XrdsSceneNodePayload::Camera(camera.into()),
        )
    }

    pub fn from_xrds_audio_clip(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        audio: &XrdsAudioClip,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &audio.name,
            true,
            audio.visible,
            audio.transform,
            XrdsSceneNodePayload::AudioClip(audio.into()),
        )
    }

    pub fn from_xrds_gltf_asset(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        asset: &XrdsGltfAsset,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &asset.name,
            true,
            asset.visible,
            asset.transform,
            XrdsSceneNodePayload::GltfAsset(asset.into()),
        )
    }

    pub fn from_xrds_cube(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        cube: &XrdsCube,
        material: Option<XrdsMaterialParams>,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &cube.name,
            cube.enabled,
            cube.visible,
            cube.transform,
            XrdsSceneNodePayload::Cube(XrdsSceneCube {
                size: cube.size,
                material: material.unwrap_or_default().into(),
                physics_body: cube.physics_body,
                gravity_scale: cube.gravity_scale,
                mass: cube.mass,
            }),
        )
    }

    pub fn from_xrds_cylinder(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        cylinder: &XrdsCylinder,
        material: Option<XrdsMaterialParams>,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &cylinder.name,
            cylinder.enabled,
            cylinder.visible,
            cylinder.transform,
            XrdsSceneNodePayload::Cylinder(XrdsSceneCylinder {
                radius: cylinder.radius,
                height: cylinder.height,
                material: material.unwrap_or_default().into(),
                physics_body: cylinder.physics_body,
                gravity_scale: cylinder.gravity_scale,
                mass: cylinder.mass,
            }),
        )
    }

    pub fn from_xrds_sphere(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        sphere: &XrdsSphere,
        material: Option<XrdsMaterialParams>,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &sphere.name,
            sphere.enabled,
            sphere.visible,
            sphere.transform,
            XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                radius: sphere.radius,
                material: material.unwrap_or_default().into(),
                physics_body: sphere.physics_body,
                gravity_scale: sphere.gravity_scale,
                mass: sphere.mass,
            }),
        )
    }

    pub fn from_xrds_plane3d(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        plane: &XrdsPlane3D,
        material: Option<XrdsMaterialParams>,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &plane.name,
            plane.enabled,
            plane.visible,
            plane.transform,
            XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
                size: plane.size,
                material: material.unwrap_or_default().into(),
                physics_body: plane.physics_body,
                gravity_scale: plane.gravity_scale,
                mass: plane.mass,
            }),
        )
    }

    pub fn from_xrds_tetrahedron(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        tetrahedron: &XrdsTetrahedron,
        material: Option<XrdsMaterialParams>,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &tetrahedron.name,
            tetrahedron.enabled,
            tetrahedron.visible,
            tetrahedron.transform,
            XrdsSceneNodePayload::Tetrahedron(XrdsSceneTetrahedron {
                vertices: tetrahedron.vertices.map(Into::into),
                material: material.unwrap_or_default().into(),
            }),
        )
    }

    pub fn from_xrds_ambient_light(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        light: &XrdsAmbientLight,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &light.name,
            light.enabled,
            light.visible,
            light.transform,
            XrdsSceneNodePayload::AmbientLight(light.into()),
        )
    }

    pub fn from_xrds_directional_light(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        light: &XrdsDirectionalLight,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &light.name,
            light.enabled,
            light.visible,
            light.transform,
            XrdsSceneNodePayload::DirectionalLight(light.into()),
        )
    }

    pub fn from_xrds_point_light(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        light: &XrdsPointLight,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &light.name,
            light.enabled,
            light.visible,
            light.transform,
            XrdsSceneNodePayload::PointLight(light.into()),
        )
    }

    pub fn from_xrds_spot_light(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        light: &XrdsSpotLight,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &light.name,
            light.enabled,
            light.visible,
            light.transform,
            XrdsSceneNodePayload::SpotLight(light.into()),
        )
    }

    pub fn from_hud_text(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        name: &str,
        hud: XrdsSceneHudText,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            name,
            true,
            true,
            TransformParams::default(),
            XrdsSceneNodePayload::HudText(hud),
        )
    }

    pub fn from_xrds_text(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        text: &XrdsText,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &text.name,
            text.enabled,
            text.visible,
            text.transform,
            XrdsSceneNodePayload::Text(XrdsSceneText {
                text: text.text.clone(),
                font_size: text.font_size,
                color: text.color,
                alignment: match text.alignment {
                    XrdsTextAlignment::Left => XrdsSceneTextAlignment::Left,
                    XrdsTextAlignment::Center => XrdsSceneTextAlignment::Center,
                    XrdsTextAlignment::Right => XrdsSceneTextAlignment::Right,
                },
                anchor: match text.anchor {
                    XrdsTextAnchor::World                          => XrdsSceneTextAnchor::World,
                    XrdsTextAnchor::Billboard                      => XrdsSceneTextAnchor::Billboard,
                    XrdsTextAnchor::HeadLocked                     => XrdsSceneTextAnchor::HeadLocked,
                    XrdsTextAnchor::BodyLocked                     => XrdsSceneTextAnchor::BodyLocked,
                    XrdsTextAnchor::ComfortPinned { depth_m }      => XrdsSceneTextAnchor::ComfortPinned { depth_m },
                    XrdsTextAnchor::Cylindrical   { radius_m }     => XrdsSceneTextAnchor::Cylindrical   { radius_m },
                },
            }),
        )
    }

    pub fn from_xrds_extruded_text(
        id: XrdsSceneNodeId,
        parent_id: Option<XrdsSceneNodeId>,
        text: &XrdsExtrudedText,
    ) -> Self {
        Self::from_parts(
            id,
            parent_id,
            &text.name,
            text.enabled,
            text.visible,
            text.transform,
            XrdsSceneNodePayload::ExtrudedText(XrdsSceneExtrudedText {
                text: text.text.clone(),
                font_size: text.font_size,
                color: text.color,
                depth: text.depth,
                alignment: match text.alignment {
                    XrdsExtrudedTextAlignment::Left => XrdsSceneTextAlignment::Left,
                    XrdsExtrudedTextAlignment::Center => XrdsSceneTextAlignment::Center,
                    XrdsExtrudedTextAlignment::Right => XrdsSceneTextAlignment::Right,
                },
            }),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneTransform {
    pub translation: [f32; 3],
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

impl Default for XrdsSceneTransform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl From<TransformParams> for XrdsSceneTransform {
    fn from(value: TransformParams) -> Self {
        Self {
            translation: value.translation,
            rotation_quat_xyzw: value.rotation_quat_xyzw,
            scale: value.scale,
        }
    }
}

impl From<XrdsSceneTransform> for TransformParams {
    fn from(value: XrdsSceneTransform) -> Self {
        Self {
            translation: value.translation,
            rotation_quat_xyzw: value.rotation_quat_xyzw,
            rotation_euler_xyz_deg: [0.0, 0.0, 0.0],
            scale: value.scale,
        }
    }
}
