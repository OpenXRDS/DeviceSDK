use xrds::sdk::{
    primitives::XrdsCube,
    world::{
        lights::{XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight},
        XrdsCamera,
    },
    CubeGeometryParams, DirectionalLightParams, PerspectiveCameraParams, TransformParams,
    XrdsColor,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct SimpleAPIApp {
    cube_handle: Option<Handle<XrdsCube>>,
}

impl XrdsApp for SimpleAPIApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let camera = api.spawn(&{
            XrdsCamera::new()
                .with_name("SimpleAPICamera")
                .looking_at([0.0, 0.0, 0.0])
        });

        // Set the camera's position using the simple API method for setting translation
        api.set_translation(&camera, [0.0, 2.0, 6.0]);
        api.set_camera_perspective(
            &camera,
            PerspectiveCameraParams {
                fov_deg: 55.0,
                near: 0.1,
                far: Some(200.0),
                order: 0,
            },
        );

        let _point_light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("SimpleAPIPointLight");
            light.transform.translation = [0.0, 5.0, 4.0];
            light.intensity = 1_200_000.0;
            light.shadows = true;
            light
        });

        let directional_light = api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("SimpleAPIDirectional");
            light.transform.translation = [4.0, 8.0, 4.0];
            light.illuminance = 4_000.0;
            light.shadows = true;
            light
        });

        api.set_directional_light_params(
            &directional_light,
            DirectionalLightParams {
                color: XrdsColor::srgb(1.0, 0.0, 0.0),
                illuminance: 8_000.0,
                shadows: true,
            },
        );

        let _ambient_light = api.spawn(&{
            let mut light = XrdsAmbientLight::new().with_name("SimpleAPIAmbient");
            light.brightness = 60.0;
            light
        });

        let cube_handle = api.spawn(&{
            let mut cube = XrdsCube::new().with_name("SimpleAPICube");
            cube.transform.translation = [0.0, 0.5, 0.0];
            cube
        });
        api.set_material_base_color(&cube_handle, XrdsColor::srgb(1.0, 1.0, 1.0));

        api.set_cube_geometry(
            &cube_handle,
            CubeGeometryParams {
                size: [2.0, 2.0, 2.0],
            },
        );
        self.cube_handle = Some(cube_handle);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let cube_handle = self
            .cube_handle
            .as_ref()
            .expect("cube handle should be initialized during setup");
        let t = ctx.elapsed_secs();
        let yaw_radians = (t * 45.0).to_radians();
        let half_yaw: f32 = 0.5 * yaw_radians;
        let bob_y = 0.5 + 0.15 * (t * 1.2).sin();
        let r = 0.5 + 0.5 * t.sin();
        let g = 0.5 + 0.5 * (t * 1.7).sin();
        let b = 0.5 + 0.5 * (t * 2.3).sin();

        // Generic typed patch path: this is the extension-first API that also works for custom
        // components once they register a TransformParams updater.
        ctx.queue_update(
            cube_handle,
            TransformParams {
                translation: [0.0, bob_y, 0.0],
                rotation_quat_xyzw: [0.0, half_yaw.sin(), 0.0, half_yaw.cos()],
                scale: [1.0, 1.0, 1.0],
            },
        );
        ctx.set_rotation(cube_handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);

        // Update using the simple helper methods instead of raw patch structs.
        ctx.set_material_base_color(cube_handle, XrdsColor::srgb(r, g, b));
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(SimpleAPIApp::default())
        .expect("failed to run simple_api");
}
