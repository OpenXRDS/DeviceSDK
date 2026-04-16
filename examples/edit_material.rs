use xrds::sdk::{
    primitives::{XrdsPlane3D, XrdsSphere},
    world::{lights::XrdsPointLight, XrdsCamera},
    XrdsClearColorConfig, XrdsColor, XrdsLinearRgba, XrdsMaterialAlphaMode, XrdsMaterialParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct EditMaterialApp {
    hero_sphere_handle: Option<Handle<XrdsSphere>>,
    reference_sphere_handle: Option<Handle<XrdsSphere>>,
    light_handle: Option<Handle<XrdsPointLight>>,
}

impl XrdsApp for EditMaterialApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let camera = api.spawn(
            &XrdsCamera::new()
                .with_name("EditMaterialCamera")
                .with_clear_color(XrdsClearColorConfig::Custom(XrdsColor::srgb(
                    0.015, 0.02, 0.03,
                ))),
        );
        api.set_translation(&camera, [0.0, 2.4, 7.5]);
        api.set_camera_look_at(&camera, Some([0.0, 0.9, 0.0]));

        let light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("EditMaterialLight");
            light.transform.translation = [3.5, 3.8, 3.5];
            light.intensity = 4_000_000.0;
            light.range = 30.0;
            light.radius = 0.2;
            light.shadows = true;
            light
        });

        let _floor = api.spawn(&{
            let mut floor = XrdsPlane3D::new().with_name("MaterialFloor");
            floor.transform.translation = [0.0, -0.05, 0.0];
            floor.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            floor.size = [10.0, 10.0];
            floor
        });
        api.set_material_params(
            &_floor,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.08, 0.09, 0.11),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: Default::default(),
                textures: Default::default(),
            },
        );

        let hero_sphere = api.spawn(&{
            let mut sphere = XrdsSphere::new().with_name("HeroMaterialSphere");
            sphere.transform.translation = [-1.35, 1.0, 0.0];
            sphere.radius = 1.0;
            sphere
        });

        api.set_material_params(
            &hero_sphere,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.9, 0.55, 0.2),
                emissive: XrdsLinearRgba::rgb(0.0, 0.0, 0.0),
                opacity: 1.0,
                unlit: false,
                pbr: Default::default(),
                textures: Default::default(),
            },
        );

        let reference_sphere = api.spawn(&{
            let mut sphere = XrdsSphere::new().with_name("ReferenceSphere");
            sphere.transform.translation = [1.35, 1.0, 0.0];
            sphere.radius = 1.0;
            sphere
        });
        api.set_material_params(
            &reference_sphere,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.82, 0.84, 0.88),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: xrds::sdk::XrdsMaterialPbrParams {
                    metallic: 0.0,
                    roughness: 0.92,
                    reflectance: 0.35,
                    double_sided: false,
                    alpha_mode: XrdsMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                },
                textures: Default::default(),
            },
        );

        self.hero_sphere_handle = Some(hero_sphere);
        self.reference_sphere_handle = Some(reference_sphere);
        self.light_handle = Some(light);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let hero_sphere = self
            .hero_sphere_handle
            .as_ref()
            .expect("hero sphere handle should be initialized during setup");
        let reference_sphere = self
            .reference_sphere_handle
            .as_ref()
            .expect("reference sphere handle should be initialized during setup");
        let light = self
            .light_handle
            .as_ref()
            .expect("light handle should be initialized during setup");

        let t = ctx.elapsed_secs();
        let sweep = 0.5 + 0.5 * (t * 0.8).sin();
        let glow = 0.5 + 0.5 * (t * 1.9).sin();
        let orbit = t * 0.9;
        let half_orbit = orbit * 0.5;

        ctx.set_translation(
            light,
            [orbit.cos() * 4.2, 3.0 + glow * 2.2, orbit.sin() * 3.2],
        );

        ctx.set_rotation(hero_sphere, [0.0, half_orbit.sin(), 0.0, half_orbit.cos()]);
        ctx.set_rotation(
            reference_sphere,
            [0.0, -half_orbit.sin(), 0.0, half_orbit.cos()],
        );

        let mut material = ctx.material_params(hero_sphere).unwrap_or_default();
        material.base_color = XrdsColor::srgba(
            0.2 + sweep * 0.75,
            0.22 + (1.0 - sweep) * 0.45,
            0.28 + glow * 0.55,
            1.0,
        );
        material.emissive = XrdsLinearRgba::rgb(0.02 + glow * 0.18, 0.01, 0.03 + glow * 0.08);
        material.opacity = 1.0;
        material.unlit = false;
        material.pbr.metallic = sweep;
        material.pbr.roughness = 0.04 + (1.0 - sweep) * 0.9;
        material.pbr.reflectance = 0.25 + glow * 0.55;
        material.pbr.double_sided = false;
        material.pbr.alpha_mode = XrdsMaterialAlphaMode::Opaque;

        ctx.set_material_params(hero_sphere, material);
    }
}

fn main() {
    Runtime::new(RuntimeParameters::default())
        .run_xrds(EditMaterialApp::default())
        .expect("failed to run edit_material");
}
