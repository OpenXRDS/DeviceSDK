use xrds::sdk::{
    primitives::XrdsCube,
    world::{
        lights::{XrdsAmbientLight, XrdsDirectionalLight, XrdsPointLight, XrdsSpotLight},
        XrdsCamera,
    },
    XrdsColor,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsKey, XrdsUpdateContext};

const POINT_INTENSITY_ON: f32 = 50_000.0;
const SPOT_INTENSITY_ON: f32 = 12_000_000.0;
const DIRECTIONAL_ILLUMINANCE_ON: f32 = 1_000.0;
const AMBIENT_BRIGHTNESS_ON: f32 = 800.0;

#[derive(Default)]
struct ActiveControlApp {
    point_light: Option<Handle<XrdsPointLight>>,
    directional_light: Option<Handle<XrdsDirectionalLight>>,
    cube: Option<Handle<XrdsCube>>,
    spot_light: Option<Handle<XrdsSpotLight>>,
    ambient_light: Option<Handle<XrdsAmbientLight>>,
}

impl XrdsApp for ActiveControlApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let camera = api.spawn(&{
            XrdsCamera::new()
                .with_name("ActiveControlCamera")
                .looking_at([0.0, 0.0, 0.0])
        });
        api.set_translation(&camera, [0.0, 3.0, 8.0]);

        self.point_light = Some(api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("PointLight");
            light.transform.translation = [0.0, 3.0, 0.0];
            light.intensity = POINT_INTENSITY_ON;
            light
        }));

        self.directional_light = Some(api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("DirectionalLight");
            light.transform.translation = [10.0, 0.0, 5.0];
            light.illuminance = DIRECTIONAL_ILLUMINANCE_ON;
            light.shadows = true;
            light
        }));

        self.cube = Some(api.spawn(&{
            let mut mesh = XrdsCube::new().with_name("ControlCube");
            mesh.transform.translation = [-1.25, 0.5, 0.0];
            mesh.size = [1.0, 1.0, 1.0];
            mesh
        }));

        self.spot_light = Some(api.spawn(&{
            let mut light = XrdsSpotLight::new().with_name("SpotLight");
            light.transform.translation = [0.0, 4.0, 3.5];
            light.intensity = SPOT_INTENSITY_ON;
            light.range = 20.0;
            light.inner_angle = 0.35;
            light.outer_angle = 0.8;
            light.shadows = true;
            light
        }));

        self.ambient_light = Some(api.spawn(&{
            let mut light = XrdsAmbientLight::new().with_name("AmbientLight");
            light.brightness = AMBIENT_BRIGHTNESS_ON;
            light.color = XrdsColor::srgb(0.0, 0.0, 1.0);
            light
        }));

        println!(
            "Controls: [1] point, [2] spot, [3] directional, [4] ambient, [C] rotate cube, [R] reset"
        );
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let point_light = self
            .point_light
            .as_ref()
            .expect("point light handle should be initialized during setup");
        let directional_light = self
            .directional_light
            .as_ref()
            .expect("directional light handle should be initialized during setup");
        let cube = self
            .cube
            .as_ref()
            .expect("cube handle should be initialized during setup");
        let spot_light = self
            .spot_light
            .as_ref()
            .expect("spot light handle should be initialized during setup");
        let ambient_light = self
            .ambient_light
            .as_ref()
            .expect("ambient light handle should be initialized during setup");

        if ctx.key_just_pressed(XrdsKey::Digit1) {
            let on = ctx
                .point_light_intensity(point_light)
                .map(|v| v <= 0.0)
                .unwrap_or(true);
            ctx.set_point_light_intensity(point_light, if on { POINT_INTENSITY_ON } else { 0.0 });
        }

        if ctx.key_just_pressed(XrdsKey::Digit2) {
            let on = ctx
                .spot_light_intensity(spot_light)
                .map(|v| v <= 0.0)
                .unwrap_or(true);
            ctx.set_spot_light_intensity(spot_light, if on { SPOT_INTENSITY_ON } else { 0.0 });
        }

        if ctx.key_just_pressed(XrdsKey::Digit3) {
            let on = ctx
                .directional_light_illuminance(directional_light)
                .map(|v| v <= 0.0)
                .unwrap_or(true);
            ctx.set_directional_light_illuminance(
                directional_light,
                if on { DIRECTIONAL_ILLUMINANCE_ON } else { 0.0 },
            );
        }

        if ctx.key_just_pressed(XrdsKey::Digit4) {
            let on = ctx
                .ambient_light_brightness(ambient_light)
                .map(|v| v <= 0.0)
                .unwrap_or(true);
            ctx.set_ambient_light_brightness(
                ambient_light,
                if on { AMBIENT_BRIGHTNESS_ON } else { 0.0 },
            );
        }

        if ctx.key_just_pressed(XrdsKey::KeyC) {
            ctx.rotate_y(cube, 45.0_f32.to_radians());
        }

        if ctx.key_just_pressed(XrdsKey::KeyR) {
            ctx.set_point_light_intensity(point_light, POINT_INTENSITY_ON);
            ctx.set_spot_light_intensity(spot_light, SPOT_INTENSITY_ON);
            ctx.set_directional_light_illuminance(directional_light, DIRECTIONAL_ILLUMINANCE_ON);
            ctx.set_ambient_light_brightness(ambient_light, AMBIENT_BRIGHTNESS_ON);
        }
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(ActiveControlApp::default())
        .expect("failed to run active_control");
}
