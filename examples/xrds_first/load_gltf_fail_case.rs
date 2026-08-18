use xrds::*;

use xrds::sdk::world::XrdsCamera;
use xrds::sdk::world::XrdsGltfAsset;

const BROKEN_GLTF_PATH: &str = "models/TestBrokenDependency/MissingBufferScene.gltf";

pub fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "LoadGltfFailCase".to_owned(),
        ..Default::default()
    })
    .run_xrds(LoadGltfFailCaseApp::default())
    .expect("Failed to run LoadGltfFailCaseApp");
}

#[derive(Default)]
struct LoadGltfFailCaseApp {
    gltf_object_handle: Option<Handle<XrdsGltfAsset>>,
    last_status: Option<XrdsGltfLoadStatus>,
    failure_logged: bool,
}

impl XrdsApp for LoadGltfFailCaseApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        println!("Attempting to load broken glTF asset: {BROKEN_GLTF_PATH}");

        let gltf_object_handle = api.spawn(&{
            let mut gltf_object =
                XrdsGltfAsset::new(BROKEN_GLTF_PATH).with_name("BrokenDependencyScene");
            gltf_object.transform.translation = [0.0, 0.0, -2.0];
            gltf_object
        });
        self.gltf_object_handle = Some(gltf_object_handle);

        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("BrokenGltfCamera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([0.0, 0.0, 3.0])
                .looking_at([0.0, 1.0, 0.0])
        });
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let Some(handle) = &self.gltf_object_handle else {
            return;
        };

        let status = ctx.gltf_load_status(handle);
        if status != self.last_status {
            println!("BrokenDependencyScene load status: {:?}", status);
            self.last_status = status.clone();
        }

        match status {
            Some(XrdsGltfLoadStatus::Failed(message)) if !self.failure_logged => {
                println!("BrokenDependencyScene failed as expected: {message}");
                self.failure_logged = true;
            }
            Some(XrdsGltfLoadStatus::Loaded) => {
                println!("Unexpected success: broken glTF asset loaded without dependency failure");
            }
            _ => {}
        }
    }
}
