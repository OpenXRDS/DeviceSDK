use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneDocument,
    XrdsSceneMaterial, XrdsSceneMaterialAlphaMode, XrdsSceneMaterialPbrParams,
    XrdsSceneMaterialTextureSlots, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsScenePointLight, XrdsSceneSphere, XrdsSceneTransform,
};
use xrds::sdk::{primitives::XrdsSphere, world::lights::XrdsPointLight, XrdsColor, XrdsId};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const DOCUMENT_ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(500);
const DOCUMENT_CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(501);
const DOCUMENT_SPHERE_ID: XrdsSceneNodeId = XrdsSceneNodeId(502);
const DOCUMENT_LIGHT_ID: XrdsSceneNodeId = XrdsSceneNodeId(503);

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentMaterialWorkflow".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentMaterialWorkflowApp::default())
    .expect("failed to run scene_document_material_workflow example");
}

#[derive(Default)]
struct SceneDocumentMaterialWorkflowApp {
    sphere: Option<Handle<XrdsSphere>>,
    light: Option<Handle<XrdsPointLight>>,
    elapsed: f32,
}

impl XrdsApp for SceneDocumentMaterialWorkflowApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported authored scene ids: {imported_ids:?}");
        println!("Scene Document Material Workflow example is running.");
        println!("The sphere has a translucent blue material (alpha blend, double-sided).");
        println!("Its emissive colour pulses over time to show live material editing.");

        self.sphere = api.handle_of::<XrdsSphere>(XrdsId::from(DOCUMENT_SPHERE_ID));
        self.light = api.handle_of::<XrdsPointLight>(XrdsId::from(DOCUMENT_LIGHT_ID));
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        self.elapsed += ctx.elapsed_secs() * 0.0; // access elapsed once for the delta
        let t = ctx.elapsed_secs();

        // Pulse the sphere's emissive colour to demonstrate live material editing.
        if let Some(ref sphere) = self.sphere {
            let pulse = (t * 1.5).sin() * 0.5 + 0.5;
            ctx.set_material_emissive(
                sphere,
                xrds::sdk::XrdsLinearRgba::rgb(0.0, pulse * 0.4, pulse * 0.9),
            );
        }
    }
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Material Workflow Test".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
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
                name: "WorkflowCamera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.5, 5.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 60.0,
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
                id: DOCUMENT_SPHERE_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "TestSphere".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                    radius: 1.0,
                    material: blue_alpha_material(),
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_LIGHT_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "FillLight".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 3.0, 3.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    color: XrdsColor::WHITE.rgba,
                    intensity: 800.0,
                    range: 20.0,
                    radius: 0.0,
                    shadows: false,
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}

fn blue_alpha_material() -> XrdsSceneMaterial {
    XrdsSceneMaterial {
        base_color: [0.0, 0.5, 1.0, 0.7],
        emissive: [0.0, 0.0, 0.0, 1.0],
        opacity: 0.7,
        unlit: false,
        pbr: XrdsSceneMaterialPbrParams {
            metallic: 0.0,
            roughness: 0.5,
            reflectance: 0.5,
            double_sided: true,
            alpha_mode: XrdsSceneMaterialAlphaMode::Blend,
            alpha_cutoff: 0.5,
        },
        textures: XrdsSceneMaterialTextureSlots::default(),
    }
}
