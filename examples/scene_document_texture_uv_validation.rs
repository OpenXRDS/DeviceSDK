use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneCamera,
    XrdsSceneCameraProjection, XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMaterialPbrParams,
    XrdsSceneMaterialTextureSlots, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsScenePlane3D, XrdsSceneTextureFilterMode, XrdsSceneTextureRef,
    XrdsSceneTextureSamplerParams, XrdsSceneTextureUvParams,
    XrdsSceneTextureUvTransformMode, XrdsSceneTextureWrapMode,
    XrdsSceneTransform,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const DOCUMENT_ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(700);
const DOCUMENT_CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(701);
const DOCUMENT_BASELINE_PLANE_ID: XrdsSceneNodeId = XrdsSceneNodeId(702);
const DOCUMENT_TRANSFORMED_PLANE_ID: XrdsSceneNodeId = XrdsSceneNodeId(703);

const GRID_TEXTURE_ASSET_ID: &str = "asset:texture-arrow-box";
const GRID_TEXTURE_URI: &str = "textures/arrow_box.png";
const QUARTER_TURN_XYZW: [f32; 4] = [0.70710677, 0.0, 0.0, 0.70710677];

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentTextureUvValidation".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentTextureUvValidationApp)
    .expect("failed to run scene_document_texture_uv_validation example");
}

struct SceneDocumentTextureUvValidationApp;

impl XrdsApp for SceneDocumentTextureUvValidationApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported authored scene ids: {imported_ids:?}");
        println!("Texture validation example is running.");
        println!("Left plane: default UVs with default sampling.");
        println!("Right plane: repeated, rotated UVs with nearest sampling and repeat wrap.");
        println!("If both planes look the same, the XRDS runtime material extension is broken.");
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Texture UV Validation".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: GRID_TEXTURE_ASSET_ID.to_string(),
            uri: GRID_TEXTURE_URI.to_string(),
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
                grabbable: false,
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID,
                parent_id: None,
                name: "ValidationCamera".to_string(),
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
                grabbable: false,
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
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
            },
            XrdsSceneNode {
                id: DOCUMENT_TRANSFORMED_PLANE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "TransformedPlane".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.35, 1.2, 0.0],
                    rotation_quat_xyzw: QUARTER_TURN_XYZW,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
                    size: [2.0, 2.0],
                    material: transformed_material(),
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
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
                texture_asset_id: GRID_TEXTURE_ASSET_ID.to_string(),
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

fn transformed_material() -> XrdsSceneMaterial {
    XrdsSceneMaterial {
        base_color: [1.0, 1.0, 1.0, 1.0],
        emissive: [0.0, 0.0, 0.0, 1.0],
        opacity: 1.0,
        unlit: true,
        textures: XrdsSceneMaterialTextureSlots {
            base_color: Some(XrdsSceneTextureRef {
                texture_asset_id: GRID_TEXTURE_ASSET_ID.to_string(),
                uv: XrdsSceneTextureUvParams {
                    set: 0,
                    offset: [0.18, 0.08],
                    scale: [6.0, 6.0],
                    rotation_deg: 32.0,
                    transform_mode: XrdsSceneTextureUvTransformMode::Centered,
                },
                sampler: XrdsSceneTextureSamplerParams {
                    wrap_u: XrdsSceneTextureWrapMode::Repeat,
                    wrap_v: XrdsSceneTextureWrapMode::Repeat,
                    min_filter: XrdsSceneTextureFilterMode::Nearest,
                    mag_filter: XrdsSceneTextureFilterMode::Nearest,
                    mipmap_filter: XrdsSceneTextureFilterMode::Nearest,
                },
            }),
            ..Default::default()
        },
        pbr: XrdsSceneMaterialPbrParams {
            double_sided: true,
            ..Default::default()
        },
    }
}
