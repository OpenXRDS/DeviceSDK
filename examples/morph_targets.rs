use xrds::sdk::world::{
    lights::{XrdsAmbientLight, XrdsDirectionalLight},
    XrdsCamera, XrdsGltfAsset,
};
use xrds::{
    Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsGltfAnimationPlaybackOptions,
    XrdsGltfAnimationSelector,
};

const MORPH_STRESS_TEST_PATH: &str = "models/animated/morphOriginal/MorphStressTest.gltf";
const MORPH_STRESS_TEST_ANIMATION_INDEX: usize = 2;

#[derive(Default)]
struct MorphTargetsApp;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "MorphTargets".to_owned(),
        ..Default::default()
    });
    runtime
        .run_xrds(MorphTargetsApp::default())
        .expect("Could not run application");
}

impl XrdsApp for MorphTargetsApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _ambient = api.spawn(&{
            let mut light = XrdsAmbientLight::new().with_name("MorphTargetsAmbient");
            light.brightness = 150.0;
            light
        });

        let gltf_handle =
            api.spawn(&XrdsGltfAsset::new(MORPH_STRESS_TEST_PATH).with_name("MorphStressTest"));
        api.play_gltf_animation(
            &gltf_handle,
            XrdsGltfAnimationSelector::Index(MORPH_STRESS_TEST_ANIMATION_INDEX),
            XrdsGltfAnimationPlaybackOptions::default(),
        )
        .expect("MorphStressTest animation request should queue until the scene is ready");

        let _sun = api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("MorphTargetsSun");
            light.transform.rotation_euler_xyz_deg = [0.0, 0.0, 90.0];
            light
        });

        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("MorphTargetsCamera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([3.0, 2.1, 10.2])
                .looking_at([0.0, 0.0, 0.0])
        });
    }
}
