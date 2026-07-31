use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube,
    XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodePayload,
    XrdsScenePointLight, XrdsSceneTransform,
};
use xrds::sdk::{
    primitives::XrdsCube, world::lights::XrdsPointLight, CubeGeometryParams, PointLightParams,
    XrdsColor, XrdsId,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

// These ids belong to the authored scene document.
// They are explicit here only because the example is demonstrating import/export fidelity.
// Runtime-first SDK code should normally use typed handles instead of hard-coding ids.
const DOCUMENT_ROOT_ID: XrdsId = XrdsId(400);
const DOCUMENT_CUBE_ID: XrdsId = XrdsId(401);
const DOCUMENT_LIGHT_ID: XrdsId = XrdsId(402);
const DOCUMENT_CAMERA_ID: XrdsId = XrdsId(403);

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentExport".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentExportApp::default())
    .expect("failed to run scene_document_export example");
}

#[derive(Default)]
struct SceneDocumentExportApp {
    cube_id: Option<XrdsId>,
    rotation_radians: f32,
}

impl XrdsApp for SceneDocumentExportApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = authored_scene_document();
        let imported_ids = api
            .import_scene_document(&document)
            .expect("scene document import should succeed");

        println!("Imported scene ids for export demo: {imported_ids:?}");

        let cube_handle = api
            .handle_of::<XrdsCube>(DOCUMENT_CUBE_ID)
            .expect("cube should resolve after import");
        let light_handle = api
            .handle_of::<XrdsPointLight>(DOCUMENT_LIGHT_ID)
            .expect("light should resolve after import");

        api.set_cube_geometry(
            &cube_handle,
            CubeGeometryParams {
                size: [2.4, 1.4, 2.4],
            },
        )
        .set_point_light_params(
            &light_handle,
            PointLightParams {
                color: XrdsColor::srgb(1.0, 0.78, 0.5),
                intensity: 240_000.0,
                range: 28.0,
                radius: 0.4,
                shadows: true,
            },
        );

        // Flush the queued XRDS commit helpers once so the export snapshot reflects committed
        // runtime state rather than transient viewport preview changes.
        api.get().update();

        let exported = api
            .export_scene_document_with_metadata(XrdsSceneMetadata {
                name: "Exported Snapshot".to_string(),
                authored_by: Some("scene_document_export example".to_string()),
                ..Default::default()
            })
            .expect("scene document export should succeed");

        println!(
            "Exported document snapshot:\n{}",
            exported
                .to_json_string_pretty()
                .expect("exported document should serialize")
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

        // This is preview-only motion. It is intentionally not re-exported because it is not
        // committed through queued XRDS document/runtime patch helpers.
        let bob_y = 1.0 + 0.15 * (ctx.elapsed_secs() * 1.5).sin();
        ctx.set_translation(&cube_handle, [0.0, bob_y, 0.0]);
    }
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Export Example Input".to_string(),
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
                grabbable: false,
            },
            XrdsSceneNode {
                id: DOCUMENT_CUBE_ID.into(),
                parent_id: Some(DOCUMENT_ROOT_ID.into()),
                name: "ExportCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.5, 1.5, 1.5],
                    material: XrdsSceneMaterial {
                        base_color: [0.22, 0.64, 0.94, 1.0],
                        emissive: [0.03, 0.05, 0.08, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: Default::default(),
                        textures: Default::default(),
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata {
                    tags: vec!["export-demo".to_string()],
                    ..Default::default()
                },
                grabbable: false,
            },
            XrdsSceneNode {
                id: DOCUMENT_LIGHT_ID.into(),
                parent_id: None,
                name: "ExportLight".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [4.0, 6.0, 4.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    intensity: 180_000.0,
                    range: 24.0,
                    shadows: false,
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
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
                grabbable: false,
            },
        ],
        ..Default::default()
    }
}
