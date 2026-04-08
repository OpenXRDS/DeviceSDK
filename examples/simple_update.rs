use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D},
    world::{lights::XrdsPointLight, XrdsCamera},
    XrdsColor,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct SimpleUpdateApp {
    light_handle: Option<Handle<XrdsPointLight>>,
}

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "SimpleUpdate".to_owned(),
        ..Default::default()
    });
    runtime
        .run_xrds(SimpleUpdateApp::default())
        .expect("Could not run application");
}

impl XrdsApp for SimpleUpdateApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("SimpleUpdateCamera")
                .near(0.1)
                .far(200.0)
                .at([-2.5, 4.5, 9.0])
                .looking_at([0.0, 0.5, 0.0])
        });

        let floor = api.spawn(&{
            let mut plane = XrdsPlane3D::new().with_name("SimpleUpdateFloor");
            plane.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            plane.size = [8.0, 8.0];
            plane
        });
        api.set_material_base_color(&floor, XrdsColor::WHITE);

        let cube = api.spawn(&{
            let mut cube = XrdsCube::new().with_name("SimpleUpdateCube");
            cube.transform.translation = [0.0, 0.5, 0.0];
            cube
        });
        api.set_material_base_color(&cube, XrdsColor::srgb(124.0 / 255.0, 144.0 / 255.0, 1.0));

        let light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("SimpleUpdateLight");
            light.transform.translation = [0.0, 3.0, 5.0];
            light.intensity = 1_200_000.0;
            light.range = 30.0;
            light.shadows = true;
            light
        });

        self.light_handle = Some(light);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let light = self
            .light_handle
            .as_ref()
            .expect("light handle should be initialized during setup");

        let t = ctx.elapsed_secs();
        ctx.set_translation(light, [5.0 * t.cos(), 3.0, 5.0 * t.sin()]);
    }
}
