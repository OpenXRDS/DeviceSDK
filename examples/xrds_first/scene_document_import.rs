use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube,
    XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodePayload,
    XrdsScenePointLight, XrdsSceneTransform,
};
use xrds::sdk::{primitives::XrdsCube, XrdsId};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

// These ids belong to the authored scene document.
// They are explicit here only because document import/export preserves stable ids across round-trips.
// Normal runtime-first SDK code should usually just keep the Handle returned by `api.spawn(...)`.
const DOCUMENT_ROOT_ID: XrdsId = XrdsId(100);
const DOCUMENT_CUBE_ID: XrdsId = XrdsId(101);
const DOCUMENT_LIGHT_ID: XrdsId = XrdsId(102);
const DOCUMENT_CAMERA_ID: XrdsId = XrdsId(103);

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentImport".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentImportApp::default())
    .expect("failed to run scene_document_import example");
}

#[derive(Default)]
struct SceneDocumentImportApp {
    rotating_cube_id: Option<XrdsId>,
    rotation_radians: f32,
}

impl XrdsApp for SceneDocumentImportApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported authored scene ids: {imported_ids:?}");

        let root_handle = api
            .handle_of::<xrds::sdk::world::XrdsNode>(DOCUMENT_ROOT_ID)
            .expect("root node should be reachable by imported XRDS id");
        let cube_handle = api
            .handle_of::<XrdsCube>(DOCUMENT_CUBE_ID)
            .expect("cube node should be reachable by imported XRDS id");

        println!(
            "Root children after import: {:?}",
            api.child_ids_of(&root_handle)
        );
        println!(
            "Imported cube parent id: {:?}",
            api.parent_id_of(&cube_handle)
        );

        self.rotating_cube_id = Some(DOCUMENT_CUBE_ID);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let Some(cube_id) = self.rotating_cube_id else {
            return;
        };
        let Some(cube_handle) = ctx.handle_of::<XrdsCube>(cube_id) else {
            return;
        };

        self.rotation_radians += ctx.delta_secs() * 0.8;
        let half_yaw = self.rotation_radians * 0.5;
        ctx.set_rotation(&cube_handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);
    }
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Imported Example Scene".to_string(),
            ..Default::default()
        },
        nodes: vec![
            XrdsSceneNode {
                id: DOCUMENT_ROOT_ID.into(),
                parent_id: None,
                name: "ImportedRoot".to_string(),
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
                id: DOCUMENT_CUBE_ID.into(),
                parent_id: Some(DOCUMENT_ROOT_ID.into()),
                name: "ImportedCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.5, 1.5, 1.5],
                    material: XrdsSceneMaterial {
                        base_color: [0.18, 0.66, 0.94, 1.0],
                        emissive: [0.03, 0.06, 0.1, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: Default::default(),
                        textures: Default::default(),
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_LIGHT_ID.into(),
                parent_id: None,
                name: "ImportedPointLight".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [4.0, 7.0, 5.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    intensity: 180_000.0,
                    range: 30.0,
                    shadows: true,
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID.into(),
                parent_id: None,
                name: "ImportedCamera".to_string(),
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
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
