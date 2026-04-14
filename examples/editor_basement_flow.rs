// Documentation-focused example: this demonstrates an editor-basement workflow for docs/tests.
// It is not the SDK implementation itself.

use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube,
    XrdsSceneDocument, XrdsSceneDocumentSession, XrdsSceneMaterial, XrdsSceneMetadata,
    XrdsSceneNode, XrdsSceneNodePayload, XrdsScenePointLight, XrdsSceneTransform, XrdsSourceLink,
};
use xrds::sdk::{
    primitives::XrdsCube,
    world::{lights::XrdsPointLight, XrdsNode},
    CubeGeometryParams, PointLightParams, XrdsColor, XrdsId,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

// These ids belong to the authored scene document used by this documentation example.
// They are explicit only to demonstrate stable import/export identity.
// Typical runtime SDK usage should keep typed handles instead of hard-coding ids.
const DOCUMENT_ROOT_ID: XrdsId = XrdsId(300);
const DOCUMENT_CUBE_ID: XrdsId = XrdsId(301);
const DOCUMENT_LIGHT_ID: XrdsId = XrdsId(302);
const DOCUMENT_CAMERA_ID: XrdsId = XrdsId(303);

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "EditorBasementFlow".to_owned(),
        ..Default::default()
    })
    .run_xrds(EditorBasementFlowApp::default())
    .expect("failed to run editor_basement_flow example");
}

#[derive(Default)]
struct EditorBasementFlowApp {
    cube_id: Option<XrdsId>,
    rotation_radians: f32,
}

impl XrdsApp for EditorBasementFlowApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let mut session =
            XrdsSceneDocumentSession::new(authored_scene_document()).expect("document is valid");

        session
            .set_node_tags(
                DOCUMENT_CUBE_ID.into(),
                vec![
                    " hero ".to_string(),
                    "editable".to_string(),
                    "hero".to_string(),
                ],
            )
            .expect("setting tags should succeed");
        session
            .set_node_layer(DOCUMENT_CUBE_ID.into(), Some(" Gameplay ".to_string()))
            .expect("setting layer should succeed");
        session
            .set_node_user_property(DOCUMENT_CUBE_ID.into(), "inspector:expanded", "true")
            .expect("setting user property should succeed");
        session
            .set_node_source_link(
                DOCUMENT_CUBE_ID.into(),
                Some(XrdsSourceLink {
                    asset_id: None,
                    source_node: Some("CubeNode".to_string()),
                    import_revision: Some("draft-2".to_string()),
                }),
            )
            .expect("setting source link should succeed");
        session
            .edit(|document| {
                document.metadata.name = "Editor Basement Flow".to_string();

                let cube = document
                    .node_mut(DOCUMENT_CUBE_ID.into())
                    .expect("cube node should exist");
                cube.transform.translation = [0.0, 1.25, 0.0];

                let XrdsSceneNodePayload::Cube(cube_payload) = &mut cube.payload else {
                    panic!("expected cube payload");
                };
                cube_payload.material.base_color = [0.86, 0.55, 0.22, 1.0];
                cube_payload.material.emissive = [0.08, 0.03, 0.0, 1.0];
            })
            .expect("document edit should succeed");

        println!(
            "Session prepared: dirty={}, can_undo={}, can_redo={}",
            session.is_dirty(),
            session.can_undo(),
            session.can_redo()
        );

        assert!(
            session.undo(),
            "session should support undo after authored edits"
        );
        assert!(
            session.redo(),
            "session should support redo after authored edits"
        );

        let imported_ids = api
            .import_scene_document(session.document())
            .expect("scene document import should succeed");
        println!("Imported scene ids: {imported_ids:?}");

        let root_handle = api
            .handle_of::<XrdsNode>(DOCUMENT_ROOT_ID)
            .expect("root node should resolve after import");
        let cube_handle = api
            .handle_of::<XrdsCube>(DOCUMENT_CUBE_ID)
            .expect("cube should resolve after import");
        let light_handle = api
            .handle_of::<XrdsPointLight>(DOCUMENT_LIGHT_ID)
            .expect("point light should resolve after import");

        println!(
            "Imported root children: {:?}",
            api.child_ids_of(&root_handle)
        );

        api.set_cube_geometry(
            &cube_handle,
            CubeGeometryParams {
                size: [1.8, 2.4, 1.8],
            },
        )
        .set_point_light_params(
            &light_handle,
            PointLightParams {
                color: XrdsColor::srgb(1.0, 0.86, 0.72),
                intensity: 260_000.0,
                range: 26.0,
                radius: 0.35,
                shadows: true,
            },
        );

        self.cube_id = Some(DOCUMENT_CUBE_ID);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let Some(cube_id) = self.cube_id else {
            return;
        };
        let Some(cube_handle) = ctx.handle_of::<XrdsCube>(cube_id) else {
            return;
        };

        self.rotation_radians += ctx.delta_secs() * 0.8;
        let half_yaw = self.rotation_radians * 0.5;
        ctx.set_rotation(&cube_handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);

        let bob_y = 1.25 + 0.1 * (ctx.elapsed_secs() * 1.5).sin();
        ctx.set_translation(&cube_handle, [0.0, bob_y, 0.0]);
    }
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Draft Basement Scene".to_string(),
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
                editor: XrdsEditorMetadata {
                    tags: vec!["folder".to_string()],
                    ..Default::default()
                },
            },
            XrdsSceneNode {
                id: DOCUMENT_CUBE_ID.into(),
                parent_id: Some(DOCUMENT_ROOT_ID.into()),
                name: "PreviewCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.5, 1.5, 1.5],
                    material: XrdsSceneMaterial {
                        base_color: [0.25, 0.6, 0.95, 1.0],
                        emissive: [0.03, 0.05, 0.09, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: Default::default(),
                        textures: Default::default(),
                    },
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
                    translation: [4.0, 6.5, 5.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    intensity: 180_000.0,
                    range: 24.0,
                    shadows: true,
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID.into(),
                parent_id: None,
                name: "Camera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 3.0, 8.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 50.0,
                        near: 0.1,
                        far: Some(200.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 1.0, 0.0]),
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    }
}
