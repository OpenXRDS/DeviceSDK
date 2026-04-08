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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsSceneMetadata {
    pub name: String,
    pub authored_by: Option<String>,
    pub default_scene_label: Option<String>,
    pub extras: BTreeMap<String, String>,
}

impl Default for XrdsSceneMetadata {
    fn default() -> Self {
        Self {
            name: "Untitled Scene".to_string(),
            authored_by: None,
            default_scene_label: None,
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
    pub transform: XrdsSceneTransform,
    pub payload: XrdsSceneNodePayload,
    pub editor: XrdsEditorMetadata,
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
                    affects_lightmapped_meshes: light.affects_lightmapped_meshes,
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
            transform: transform.into(),
            payload,
            editor: XrdsEditorMetadata::default(),
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
