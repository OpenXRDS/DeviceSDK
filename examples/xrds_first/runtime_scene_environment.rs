use xrds::scene_graph::{
    XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneEnvironment, XrdsSceneExposureEnvironment,
    XrdsSceneFogEnvironment, XrdsSceneIblEnvironment, XrdsSceneSkyboxEnvironment,
};
use xrds::sdk::{
    primitives::{XrdsPlane3D, XrdsSphere},
    world::{lights::XrdsPointLight, XrdsCamera},
    XrdsClearColorConfig, XrdsColor, XrdsLinearRgba, XrdsMaterialAlphaMode, XrdsMaterialParams,
    XrdsMaterialPbrParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const IBL_DIFFUSE_ASSET_ID: &str = "asset:ibl-diffuse";
const IBL_SPECULAR_ASSET_ID: &str = "asset:ibl-specular";
const SKYBOX_ASSET_ID: &str = "asset:skybox";

#[derive(Default)]
struct RuntimeSceneEnvironmentApp {
    polished_sphere: Option<Handle<XrdsSphere>>,
    rough_sphere: Option<Handle<XrdsSphere>>,
    environment_cleared: bool,
    environment_restored: bool,
}

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "RuntimeSceneEnvironment".to_owned(),
        ..Default::default()
    })
    .run_xrds(RuntimeSceneEnvironmentApp::default())
    .expect("failed to run runtime_scene_environment example");
}

impl XrdsApp for RuntimeSceneEnvironmentApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // Runtime-first scene construction still uses scene-graph asset ids for environment
        // policy, because the policy needs durable texture references instead of raw handles.
        api.merge_scene_assets(&[
            XrdsSceneAsset {
                id: IBL_DIFFUSE_ASSET_ID.to_string(),
                uri: "environment_maps/diffuse.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: IBL_SPECULAR_ASSET_ID.to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: SKYBOX_ASSET_ID.to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ]);

        // Once the asset ids are known to XRDS, the runtime can project scene IBL, skybox,
        // manual exposure, and linear fog onto managed 3D cameras without going through authored document
        // import.
        api.set_scene_environment(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: IBL_DIFFUSE_ASSET_ID.to_string(),
                specular_asset_id: IBL_SPECULAR_ASSET_ID.to_string(),
                intensity: 900.0,
            }),
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: SKYBOX_ASSET_ID.to_string(),
                brightness: 900.0,
            }),
            exposure: Some(XrdsSceneExposureEnvironment { ev100: 6.0 }),
            fog: Some(XrdsSceneFogEnvironment {
                color: [0.35, 0.48, 0.66, 1.0],
                start: 5.0,
                end: 40.0,
            }),
            ..Default::default()
        });

        let camera = api.spawn(
            &XrdsCamera::new()
                .with_name("RuntimeEnvironmentCamera")
                .with_clear_color(XrdsClearColorConfig::Custom(XrdsColor::srgb(
                    0.015, 0.02, 0.03,
                )))
                .at([0.0, 2.2, 7.0])
                .looking_at([0.0, 1.0, 0.0]),
        );
        api.set_camera_perspective(
            &camera,
            xrds::sdk::PerspectiveCameraParams {
                fov_deg: 42.0,
                near: 0.1,
                far: Some(100.0),
                order: 0,
            },
        );

        let light = api.spawn(&{
            let mut light = XrdsPointLight::new().with_name("RuntimeEnvironmentLight");
            light.transform.translation = [0.0, 4.5, 3.5];
            light.intensity = 45_000.0;
            light.range = 25.0;
            light.shadows = true;
            light
        });
        api.set_visible(&light, true);

        let floor = api.spawn(&{
            let mut floor = XrdsPlane3D::new().with_name("RuntimeEnvironmentFloor");
            floor.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            floor.size = [8.0, 8.0];
            floor
        });
        api.set_material_params(
            &floor,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.12, 0.12, 0.14),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: XrdsMaterialPbrParams {
                    metallic: 0.0,
                    roughness: 0.92,
                    reflectance: 0.2,
                    double_sided: true,
                    alpha_mode: XrdsMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                },
                textures: Default::default(),
            },
        );

        let polished_sphere = api.spawn(&{
            let mut sphere = XrdsSphere::new().with_name("RuntimeEnvironmentPolishedSphere");
            sphere.transform.translation = [-1.2, 1.0, 0.0];
            sphere.radius = 1.0;
            sphere
        });
        api.set_material_params(
            &polished_sphere,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.92, 0.95, 1.0),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: XrdsMaterialPbrParams {
                    metallic: 1.0,
                    roughness: 0.08,
                    reflectance: 0.95,
                    double_sided: false,
                    alpha_mode: XrdsMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                },
                textures: Default::default(),
            },
        );

        let rough_sphere = api.spawn(&{
            let mut sphere = XrdsSphere::new().with_name("RuntimeEnvironmentRoughSphere");
            sphere.transform.translation = [1.2, 1.0, 0.0];
            sphere.radius = 1.0;
            sphere
        });
        api.set_material_params(
            &rough_sphere,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.96, 0.8, 0.62),
                emissive: XrdsLinearRgba::BLACK,
                opacity: 1.0,
                unlit: false,
                pbr: XrdsMaterialPbrParams {
                    metallic: 1.0,
                    roughness: 0.72,
                    reflectance: 0.9,
                    double_sided: false,
                    alpha_mode: XrdsMaterialAlphaMode::Opaque,
                    alpha_cutoff: 0.5,
                },
                textures: Default::default(),
            },
        );

        self.polished_sphere = Some(polished_sphere);
        self.rough_sphere = Some(rough_sphere);

        println!("Runtime scene environment example is running.");
        println!("Initial state: scene IBL, skybox, manual exposure, and linear fog are enabled through runtime APIs.");
        println!("Around 4 seconds: XRDS clears the scene environment policy.");
        println!("Around 8 seconds: XRDS restores the same scene environment policy.");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let polished_sphere = self
            .polished_sphere
            .as_ref()
            .expect("polished sphere handle should be initialized during setup");
        let rough_sphere = self
            .rough_sphere
            .as_ref()
            .expect("rough sphere handle should be initialized during setup");

        let t = ctx.elapsed_secs();
        let polished_half_yaw = 0.5 * (t * 0.9);
        let rough_half_yaw = 0.5 * (-t * 0.6);
        ctx.set_rotation(
            polished_sphere,
            [0.0, polished_half_yaw.sin(), 0.0, polished_half_yaw.cos()],
        );
        ctx.set_rotation(
            rough_sphere,
            [0.0, rough_half_yaw.sin(), 0.0, rough_half_yaw.cos()],
        );

        // The runtime API can change global scene environment policy live. This shows the scene
        // dimming when managed environment maps are removed, then returning when restored.
        if !self.environment_cleared && t >= 4.0 {
            ctx.clear_scene_environment();
            self.environment_cleared = true;
            println!("Runtime scene environment cleared.");
        }

        if !self.environment_restored && t >= 8.0 {
            ctx.set_scene_environment(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: IBL_DIFFUSE_ASSET_ID.to_string(),
                    specular_asset_id: IBL_SPECULAR_ASSET_ID.to_string(),
                    intensity: 900.0,
                }),
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: SKYBOX_ASSET_ID.to_string(),
                    brightness: 900.0,
                }),
                exposure: Some(XrdsSceneExposureEnvironment { ev100: 6.0 }),
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    start: 5.0,
                    end: 40.0,
                }),
                ..Default::default()
            });
            self.environment_restored = true;
            println!("Runtime scene environment restored.");
        }
    }
}
