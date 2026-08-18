use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneCamera,
    XrdsSceneCameraProjection, XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMaterialPbrParams,
    XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload,
    XrdsScenePlane3D, XrdsScenePointLight, XrdsSceneSphere, XrdsSceneTransform,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const DOCUMENT_ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(780);
const DOCUMENT_CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(781);
const DOCUMENT_KEY_LIGHT_ID: XrdsSceneNodeId = XrdsSceneNodeId(782);
const DOCUMENT_FLOOR_ID: XrdsSceneNodeId = XrdsSceneNodeId(783);
const DOCUMENT_POLISHED_SPHERE_ID: XrdsSceneNodeId = XrdsSceneNodeId(784);
const DOCUMENT_ROUGH_SPHERE_ID: XrdsSceneNodeId = XrdsSceneNodeId(785);

const IBL_DIFFUSE_ASSET_ID: &str = "asset:ibl-diffuse";
const IBL_SPECULAR_ASSET_ID: &str = "asset:ibl-specular";
const IBL_DIFFUSE_URI: &str = "environment_maps/diffuse.ktx2";
const IBL_SPECULAR_URI: &str = "environment_maps/specular.ktx2";

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentEnvironmentImport".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentEnvironmentImportApp)
    .expect("failed to run scene_document_environment_import example");
}

struct SceneDocumentEnvironmentImportApp;

impl XrdsApp for SceneDocumentEnvironmentImportApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported authored scene ids: {imported_ids:?}");
        println!("Scene environment import example is running.");
        println!("The authored document attaches scene-level IBL, manual exposure, and linear fog to the imported camera.");
        println!(
            "The left sphere is polished metal and should show tighter environment reflections."
        );
        println!("The right sphere is rougher metal and should blur the same environment.");
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

fn authored_scene_document() -> XrdsSceneDocument {
    let mut document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Scene Environment Import".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
        assets: vec![
            XrdsSceneAsset {
                id: IBL_DIFFUSE_ASSET_ID.to_string(),
                uri: IBL_DIFFUSE_URI.to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: IBL_SPECULAR_ASSET_ID.to_string(),
                uri: IBL_SPECULAR_URI.to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ],
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
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID,
                parent_id: None,
                name: "EnvironmentCamera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 2.2, 6.5],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 42.0,
                        near: 0.1,
                        far: Some(100.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 1.0, 0.0]),
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_KEY_LIGHT_ID,
                parent_id: None,
                name: "FillLight".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 4.5, 3.5],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    intensity: 45_000.0,
                    range: 25.0,
                    shadows: true,
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_FLOOR_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "Floor".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 0.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D {
                    size: [8.0, 8.0],
                    material: XrdsSceneMaterial {
                        base_color: [0.12, 0.12, 0.14, 1.0],
                        emissive: [0.0, 0.0, 0.0, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: XrdsSceneMaterialPbrParams {
                            roughness: 0.92,
                            reflectance: 0.2,
                            double_sided: true,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_POLISHED_SPHERE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "PolishedSphere".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [-1.2, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                    radius: 1.0,
                    material: XrdsSceneMaterial {
                        base_color: [0.92, 0.95, 1.0, 1.0],
                        emissive: [0.0, 0.0, 0.0, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: XrdsSceneMaterialPbrParams {
                            metallic: 1.0,
                            roughness: 0.08,
                            reflectance: 0.95,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_ROUGH_SPHERE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "RoughSphere".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.2, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                    radius: 1.0,
                    material: XrdsSceneMaterial {
                        base_color: [0.96, 0.8, 0.62, 1.0],
                        emissive: [0.0, 0.0, 0.0, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: XrdsSceneMaterialPbrParams {
                            metallic: 1.0,
                            roughness: 0.72,
                            reflectance: 0.9,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    };

    document
        .set_ibl_environment(IBL_DIFFUSE_ASSET_ID, IBL_SPECULAR_ASSET_ID, 900.0)
        .expect("scene environment authoring should validate");
    document
        .set_exposure_environment(6.0)
        .expect("scene exposure authoring should validate");
    document
        .set_fog_environment([0.35, 0.48, 0.66, 1.0], 5.0, 40.0)
        .expect("scene fog authoring should validate");

    document
}
