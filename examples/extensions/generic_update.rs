use xrds::sdk::{
    world::{lights::XrdsPointLight, XrdsCamera},
    TransformParams, XrdsColor, XrdsComponent, XrdsMaterialParams, XrdsObject,
};
use xrds::{
    Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsGeometrySource, XrdsUpdateContext,
};

#[derive(Debug, Clone)]
struct PulseCube {
    name: String,
    visible: bool,
    transform: TransformParams,
    size: [f32; 3],
    color: XrdsColor,
}

impl PulseCube {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            visible: true,
            transform: TransformParams::default(),
            size: [1.5, 1.5, 1.5],
            color: XrdsColor::srgb(0.3, 0.7, 1.0),
        }
    }
}

impl XrdsObject for PulseCube {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for PulseCube {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

#[derive(Debug, Clone, Copy)]
struct PulseCubePatch {
    color: XrdsColor,
    size: [f32; 3],
}

#[derive(Default)]
struct GenericUpdateApp {
    cube_handle: Option<Handle<PulseCube>>,
}

impl XrdsApp for GenericUpdateApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let camera = api.spawn(&{
            XrdsCamera::new()
                .with_name("GenericUpdateCamera")
                .looking_at([0.0, 0.0, 0.0])
        });
        api.set_translation(&camera, [0.0, 2.5, 6.0]);

        let _light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("GenericUpdateLight");
            light.transform.translation = [3.0, 6.0, 4.0];
            light.intensity = 1_200_000.0;
            light.shadows = true;
            light
        });

        // Custom XRDS types are open at the descriptor layer, but their visible shape is still
        // expected to come from XRDS-provided geometry/material sources.
        // Here PulseCube is a user-defined type that maps onto the SDK's cuboid surface path.
        api.register_surface_interpreter::<PulseCube, _>(|cube| XrdsGeometrySource::PbrCuboid {
            half_extents: [cube.size[0] * 0.5, cube.size[1] * 0.5, cube.size[2] * 0.5],
            material: XrdsMaterialParams {
                base_color: cube.color,
                pbr: Default::default(),
                ..Default::default()
            },
        });

        /*
           sdk user still registers a custom updater for custom patch types.
           XRDS keeps the descriptor open, while the runtime realization stays inside the SDK's
           supported shape vocabulary.
        */
        api.register_recipe_updater::<PulseCube, PulseCubePatch, _>(|cube, patch| {
            cube.color = patch.color;
            cube.size = patch.size;
        });

        let cube_handle = api.spawn(&{
            let mut cube = PulseCube::new("GenericUpdateCube");
            cube.transform.translation = [0.0, 1.5, 0.0];
            cube
        });

        self.cube_handle = Some(cube_handle);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let cube_handle = self
            .cube_handle
            .as_ref()
            .expect("custom cube handle should be initialized during setup");

        let t = ctx.elapsed_secs();
        let pulse = 0.5 + 0.5 * (t * 1.5).sin();
        let size = [0.75 + pulse * 1.75, 0.6 + pulse * 1.2, 0.75 + pulse * 1.75];
        let color = XrdsColor::srgb(1.0 - pulse, 0.2 + 0.8 * pulse, 0.15 + 0.85 * pulse);
        let bob_y = 1.0 + 1.2 * (t * 1.5).sin();
        let yaw = t * 1.4;
        let half_yaw = yaw * 0.5;

        // This is the generic extension path: custom handle + custom patch type.
        // The patch mutates the custom descriptor only; XRDS rebuilds the cuboid realization.
        ctx.queue_update(cube_handle, PulseCubePatch { color, size });

        // Add obvious motion so it is easy to see the example updating every frame.
        ctx.set_translation(cube_handle, [0.0, bob_y, 0.0]);
        ctx.set_rotation(cube_handle, [0.0, half_yaw.sin(), 0.0, half_yaw.cos()]);
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(GenericUpdateApp::default())
        .expect("failed to run generic_update");
}
