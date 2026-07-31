use xrds::*;

use xrds::sdk::world::XrdsCamera;
use xrds::sdk::world::XrdsGltfAsset;

pub fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "LoadGltf".to_owned(),
        ..Default::default()
    })
    .run_xrds(LoadGltfApp::default())
    .expect("Failed to run LoadGltfApp");
}

#[derive(Default)]
struct LoadGltfApp {
    gltf_object_handle: Option<Handle<XrdsGltfAsset>>,
    last_status: Option<XrdsGltfLoadStatus>,
    rotation_radians: f32,
}

impl XrdsApp for LoadGltfApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let gltf_object_handle = api.spawn(&{
            let mut gltf_object =
                XrdsGltfAsset::new("models/StainedGlassLamp/StainedGlassLamp.gltf")
                    .with_name("StainedGlassLamp");
            gltf_object.transform.translation = [0.0, 0.0, -2.0];
            gltf_object
        });
        self.gltf_object_handle = Some(gltf_object_handle);

        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("GltfCamera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([0.0, 0.0, 3.0])
                .looking_at([0.0, 1.0, 0.0])
        });
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        if let Some(handle) = &self.gltf_object_handle {
            let status = ctx.gltf_load_status(handle);
            if status != self.last_status {
                println!("StainedGlassLamp load status: {:?}", status);
                self.last_status = status.clone();
            }

            if matches!(status, Some(XrdsGltfLoadStatus::Loaded)) {
                self.rotation_radians += ctx.delta_secs() * 0.6;
                let half_yaw = self.rotation_radians * 0.5;
                ctx.set_rotation(handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);
            }
        }
    }
}
