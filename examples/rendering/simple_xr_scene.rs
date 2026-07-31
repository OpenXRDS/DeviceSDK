use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D},
    world::lights::XrdsPointLight,
    world::XrdsCamera,
    XrdsColor,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp};

struct SimpleXrSceneApp;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "SimpleXRScene".to_owned(),
        asset_path: None,
        enable_xr: true,
        ..Default::default()
    });
    runtime
        .run_xrds(SimpleXrSceneApp)
        .expect("Could not run application");
}

impl XrdsApp for SimpleXrSceneApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("SimpleXrSceneCamera")
                .near(0.1)
                .far(200.0)
                .at([-2.5, 4.5, 9.0])
                .looking_at([0.0, 0.5, 0.0])
        });

        let floor = api.spawn(&{
            let mut plane = XrdsPlane3D::new().with_name("SimpleXrSceneFloor");
            plane.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            plane.size = [8.0, 8.0];
            plane
        });
        api.set_material_base_color(&floor, XrdsColor::WHITE);

        let cube = api.spawn(&{
            let mut cube = XrdsCube::new().with_name("SimpleXrSceneCube");
            cube.transform.translation = [0.0, 0.5, 0.0];
            cube
        });
        api.set_material_base_color(&cube, XrdsColor::srgb(124.0 / 255.0, 144.0 / 255.0, 1.0));

        let _light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("SimpleXrSceneLight");
            light.transform.translation = [4.0, 8.0, 4.0];
            light.intensity = 1_200_000.0;
            light.range = 30.0;
            light.shadows = true;
            light
        });
    }
}
