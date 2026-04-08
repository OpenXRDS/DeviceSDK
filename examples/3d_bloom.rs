use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use xrds::sdk::{
    primitives::XrdsSphere, world::XrdsCamera, XrdsBloom, XrdsClearColorConfig, XrdsColor,
    XrdsLinearRgba, XrdsTonemapping,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

struct BloomSphere {
    handle: Handle<XrdsSphere>,
    base_x: f32,
    base_z: f32,
}

#[derive(Default)]
struct Handler {
    spheres: Vec<BloomSphere>,
}

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "3dBloom".to_owned(),
        ..Default::default()
    });
    runtime
        .run_xrds(Handler::default())
        .expect("Could not run application");
}

impl XrdsApp for Handler {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        self.spheres = setup(api);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let t = ctx.elapsed_secs();

        for sphere in &self.spheres {
            let y = (sphere.base_x + sphere.base_z + t).sin();
            ctx.set_translation(&sphere.handle, [sphere.base_x, y, sphere.base_z]);
        }
    }
}

fn setup(api: &mut XrdsAPI<'_>) -> Vec<BloomSphere> {
    api.spawn(&{
        let mut camera = XrdsCamera::new()
            .with_name("BloomCamera")
            .looking_at([0.0, 0.0, 0.0])
            .with_clear_color(XrdsClearColorConfig::Custom(XrdsColor::BLACK))
            .with_tonemapping(XrdsTonemapping::TonyMcMapface)
            .with_bloom(XrdsBloom::Natural);
        camera.transform.translation = [-2.0, 2.5, 5.0];
        camera
    });

    let mut spheres = Vec::new();

    for x in -5..5 {
        for z in -5..5 {
            // This generates a pseudo-random integer between `[0, 6)`, but deterministically so
            // the same spheres are always the same colors.
            let mut hasher = DefaultHasher::new();
            (x, z).hash(&mut hasher);
            let rand = (hasher.finish() + 3) % 6;

            let (emissive, scale) = match rand {
                0 => (XrdsLinearRgba::rgb(0.0, 0.0, 150.0), 0.5),
                1 => (XrdsLinearRgba::rgb(1000.0, 1000.0, 1000.0), 0.1),
                2 => (XrdsLinearRgba::rgb(50.0, 0.0, 0.0), 1.0),
                3..=5 => (XrdsLinearRgba::BLACK, 1.5),
                _ => unreachable!(),
            };

            let sphere = api.spawn(&{
                let mut sphere = XrdsSphere::new().with_name(format!("BloomSphere_{x}_{z}"));
                sphere.radius = 0.4;
                let base_x = x as f32 * 2.0;
                let base_z = z as f32 * 2.0;
                sphere.transform.translation = [base_x, 0.0, base_z];
                sphere.transform.scale = [scale, scale, scale];
                sphere
            });
            api.set_material_base_color(&sphere, XrdsColor::BLACK);
            api.set_material_emissive(&sphere, emissive);

            spheres.push(BloomSphere {
                handle: sphere,
                base_x: x as f32 * 2.0,
                base_z: z as f32 * 2.0,
            });
        }
    }

    spheres
}
