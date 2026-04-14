use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneCamera,
    XrdsSceneCameraProjection, XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMaterialPbrParams,
    XrdsSceneMaterialTextureSlots, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsScenePlane3D, XrdsSceneTextureRef, XrdsSceneTextureSamplerParams,
    XrdsSceneTextureUvParams, XrdsSceneTextureUvTransformMode, XrdsSceneTransform,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const DOCUMENT_ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(740);
const DOCUMENT_CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(741);
const DOCUMENT_BASELINE_PLANE_ID: XrdsSceneNodeId = XrdsSceneNodeId(742);
const DOCUMENT_ROTATED_PLANE_ID: XrdsSceneNodeId = XrdsSceneNodeId(743);

const ARROW_TEXTURE_ASSET_ID: &str = "asset:texture-arrow-box";
const ARROW_TEXTURE_URI: &str = "textures/arrow_box.png";
const QUARTER_TURN_XYZW: [f32; 4] = [0.70710677, 0.0, 0.0, 0.70710677];

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentTextureUvRotationOnly".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentTextureUvRotationOnlyApp)
    .expect("failed to run scene_document_texture_uv_rotation_only example");
}

struct SceneDocumentTextureUvRotationOnlyApp;

impl XrdsApp for SceneDocumentTextureUvRotationOnlyApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported authored scene ids: {imported_ids:?}");
        println!("Texture UV rotation-only example is running.");
        println!("Left plane: source texture with default UVs.");
        println!("Right plane: same texture and sampler, with a centered 90 degree UV rotation.");
        println!("Authored UV rotation is center-based by default.");
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Texture UV Rotation Only".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: ARROW_TEXTURE_ASSET_ID.to_string(),
            uri: ARROW_TEXTURE_URI.to_string(),
            kind: XrdsSceneAssetKind::Texture,
        }],
        nodes: vec![
            XrdsSceneNode {
                id: DOCUMENT_ROOT_ID,
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID,
                parent_id: None,
                name: "RotationOnlyCamera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.4, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 40.0,
                        near: 0.1,
                        far: Some(100.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 1.2, 0.0]),
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_BASELINE_PLANE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "BaselinePlane".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [-1.35, 1.2, 0.0],
                    rotation_quat_xyzw: QUARTER_TURN_XYZW,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
                    size: [2.0, 2.0],
                    material: baseline_material(),
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_ROTATED_PLANE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "RotatedPlane".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.35, 1.2, 0.0],
                    rotation_quat_xyzw: QUARTER_TURN_XYZW,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
                    size: [2.0, 2.0],
                    material: rotated_material(),
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    }
}

fn baseline_material() -> XrdsSceneMaterial {
    XrdsSceneMaterial {
        base_color: [1.0, 1.0, 1.0, 1.0],
        emissive: [0.0, 0.0, 0.0, 1.0],
        opacity: 1.0,
        unlit: true,
        textures: XrdsSceneMaterialTextureSlots {
            base_color: Some(XrdsSceneTextureRef {
                texture_asset_id: ARROW_TEXTURE_ASSET_ID.to_string(),
                uv: XrdsSceneTextureUvParams::default(),
                sampler: XrdsSceneTextureSamplerParams::default(),
            }),
            ..Default::default()
        },
        pbr: XrdsSceneMaterialPbrParams {
            double_sided: true,
            ..Default::default()
        },
    }
}

fn rotated_material() -> XrdsSceneMaterial {
    XrdsSceneMaterial {
        base_color: [1.0, 1.0, 1.0, 1.0],
        emissive: [0.0, 0.0, 0.0, 1.0],
        opacity: 1.0,
        unlit: true,
        textures: XrdsSceneMaterialTextureSlots {
            base_color: Some(XrdsSceneTextureRef {
                texture_asset_id: ARROW_TEXTURE_ASSET_ID.to_string(),
                uv: XrdsSceneTextureUvParams {
                    rotation_deg: 90.0,
                    transform_mode: XrdsSceneTextureUvTransformMode::Centered,
                    ..Default::default()
                },
                sampler: XrdsSceneTextureSamplerParams::default(),
            }),
            ..Default::default()
        },
        pbr: XrdsSceneMaterialPbrParams {
            double_sided: true,
            ..Default::default()
        },
    }
}
