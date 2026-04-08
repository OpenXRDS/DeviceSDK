use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneDocument,
    XrdsSceneDocumentSession, XrdsSceneMaterialAlphaMode, XrdsSceneMetadata, XrdsSceneNode,
    XrdsSceneNodePayload, XrdsScenePointLight, XrdsSceneSphere, XrdsSceneTransform,
};
use xrds::sdk::{primitives::XrdsSphere, world::XrdsCamera, XrdsColor, XrdsId};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const DOCUMENT_ROOT_ID: XrdsId = XrdsId(500);
const DOCUMENT_CAMERA_ID: XrdsId = XrdsId(501);
const DOCUMENT_SPHERE_ID: XrdsId = XrdsId(502);
const DOCUMENT_LIGHT_ID: XrdsId = XrdsId(503);

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
    sphere_id: Option<XrdsId>,
}

impl XrdsApp for SceneDocumentMaterialWorkflowApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let mut session =
            XrdsSceneDocumentSession::new(authored_scene_document()).expect("document is valid");

        session
            .set_node_material_base_color(
                DOCUMENT_SPHERE_ID.into(),
                XrdsColor::srgb(0.92, 0.54, 0.18),
            )
            .expect("setting base color should succeed");
        session
            .set_node_material_metallic(DOCUMENT_SPHERE_ID.into(), 0.95)
            .expect("setting metallic should succeed");
        session
            .set_node_material_perceptual_roughness(DOCUMENT_SPHERE_ID.into(), 0.08)
            .expect("setting roughness should succeed");
        session
            .set_node_material_reflectance(DOCUMENT_SPHERE_ID.into(), 0.78)
            .expect("setting reflectance should succeed");
        session
            .set_node_material_double_sided(DOCUMENT_SPHERE_ID.into(), false)
            .expect("setting double sided should succeed");
        session
            .set_node_material_alpha_mode(
                DOCUMENT_SPHERE_ID.into(),
                XrdsSceneMaterialAlphaMode::Opaque,
            )
            .expect("setting alpha mode should succeed");
        session
            .set_node_material_opacity(DOCUMENT_SPHERE_ID.into(), 1.0)
            .expect("setting opacity should succeed");

        let material = session
            .document()
            .node_material(DOCUMENT_SPHERE_ID.into())
            .expect("sphere material should exist");
        println!(
            "Authored material: base_color={:?}, metallic={:.2}, roughness={:.2}, reflectance={:.2}",
            material.base_color,
            material.pbr.metallic,
            material.pbr.perceptual_roughness,
            material.pbr.reflectance
        );

        let imported_ids = api
            .import_scene_document(session.document())
            .expect("scene document import should succeed");
        println!("Imported scene ids: {imported_ids:?}");

        let camera_handle = api
            .handle_of::<XrdsCamera>(DOCUMENT_CAMERA_ID)
            .expect("camera should resolve after import");
        api.set_camera_look_at(&camera_handle, Some([0.0, 1.0, 0.0]));

        self.sphere_id = Some(DOCUMENT_SPHERE_ID);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let Some(sphere_id) = self.sphere_id else {
            return;
        };
        let Some(sphere_handle) = ctx.handle_of::<XrdsSphere>(sphere_id) else {
            return;
        };

        let yaw = ctx.elapsed_secs() * 0.7;
        let half_yaw = yaw * 0.5;
        ctx.set_rotation(&sphere_handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);
    }
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Scene Document Material Workflow".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
        nodes: vec![
            XrdsSceneNode {
                id: DOCUMENT_ROOT_ID.into(),
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID.into(),
                parent_id: None,
                name: "Camera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 2.2, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::default(),
                    look_at: Some([0.0, 1.0, 0.0]),
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_SPHERE_ID.into(),
                parent_id: Some(DOCUMENT_ROOT_ID.into()),
                name: "HeroSphere".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                    radius: 1.0,
                    material: Default::default(),
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_LIGHT_ID.into(),
                parent_id: None,
                name: "KeyLight".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [3.0, 4.5, 4.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    color: [1.0, 0.94, 0.88, 1.0],
                    intensity: 2_400_000.0,
                    range: 25.0,
                    radius: 0.25,
                    shadows: true,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    }
}
